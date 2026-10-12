//! Native geometry strictness and bit-exact editable-parameter persistence.
//!
//! Independent public-interface oracles from the Pi core quality review.

use std::io::Cursor;

use affint_document::{
    Change, Content, Edit, LayerSpec, Limits, LiveOperation, PageSize, Parent, Session,
};
use affint_storage::{Error, FileLimits, Store};
use kurbo::Rect;
use sha2::{Digest, Sha256};

fn policy() -> Store {
    Store::new(FileLimits::default(), Limits::default()).expect("ordinary policy")
}

fn encode(session: &Session) -> Vec<u8> {
    let mut bytes = Vec::new();
    policy()
        .encode(&session.snapshot(), &mut bytes)
        .expect("supported source fixture");
    bytes
}

fn decode(bytes: Vec<u8>) -> Result<Session, Error> {
    let length = bytes.len() as u64;
    policy().decode(&mut Cursor::new(bytes), length)
}

// Independently rebuild the framing/checksum after a metadata mutation. This
// ensures refusal cannot be attributed to an unchanged checksum or wrong length.
fn replace_manifest(original: &[u8], manifest: &serde_json::Value) -> Vec<u8> {
    assert!(original.len() >= 56, "fixture header");
    let old_length = u64::from_le_bytes(original[8..16].try_into().expect("length field"));
    let body_start = 56usize
        .checked_add(usize::try_from(old_length).expect("bounded fixture"))
        .expect("bounded offset");
    let body = &original[body_start..];
    let metadata = serde_json::to_vec(manifest).expect("fixture JSON");
    let total = 56usize
        .checked_add(metadata.len())
        .and_then(|length| length.checked_add(body.len()))
        .expect("small fixture total");
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(b"AFFINT01");
    bytes.extend_from_slice(&(metadata.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    bytes.extend_from_slice(&Sha256::digest(&metadata));
    bytes.extend_from_slice(&metadata);
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn native_crop_refuses_a_checksummed_unknown_nested_geometry_member() {
    let mut session =
        Session::new(PageSize::new(4.0, 4.0).expect("page"), Limits::default()).expect("session");
    let parent = Parent::Page(session.snapshot().page_order()[0]);
    let mut layer = LayerSpec::new("Crop fixture", Content::Group);
    layer.operations = vec![LiveOperation::Crop(Rect::new(0.0, 0.0, 2.0, 2.0))];
    session
        .transact(Edit::new(
            "Insert supported crop",
            [Change::InsertLayer {
                parent,
                index: 0,
                layer,
            }],
        ))
        .expect("supported document");
    let original = encode(&session);
    decode(original.clone()).expect("unaltered fixture must load");
    let length = u64::from_le_bytes(original[8..16].try_into().expect("manifest length"));
    let length = usize::try_from(length).expect("small manifest");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&original[56..56 + length]).expect("manifest");
    let crop = manifest["layers"][0]["operations"][0]["Crop"]
        .as_object_mut()
        .expect("probe requires object-shaped serialized Kurbo Rect");
    crop.insert(
        "future_crop_rule".to_owned(),
        serde_json::json!({"retain_or_refuse": true}),
    );
    let mutated = replace_manifest(&original, &manifest);
    let result = decode(mutated);
    assert!(
        matches!(&result, Err(Error::Json(_)) | Err(Error::InvalidFormat(_))),
        "unknown nested geometry must be refused, not silently lost: {result:?}"
    );
}

#[test]
fn native_page_dimensions_preserve_exact_finite_f64_bits() {
    // Fixed xorshift seed and a bounded 4,096-case corpus. All generated values
    // lie in [1, 2), avoiding NaN, infinity, signed-zero policy, and underflow.
    // This is a fidelity oracle, not a benchmark. Stop at the first mismatch and
    // retain its input bits/JSON as the minimal independent regression fixture.
    let mut seed = 0x4f6e_6553_6176_6552_u64;
    let store = policy();
    for case in 0..4_096 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let input_bits = (0x3ff_u64 << 52) | (seed & ((1_u64 << 52) - 1));
        let width = f64::from_bits(input_bits);
        let session = Session::new(
            PageSize::new(width, 2.0).expect("positive finite width"),
            Limits::default(),
        )
        .expect("session");
        let mut bytes = Vec::new();
        store
            .encode(&session.snapshot(), &mut bytes)
            .expect("encode");
        let manifest_length = u64::from_le_bytes(bytes[8..16].try_into().expect("length field"));
        let manifest_length = usize::try_from(manifest_length).expect("small fixture");
        let manifest_text = std::str::from_utf8(&bytes[56..56 + manifest_length])
            .expect("UTF-8 manifest")
            .to_owned();
        let length = bytes.len() as u64;
        let restored = store
            .decode(&mut Cursor::new(bytes), length)
            .expect("valid native stream");
        let snapshot = restored.snapshot();
        let actual = snapshot
            .page(snapshot.page_order()[0])
            .expect("restored page")
            .size()
            .width();
        assert_eq!(
            actual.to_bits(),
            input_bits,
            "case={case}, original_bits={input_bits:#018x}, width={width:?}, manifest={manifest_text}"
        );
    }
}
