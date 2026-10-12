//! Editable native persistence with bounded decoding and atomic replacement.
//!
//! This initial format preserves supported artwork and original RGBA8 sources.
//! Unknown formats, fields, operations, extra bytes, and corrupted data are refused.
//! Session history is not persisted yet. File publication and directory durability
//! have separate outcomes: a failed directory sync cannot undo a completed rename.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod raster;
mod wire;

pub use raster::{ImportedRaster, PngLimits, SourceColor, load_png, save_png};

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use affint_document::{Revision, Session, Snapshot};
use tempfile::NamedTempFile;

/// Independent serialized-input and source-allocation limits.
#[derive(Debug, Clone, Copy)]
pub struct FileLimits {
    /// Maximum bytes in the entire native archive.
    pub max_file_bytes: u64,
    /// Maximum UTF-8 JSON manifest bytes, checked before decoder allocation.
    pub max_manifest_bytes: usize,
    /// Maximum combined bytes of distinct original pixel blobs during load/save.
    pub max_source_bytes: usize,
}

impl Default for FileLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 600 * 1024 * 1024,
            max_manifest_bytes: 8 * 1024 * 1024,
            max_source_bytes: 512 * 1024 * 1024,
        }
    }
}

/// A validated persistence policy. Heavy file operations belong on a worker.
#[derive(Debug, Clone)]
pub struct Store {
    file_limits: FileLimits,
    document_limits: affint_document::Limits,
}

impl Store {
    /// Creates a policy with finite, nonzero file limits.
    pub fn new(
        file_limits: FileLimits,
        document_limits: affint_document::Limits,
    ) -> Result<Self, Error> {
        if file_limits.max_file_bytes < wire::HEADER_BYTES as u64
            || file_limits.max_manifest_bytes == 0
            || file_limits.max_source_bytes == 0
        {
            return Err(Error::InvalidFormat("zero or inconsistent file limits"));
        }
        Ok(Self {
            file_limits,
            document_limits,
        })
    }

    /// Loads and validates a complete editable document before returning it.
    ///
    /// No caller's current session is changed on failure. This format never
    /// extracts files or follows archive-supplied filesystem paths.
    pub fn load(&self, path: impl AsRef<Path>) -> Result<Session, Error> {
        let mut file = File::open(path.as_ref())?;
        let length = file.metadata()?.len();
        self.decode(&mut file, length)
    }

    /// Writes a same-directory temporary file, syncs it, and replaces the target.
    ///
    /// Errors before publication preserve the existing target. A successful
    /// receipt names the saved revision, so an older background save cannot mark
    /// newer edits clean. Inspect its durability field for directory-sync failures.
    pub fn save(&self, snapshot: &Snapshot, path: impl AsRef<Path>) -> Result<SaveReceipt, Error> {
        let durability = replace_atomically(path.as_ref(), |file| self.encode(snapshot, file))?;
        Ok(SaveReceipt {
            revision: snapshot.revision(),
            durability,
        })
    }

    /// Decodes a bounded seekable stream, such as a file or a test fixture.
    ///
    /// `length` must describe the complete stream; exact EOF is also checked.
    /// No binary sequence length is given directly to Serde. JSON has a byte cap
    /// and its standard recursion limit; source allocation is admitted separately.
    pub fn decode<R: Read + Seek>(&self, reader: &mut R, length: u64) -> Result<Session, Error> {
        check_limit("file bytes", length, self.file_limits.max_file_bytes)?;
        if length < wire::HEADER_BYTES as u64 {
            return Err(Error::InvalidFormat("truncated header"));
        }
        reader.seek(SeekFrom::Start(0))?;
        wire::decode(
            reader,
            length,
            self.file_limits,
            self.document_limits.clone(),
        )
    }

    /// Encodes supported editable content without copying source pixels.
    ///
    /// The stream must be empty or truncated by its owner; this method does not
    /// publish files. Use [`Self::save`] for atomic native persistence.
    pub fn encode<W: Write>(&self, snapshot: &Snapshot, writer: &mut W) -> Result<(), Error> {
        wire::encode(snapshot, writer, self.file_limits)
    }
}

/// Whether a successfully published file has confirmed directory durability.
#[derive(Debug)]
pub enum Durability {
    /// The file and containing directory were synchronized successfully.
    Synced,
    /// Replacement succeeded, but synchronizing its containing directory failed.
    PublishedUnconfirmed {
        /// Directory whose rename durability could not be confirmed.
        directory: PathBuf,
        /// Actual operating-system failure; it is never silently suppressed.
        source: io::Error,
    },
    /// This platform's directory-sync behavior has not been implemented/verified.
    PlatformUnverified,
}

/// The exact artwork revision whose editable file was successfully published.
#[derive(Debug)]
pub struct SaveReceipt {
    /// Saved revision, for background completion and dirty-state handling.
    pub revision: Revision,
    /// Publication succeeded; the durability outcome may still need attention.
    pub durability: Durability,
}

/// A rejected archive or file operation. Failed decode never publishes artwork.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem or stream operation failed before publication.
    #[error("file operation failed: {0}")]
    Io(#[from] io::Error),
    /// Manifest decoding/encoding failed, including unknown fields or variants.
    #[error("invalid manifest: {0}")]
    Json(#[from] serde_json::Error),
    /// Document invariants or resource admission failed.
    #[error("invalid artwork: {0}")]
    Document(#[from] affint_document::Error),
    /// PNG stream validation or decoding failed.
    #[error("PNG decode failed: {0}")]
    PngDecode(#[from] png::DecodingError),
    /// PNG encoding or finalization failed.
    #[error("PNG export failed: {0}")]
    PngEncode(#[from] png::EncodingError),
    /// A source needs a precision/color/orientation policy not implemented yet.
    #[error("unsupported image: {0}")]
    UnsupportedImage(&'static str),
    /// The framing, schema, references, or exact lengths are unsupported/invalid.
    #[error("invalid or unsupported AFFINT archive: {0}")]
    InvalidFormat(&'static str),
    /// Manifest or original source bytes do not match their saved digest.
    #[error("archive checksum mismatch: {0}")]
    Checksum(&'static str),
    /// A finite input or allocation budget was exceeded before pixel allocation.
    #[error("{resource} needs {actual}, limit {limit}")]
    Limit {
        /// Named bounded resource.
        resource: &'static str,
        /// Required or supplied amount.
        actual: u64,
        /// Configured maximum.
        limit: u64,
    },
    /// Application-owned bounded storage allocation could not be reserved.
    #[error("could not reserve {0} storage bytes")]
    Allocation(usize),
    /// Input-derived size arithmetic exceeded the target's addressable range.
    #[error("archive size overflow")]
    Overflow,
}

pub(crate) fn check_limit(resource: &'static str, actual: u64, limit: u64) -> Result<(), Error> {
    if actual > limit {
        return Err(Error::Limit {
            resource,
            actual,
            limit,
        });
    }
    Ok(())
}

fn replace_atomically(
    path: &Path,
    encode: impl FnOnce(&mut File) -> Result<(), Error>,
) -> Result<Durability, Error> {
    if path.file_name().is_none() {
        return Err(Error::InvalidFormat("destination must name a file"));
    }
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut temporary = NamedTempFile::new_in(parent)?;
    encode(temporary.as_file_mut())?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    let published = temporary.persist(path).map_err(|error| error.error)?;
    // Keep the published handle alive until directory synchronization completes.
    let durability = sync_directory(parent);
    drop(published);
    Ok(durability)
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> Durability {
    match File::open(directory).and_then(|file| file.sync_all()) {
        Ok(()) => Durability::Synced,
        Err(source) => Durability::PublishedUnconfirmed {
            directory: directory.to_owned(),
            source,
        },
    }
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Durability {
    Durability::PlatformUnverified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_write_preserves_the_existing_destination_and_removes_staging() {
        let directory = tempfile::tempdir().expect("test directory");
        let path = directory.path().join("art.affint");
        std::fs::write(&path, b"prior valid artwork").expect("prior file");
        let result = replace_atomically(&path, |file| {
            file.write_all(b"incomplete replacement")?;
            Err(io::Error::other("injected write failure").into())
        });
        assert!(matches!(result, Err(Error::Io(_))));
        assert_eq!(
            std::fs::read(&path).expect("prior survives"),
            b"prior valid artwork"
        );
        assert_eq!(
            std::fs::read_dir(directory.path()).expect("list").count(),
            1
        );
    }

    #[test]
    fn a_failed_rename_keeps_the_destination_and_removes_staging() {
        let directory = tempfile::tempdir().expect("test directory");
        let target = directory.path().join("already-a-directory");
        std::fs::create_dir(&target).expect("target directory");
        std::fs::write(target.join("sentinel"), b"keep").expect("sentinel");
        let result = replace_atomically(&target, |file| {
            file.write_all(b"replacement")?;
            Ok(())
        });
        assert!(matches!(result, Err(Error::Io(_))));
        assert_eq!(
            std::fs::read(target.join("sentinel")).expect("sentinel"),
            b"keep"
        );
        assert_eq!(
            std::fs::read_dir(directory.path()).expect("list").count(),
            1
        );
    }
}
