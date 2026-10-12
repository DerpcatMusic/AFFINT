//! Corruption, strict admission, exact source persistence, and saved revisions.

use std::{io::Cursor, sync::Arc};

use affint_document::{
    Change, Content, Edit, LayerSpec, Limits, LiveOperation, Mask, MaskMode, PageSize, Parent,
    PixelSize, RasterAsset, Session, Transform,
};
use affint_storage::{Error, FileLimits, Store};
use kurbo::Rect;

fn store() -> Store {
    Store::new(FileLimits::default(), Limits::default()).expect("default policy")
}

fn fixture() -> Session {
    let mut session =
        Session::new(PageSize::new(3., 2.).expect("size"), Limits::default()).expect("session");
    let page = session.snapshot().page_order()[0];
    let original = Arc::new(
        RasterAsset::new(
            PixelSize::new(3, 2).expect("pixels"),
            vec![
                255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 41, 72, 93, 255, 0, 0, 0, 0, 255,
                255, 255, 255,
            ],
        )
        .expect("source"),
    );
    let created = session
        .transact(Edit::new(
            "Import original",
            [Change::InsertRaster {
                parent: Parent::Page(page),
                index: 0,
                name: "Fine detail".into(),
                asset: original,
                transform: Transform::IDENTITY,
            }],
        ))
        .expect("insert");
    let alias = session
        .transact(Edit::new(
            "Shared source",
            [Change::InsertLayer {
                parent: Parent::Page(page),
                index: 1,
                layer: LayerSpec::new(
                    "Source mask",
                    Content::Raster {
                        asset: created.assets[0],
                    },
                ),
            }],
        ))
        .expect("alias");
    let mut hidden = LayerSpec::new(
        "Source mask",
        Content::Raster {
            asset: created.assets[0],
        },
    );
    hidden.visible = false;
    session
        .transact(Edit::new(
            "Editable live workflow",
            [
                Change::SetOperations {
                    node: created.nodes[0],
                    operations: vec![
                        LiveOperation::Exposure(2.5),
                        LiveOperation::Crop(Rect::new(0.25, 0., 2.75, 2.)),
                    ],
                },
                Change::SetLayer {
                    node: alias.nodes[0],
                    layer: hidden,
                },
                Change::SetMask {
                    node: created.nodes[0],
                    mask: Some(Mask {
                        source: alias.nodes[0],
                        mode: MaskMode::Luminance,
                        inverted: true,
                    }),
                },
            ],
        ))
        .expect("live operations");
    session
}

fn encoded(session: &Session) -> Vec<u8> {
    let mut bytes = Vec::new();
    store()
        .encode(&session.snapshot(), &mut bytes)
        .expect("encode");
    bytes
}

fn decode(bytes: Vec<u8>) -> Result<Session, Error> {
    let length = bytes.len() as u64;
    store().decode(&mut Cursor::new(bytes), length)
}

#[test]
fn editable_operations_relationships_and_original_pixels_survive_native_persistence() {
    let session = fixture();
    let original = serde_json::to_value(session.snapshot().archive().expect("archive"))
        .expect("original archive");
    let reopened = decode(encoded(&session)).expect("valid native archive");
    let actual = serde_json::to_value(reopened.snapshot().archive().expect("archive"))
        .expect("reopened archive");
    assert_eq!(actual, original);
    assert_ne!(
        reopened.snapshot().revision(),
        session.snapshot().revision()
    );
    assert_eq!(
        reopened
            .snapshot()
            .assets()
            .next()
            .expect("source")
            .1
            .pixels()[4..8],
        [0, 255, 0, 128]
    );
}

#[test]
fn shared_pixels_under_multiple_asset_identities_are_saved_and_loaded_once() {
    let mut session = fixture();
    let snapshot = session.snapshot();
    let (_, asset) = snapshot.assets().next().expect("source");
    let source =
        Arc::new(RasterAsset::new(asset.size(), asset.pixels().to_vec()).expect("same content"));
    session
        .transact(Edit::new(
            "Second source identity",
            [Change::AddAsset { asset: source }],
        ))
        .expect("second identity");
    let policy = Store::new(
        FileLimits {
            max_source_bytes: 24,
            ..FileLimits::default()
        },
        Limits {
            max_pixel_bytes: 24,
            ..Limits::default()
        },
    )
    .expect("tight policy");
    let mut bytes = Vec::new();
    policy
        .encode(&session.snapshot(), &mut bytes)
        .expect("deduplicated save");
    let length = bytes.len() as u64;
    let reopened = policy
        .decode(&mut Cursor::new(bytes), length)
        .expect("deduplicated load");
    let snapshot = reopened.snapshot();
    let pointers: Vec<_> = snapshot
        .assets()
        .map(|(_, a)| a.pixels().as_ptr())
        .collect();
    assert_eq!(pointers.len(), 2);
    assert_eq!(pointers[0], pointers[1]);
}

#[test]
fn identical_bytes_with_different_image_dimensions_keep_distinct_geometry() {
    let mut session = fixture();
    let snapshot = session.snapshot();
    let (_, asset) = snapshot.assets().next().expect("source");
    let reinterpreted = Arc::new(
        RasterAsset::new(
            PixelSize::new(2, 3).expect("dimensions"),
            asset.pixels().to_vec(),
        )
        .expect("equal length"),
    );
    session
        .transact(Edit::new(
            "Different dimensions",
            [Change::AddAsset {
                asset: reinterpreted,
            }],
        ))
        .expect("insert");
    let reopened = decode(encoded(&session)).expect("not a digest collision");
    let sizes: Vec<_> = reopened
        .snapshot()
        .assets()
        .map(|(_, a)| a.size())
        .collect();
    assert_ne!(sizes[0], sizes[1]);
}

#[test]
fn pixel_corruption_manifest_corruption_truncation_and_appends_are_refused() {
    let bytes = encoded(&fixture());
    let mut corrupt_pixel = bytes.clone();
    let end = corrupt_pixel.len() - 1;
    corrupt_pixel[end] ^= 0x80;
    assert!(matches!(
        decode(corrupt_pixel),
        Err(Error::Checksum("original source"))
    ));
    let mut corrupt_manifest = bytes.clone();
    corrupt_manifest[60] ^= 0x01;
    assert!(matches!(
        decode(corrupt_manifest),
        Err(Error::Checksum("manifest"))
    ));
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(matches!(
        decode(truncated),
        Err(Error::InvalidFormat("file length mismatch"))
    ));
    let mut appended = bytes;
    appended.push(0);
    assert!(matches!(
        decode(appended),
        Err(Error::InvalidFormat("file length mismatch"))
    ));
}

#[test]
fn forged_giant_manifest_is_refused_before_allocating_or_decoding_it() {
    let mut bytes = encoded(&fixture());
    bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        decode(bytes),
        Err(Error::Limit {
            resource: "manifest bytes",
            ..
        })
    ));
}

#[test]
fn unsupported_formats_and_strict_manifest_fields_are_refused() {
    let bytes = encoded(&fixture());
    let mut unknown = bytes.clone();
    unknown[7] = b'2';
    assert!(matches!(decode(unknown), Err(Error::InvalidFormat(_))));
    let length = u64::from_le_bytes(bytes[8..16].try_into().expect("manifest length")) as usize;
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&bytes[56..56 + length]).expect("manifest");
    manifest
        .as_object_mut()
        .expect("object")
        .insert("future_feature".into(), true.into());
    let replacement = serde_json::to_vec(&manifest).expect("metadata");
    let mut changed = Vec::new();
    changed.extend_from_slice(&bytes[..8]);
    changed.extend_from_slice(&(replacement.len() as u64).to_le_bytes());
    changed.extend_from_slice(
        &((56 + replacement.len() + bytes.len() - 56 - length) as u64).to_le_bytes(),
    );
    use sha2::{Digest, Sha256};
    changed.extend_from_slice(&Sha256::digest(&replacement));
    changed.extend_from_slice(&replacement);
    changed.extend_from_slice(&bytes[56 + length..]);
    assert!(matches!(decode(changed), Err(Error::Json(_))));
}

#[test]
fn a_completed_background_save_receipt_cannot_mark_newer_edits_clean() {
    let mut session = fixture();
    let saved = session.snapshot();
    let page = saved.page_order()[0];
    session
        .transact(Edit::new(
            "New artwork",
            [Change::SetPage {
                page,
                name: "New page name".into(),
                size: PageSize::new(3., 2.).expect("size"),
            }],
        ))
        .expect("new edit");
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("editorial.affint");
    let receipt = store()
        .save(&saved, &path)
        .expect("publish original revision");
    assert_eq!(receipt.revision, saved.revision());
    assert_ne!(receipt.revision, session.snapshot().revision());
    let reopened = store().load(&path).expect("reopen");
    assert_eq!(
        reopened.snapshot().page(page).expect("page").name(),
        "Page 1"
    );
}

#[test]
fn a_rejected_save_budget_preserves_a_previously_valid_file() {
    let session = fixture();
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("editorial.affint");
    store()
        .save(&session.snapshot(), &path)
        .expect("first save");
    let original = std::fs::read(&path).expect("first bytes");
    let tight = Store::new(
        FileLimits {
            max_source_bytes: 1,
            ..FileLimits::default()
        },
        Limits::default(),
    )
    .expect("tight policy");
    assert!(matches!(
        tight.save(&session.snapshot(), &path),
        Err(Error::Limit { .. })
    ));
    assert_eq!(std::fs::read(&path).expect("preserved"), original);
    store().load(&path).expect("preserved valid file");
}
