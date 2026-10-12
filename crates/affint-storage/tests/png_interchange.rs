//! Independent PNG fixtures, deferred corruption, precision refusal, and export.

use affint_document::PixelSize;
use affint_storage::{Error, PngLimits, SourceColor, load_png, save_png};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn independent_rgb_gray_alpha_and_transparent_color_samples_import_exactly() {
    for (name, expected) in [
        (
            "rgb-2x2.png",
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 17, 29, 43, 255,
            ],
        ),
        (
            "rgba-2x2.png",
            vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 17, 29, 43, 71],
        ),
        (
            "gray-alpha-2x1.png",
            vec![19, 19, 19, 255, 203, 203, 203, 42],
        ),
    ] {
        let imported = load_png(fixture(name), PngLimits::default()).expect("independent source");
        assert_eq!(imported.asset.pixels(), expected);
        assert_eq!(imported.color, SourceColor::AssumedSrgb);
    }
}

#[test]
fn sixteen_bit_sources_and_excess_pixels_are_refused_without_quantization_or_resize() {
    assert!(matches!(
        load_png(fixture("gray16-2x1.png"), PngLimits::default()),
        Err(Error::UnsupportedImage(_))
    ));
    let limits = PngLimits {
        max_pixels: 3,
        ..PngLimits::default()
    };
    assert!(matches!(
        load_png(fixture("rgb-2x2.png"), limits),
        Err(Error::Limit {
            resource: "original PNG pixels",
            actual: 4,
            ..
        })
    ));
}

#[test]
fn pixels_are_not_success_when_the_later_iend_checksum_is_corrupt() {
    let mut bytes = std::fs::read(fixture("rgba-2x2.png")).expect("fixture");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x80;
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("corrupt.png");
    std::fs::write(&path, bytes).expect("corrupt fixture");
    assert!(matches!(
        load_png(path, PngLimits::default()),
        Err(Error::PngDecode(_))
    ));
}

#[test]
fn export_has_exact_dimensions_pixels_and_explicit_srgb_tagging() {
    let source = load_png(fixture("rgba-2x2.png"), PngLimits::default()).expect("source");
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("export.png");
    save_png(&path, source.asset.size(), source.asset.pixels()).expect("export");
    let exported = load_png(&path, PngLimits::default()).expect("tagged PNG");
    assert_eq!(exported.asset.size(), PixelSize::new(2, 2).expect("size"));
    assert_eq!(exported.asset.pixels(), source.asset.pixels());
    assert_eq!(exported.color, SourceColor::DeclaredSrgb);
    let prior = std::fs::read(&path).expect("export bytes");
    assert!(matches!(
        save_png(&path, source.asset.size(), &[0; 3]),
        Err(Error::Document(_))
    ));
    assert_eq!(std::fs::read(path).expect("preserved prior"), prior);
}
