use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    sync::Arc,
};

use affint_document::{
    Archive, AssetId, AssetRecord, LayerRecord, PageRecord, PixelSize, RasterAsset, Session,
    Snapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, FileLimits, check_limit};

const MAGIC: &[u8; 8] = b"AFFINT01";
pub(crate) const HEADER_BYTES: usize = 56;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    document_schema: u32,
    next_id: u64,
    pages: Vec<PageRecord>,
    layers: Vec<LayerRecord>,
    assets: Vec<SourceReference>,
    blobs: Vec<Blob>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceReference {
    id: AssetId,
    blob: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Blob {
    size: PixelSize,
    sha256: [u8; 32],
}

pub(crate) fn encode<W: Write>(
    snapshot: &Snapshot,
    writer: &mut W,
    limits: FileLimits,
) -> Result<(), Error> {
    let archive = snapshot.archive()?;
    let mut unique: Vec<&RasterAsset> = Vec::new();
    let mut by_digest = BTreeMap::new();
    let mut sources = Vec::new();
    let mut blobs = Vec::new();
    let mut source_bytes = 0usize;
    for record in &archive.assets {
        let digest: [u8; 32] = Sha256::digest(record.asset.pixels()).into();
        let key = (
            digest,
            record.asset.size().width(),
            record.asset.size().height(),
        );
        let blob = match by_digest.get(&key).copied() {
            Some(index) => {
                let prior: &RasterAsset = unique[index];
                if prior.size() != record.asset.size() || prior.pixels() != record.asset.pixels() {
                    return Err(Error::Checksum("ambiguous source digest"));
                }
                index
            }
            None => {
                source_bytes = source_bytes
                    .checked_add(record.asset.pixels().len())
                    .ok_or(Error::Overflow)?;
                check_limit(
                    "source bytes",
                    source_bytes as u64,
                    limits.max_source_bytes as u64,
                )?;
                let index = unique.len();
                unique.push(&record.asset);
                blobs.push(Blob {
                    size: record.asset.size(),
                    sha256: digest,
                });
                by_digest.insert(key, index);
                index
            }
        };
        sources.push(SourceReference {
            id: record.id,
            blob,
        });
    }
    let manifest = Manifest {
        format_version: 1,
        document_schema: archive.schema,
        next_id: archive.next_id,
        pages: archive.pages,
        layers: archive.layers,
        assets: sources,
        blobs,
    };
    let mut metadata = BoundedJson {
        bytes: Vec::new(),
        limit: limits.max_manifest_bytes,
    };
    serde_json::to_writer(&mut metadata, &manifest)?;
    let file_length = (HEADER_BYTES as u64)
        .checked_add(metadata.bytes.len() as u64)
        .and_then(|n| n.checked_add(source_bytes as u64))
        .ok_or(Error::Overflow)?;
    check_limit("file bytes", file_length, limits.max_file_bytes)?;
    writer.write_all(MAGIC)?;
    writer.write_all(&(metadata.bytes.len() as u64).to_le_bytes())?;
    writer.write_all(&file_length.to_le_bytes())?;
    writer.write_all(&Sha256::digest(&metadata.bytes))?;
    writer.write_all(&metadata.bytes)?;
    for source in unique {
        writer.write_all(source.pixels())?;
    }
    Ok(())
}

pub(crate) fn decode<R: Read>(
    reader: &mut R,
    length: u64,
    limits: FileLimits,
    document_limits: affint_document::Limits,
) -> Result<Session, Error> {
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(Error::InvalidFormat("unrecognized format version"));
    }
    let manifest_length = read_u64(reader)?;
    let saved_length = read_u64(reader)?;
    let mut saved_digest = [0; 32];
    reader.read_exact(&mut saved_digest)?;
    if saved_length != length {
        return Err(Error::InvalidFormat("file length mismatch"));
    }
    check_limit(
        "manifest bytes",
        manifest_length,
        limits.max_manifest_bytes as u64,
    )?;
    let body_start = (HEADER_BYTES as u64)
        .checked_add(manifest_length)
        .ok_or(Error::Overflow)?;
    if body_start > length {
        return Err(Error::InvalidFormat("manifest exceeds file"));
    }
    let mut metadata = allocate(usize::try_from(manifest_length).map_err(|_| Error::Overflow)?)?;
    reader.read_exact(&mut metadata)?;
    let actual_digest: [u8; 32] = Sha256::digest(&metadata).into();
    if saved_digest != actual_digest {
        return Err(Error::Checksum("manifest"));
    }
    let manifest: Manifest = serde_json::from_slice(&metadata)?;
    drop(metadata);
    if manifest.format_version != 1 || manifest.document_schema != 1 {
        return Err(Error::InvalidFormat("unsupported editable schema"));
    }
    check_limit(
        "pages",
        manifest.pages.len() as u64,
        document_limits.max_pages as u64,
    )?;
    check_limit(
        "layers",
        manifest.layers.len() as u64,
        document_limits.max_layers as u64,
    )?;
    check_limit(
        "assets",
        manifest.assets.len() as u64,
        document_limits.max_assets as u64,
    )?;
    check_limit(
        "blobs",
        manifest.blobs.len() as u64,
        document_limits.max_assets as u64,
    )?;
    let mut referenced = BTreeSet::new();
    for source in &manifest.assets {
        if source.blob >= manifest.blobs.len() {
            return Err(Error::InvalidFormat("missing original source blob"));
        }
        referenced.insert(source.blob);
    }
    if referenced.len() != manifest.blobs.len() {
        return Err(Error::InvalidFormat("unreferenced source blob"));
    }
    let mut source_bytes = 0usize;
    for blob in &manifest.blobs {
        source_bytes = source_bytes
            .checked_add(blob.size.byte_len()?)
            .ok_or(Error::Overflow)?;
    }
    check_limit(
        "source bytes",
        source_bytes as u64,
        limits.max_source_bytes.min(document_limits.max_pixel_bytes) as u64,
    )?;
    if body_start
        .checked_add(source_bytes as u64)
        .ok_or(Error::Overflow)?
        != length
    {
        return Err(Error::InvalidFormat("source lengths disagree with file"));
    }
    let mut originals = Vec::new();
    originals
        .try_reserve_exact(manifest.blobs.len())
        .map_err(|_| Error::Allocation(manifest.blobs.len()))?;
    for blob in manifest.blobs {
        let mut pixels = allocate(blob.size.byte_len()?)?;
        reader.read_exact(&mut pixels)?;
        let digest: [u8; 32] = Sha256::digest(&pixels).into();
        if digest != blob.sha256 {
            return Err(Error::Checksum("original source"));
        }
        originals.push(Arc::new(RasterAsset::new(blob.size, pixels)?));
    }
    let mut trailing = [0; 1];
    if reader.read(&mut trailing)? != 0 {
        return Err(Error::InvalidFormat("trailing bytes"));
    }
    let mut assets = Vec::new();
    assets
        .try_reserve_exact(manifest.assets.len())
        .map_err(|_| Error::Allocation(manifest.assets.len()))?;
    for reference in manifest.assets {
        let asset = originals
            .get(reference.blob)
            .ok_or(Error::InvalidFormat("missing source blob"))?;
        assets.push(AssetRecord {
            id: reference.id,
            asset: Arc::clone(asset),
        });
    }
    Ok(Session::from_archive(
        Archive {
            schema: manifest.document_schema,
            next_id: manifest.next_id,
            pages: manifest.pages,
            assets,
            layers: manifest.layers,
        },
        document_limits,
    )?)
}

fn read_u64(reader: &mut impl Read) -> Result<u64, Error> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn allocate(bytes: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(bytes)
        .map_err(|_| Error::Allocation(bytes))?;
    buffer.resize(bytes, 0);
    Ok(buffer)
}

struct BoundedJson {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("manifest size overflow"))?;
        if length > self.limit {
            return Err(io::Error::other("manifest byte limit exceeded"));
        }
        if length > self.bytes.capacity() {
            // Exact growth would reallocate for every JSON token. Bounded doubling
            // amortizes it without allowing spare capacity to exceed admission.
            let target = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(length)
                .min(self.limit);
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| io::Error::other("manifest allocation failed"))?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
