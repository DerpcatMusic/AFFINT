use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use affint_document::{PixelSize, RasterAsset};

use crate::{Durability, Error, check_limit, replace_atomically};

/// Independent PNG input, pixel, and decoder-workspace admission.
#[derive(Debug, Clone, Copy)]
pub struct PngLimits {
    /// Maximum encoded file length. Default: 256 MiB.
    pub max_encoded_bytes: u64,
    /// Maximum original pixel count, never enforced by resizing. Default: 32 MP.
    pub max_pixels: u64,
    /// png crate's internal allocation budget. Default: 64 MiB.
    pub max_decoder_bytes: usize,
}

impl Default for PngLimits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 256 * 1024 * 1024,
            max_pixels: 32_000_000,
            max_decoder_bytes: 64 * 1024 * 1024,
        }
    }
}

/// The interpretation actually established for an imported source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceColor {
    /// The PNG explicitly declares sRGB.
    DeclaredSrgb,
    /// The PNG has no color-space tags; its samples are explicitly assumed sRGB.
    AssumedSrgb,
}

/// An exact original pixel source and its established color interpretation.
#[derive(Debug)]
pub struct ImportedRaster {
    /// Original full-resolution pixels normalized to straight RGBA8.
    pub asset: RasterAsset,
    /// Declared versus assumed sRGB; the UI should preserve this distinction.
    pub color: SourceColor,
}

/// Imports supported PNG samples with full-stream checksum validation.
///
/// Supports 8-bit and expanded lower-bit sources. Refuses 16-bit data, APNG, ICC,
/// EXIF/orientation, HDR/CICP, and explicitly tagged non-sRGB color policies.
/// Untagged samples are returned as explicitly assumed sRGB. No thumbnail fallback
/// or resolution/precision reduction occurs. Encoded metadata is not retained.
pub fn load_png(path: impl AsRef<Path>, limits: PngLimits) -> Result<ImportedRaster, Error> {
    if limits.max_pixels == 0 || limits.max_decoder_bytes == 0 || limits.max_encoded_bytes == 0 {
        return Err(Error::InvalidFormat("zero PNG limits"));
    }
    let file = File::open(path)?;
    check_limit(
        "encoded PNG bytes",
        file.metadata()?.len(),
        limits.max_encoded_bytes,
    )?;
    let mut decoder = png::Decoder::new_with_limits(
        BufReader::new(file),
        png::Limits {
            bytes: limits.max_decoder_bytes,
        },
    );
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    check_info(reader.info())?;
    let size = PixelSize::new(reader.info().width, reader.info().height)?;
    let pixels = u64::from(size.width())
        .checked_mul(u64::from(size.height()))
        .ok_or(Error::Overflow)?;
    check_limit("original PNG pixels", pixels, limits.max_pixels)?;
    let rgba_length = size.byte_len()?;
    let decoded_length = reader.output_buffer_size().ok_or(Error::Overflow)?;
    if decoded_length > rgba_length {
        return Err(Error::UnsupportedImage("decoded precision exceeds RGBA8"));
    }
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(rgba_length)
        .map_err(|_| Error::Allocation(rgba_length))?;
    buffer.resize(decoded_length, 0);
    let output = reader.next_frame(&mut buffer)?;
    if output.bit_depth != png::BitDepth::Eight
        || output.width != size.width()
        || output.height != size.height()
    {
        return Err(Error::UnsupportedImage(
            "non-RGBA8 frame dimensions or precision",
        ));
    }
    // IDAT pixels alone are insufficient: validate deferred chunks/checksums/IEND.
    reader.finish()?;
    check_info(reader.info())?;
    let color = if reader.info().srgb.is_some() {
        SourceColor::DeclaredSrgb
    } else {
        SourceColor::AssumedSrgb
    };
    let channels = match output.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return Err(Error::UnsupportedImage("palette was not expanded")),
    };
    let pixel_count = rgba_length / 4;
    if output.buffer_size() != pixel_count.checked_mul(channels).ok_or(Error::Overflow)? {
        return Err(Error::InvalidFormat("PNG output length mismatch"));
    }
    buffer.truncate(output.buffer_size());
    buffer.resize(rgba_length, 0);
    // Expand backwards within the already-admitted allocation; no second image.
    if channels != 4 {
        for index in (0..pixel_count).rev() {
            let source = index * channels;
            let rgba = match channels {
                3 => [buffer[source], buffer[source + 1], buffer[source + 2], 255],
                2 => [
                    buffer[source],
                    buffer[source],
                    buffer[source],
                    buffer[source + 1],
                ],
                1 => [buffer[source], buffer[source], buffer[source], 255],
                _ => return Err(Error::UnsupportedImage("PNG channel count")),
            };
            buffer[index * 4..index * 4 + 4].copy_from_slice(&rgba);
        }
    }
    Ok(ImportedRaster {
        asset: RasterAsset::new(size, buffer)?,
        color,
    })
}

fn check_info(info: &png::Info<'_>) -> Result<(), Error> {
    if info.bit_depth == png::BitDepth::Sixteen {
        return Err(Error::UnsupportedImage(
            "16-bit originals need a preserved high-precision asset",
        ));
    }
    if info.animation_control.is_some() || info.frame_control.is_some() {
        return Err(Error::UnsupportedImage(
            "animated PNG requires frame-aware import",
        ));
    }
    if info.icc_profile.is_some() {
        return Err(Error::UnsupportedImage(
            "ICC conversion and source-profile retention are required",
        ));
    }
    if info.exif_metadata.is_some() {
        return Err(Error::UnsupportedImage(
            "EXIF orientation and metadata retention are required",
        ));
    }
    if info.coding_independent_code_points.is_some()
        || info.mastering_display_color_volume.is_some()
        || info.content_light_level.is_some()
    {
        return Err(Error::UnsupportedImage(
            "CICP/HDR source interpretation is required",
        ));
    }
    if info.srgb.is_none() && (info.gama_chunk.is_some() || info.chrm_chunk.is_some()) {
        return Err(Error::UnsupportedImage(
            "tagged gamma/chromaticities need color conversion",
        ));
    }
    Ok(())
}

/// Atomically exports exactly sized straight RGBA8 sRGB pixels as a tagged PNG.
///
/// The caller keeps its render frame/budget lease alive while this borrowed buffer
/// is encoded. PNG writer finalization is checked before file publication.
pub fn save_png(
    path: impl AsRef<Path>,
    size: PixelSize,
    rgba8: &[u8],
) -> Result<Durability, Error> {
    let expected = size.byte_len()?;
    if rgba8.len() != expected {
        return Err(affint_document::Error::PixelLength {
            expected,
            actual: rgba8.len(),
        }
        .into());
    }
    replace_atomically(path.as_ref(), |file| {
        let mut buffered = BufWriter::new(file);
        {
            let mut encoder = png::Encoder::new(&mut buffered, size.width(), size.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(rgba8)?;
            writer.finish()?;
        }
        buffered.flush()?;
        Ok(())
    })
}
