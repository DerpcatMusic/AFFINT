//! Public-interface scenarios for consumed edit payloads and atomic staging.
//!
//! Owner identity is checked across publication, previews, and history; rejection
//! controls check rollback and irreversible identities. These tests do not measure
//! allocator requests or establish a process RSS cap.

use std::mem::size_of;

use affint_document::{
    Change, Content, Edit, Error, History, LayerSpec, Limits, LiveOperation, NodeId, PageSize,
    Parent, Session, Transform,
};

const METADATA_BYTES: usize = 32 * 1024;
const OVERSIZED_OPERATION_COUNT: usize = 32_768;

fn session(limits: Limits) -> Session {
    Session::new(PageSize::new(8.0, 8.0).unwrap(), limits).unwrap()
}

fn page_owner(session: &Session) -> Parent {
    Parent::Page(session.snapshot().page_order()[0])
}

fn insert_group(session: &mut Session, layer: LayerSpec) -> NodeId {
    let parent = page_owner(session);
    session
        .transact(Edit::new(
            "Insert group",
            [Change::InsertLayer {
                parent,
                index: 0,
                layer,
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn small_session() -> (Session, NodeId) {
    let mut session = session(Limits {
        max_metadata_bytes: METADATA_BYTES,
        ..Limits::default()
    });
    let node = insert_group(&mut session, LayerSpec::new("Group", Content::Group));
    (session, node)
}

fn oversized() -> Vec<LiveOperation> {
    let operations = vec![LiveOperation::Exposure(0.0); OVERSIZED_OPERATION_COUNT];
    assert!(operations.len() * size_of::<LiveOperation>() > METADATA_BYTES);
    operations
}

#[test]
fn submitted_operation_owners_survive_publication_and_history_without_replication() {
    let (mut session, node) = small_session();
    let before = session.snapshot();
    let operations = vec![LiveOperation::Exposure(0.25), LiveOperation::Exposure(-0.5)];
    let owner = operations.as_ptr();
    assert_eq!(operations.capacity(), operations.len());
    session
        .transact(Edit::new(
            "Light",
            [Change::SetOperations { node, operations }],
        ))
        .unwrap();
    let edited = session.snapshot();
    assert_eq!(edited.node(node).unwrap().operations().as_ptr(), owner);
    assert_eq!(
        edited.node(node).unwrap().operations(),
        &[LiveOperation::Exposure(0.25), LiveOperation::Exposure(-0.5),]
    );
    assert!(before.node(node).unwrap().operations().is_empty());
    assert!(
        session
            .history(History::Undo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .is_empty()
    );
    assert_eq!(
        session
            .history(History::Redo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .as_ptr(),
        owner
    );
}

#[test]
fn both_insert_and_replace_layer_specs_transfer_their_operation_owners() {
    let mut session = session(Limits::default());
    let mut inserted = LayerSpec::new("Inserted", Content::Group);
    inserted.operations = vec![LiveOperation::Exposure(0.25)];
    let inserted_owner = inserted.operations.as_ptr();
    let node = insert_group(&mut session, inserted);
    let original = session.snapshot();
    assert_eq!(
        original.node(node).unwrap().operations().as_ptr(),
        inserted_owner
    );

    let mut replacement = LayerSpec::new("Replaced", Content::Group);
    replacement.operations = vec![LiveOperation::Exposure(0.5), LiveOperation::Exposure(-0.75)];
    let replacement_owner = replacement.operations.as_ptr();
    session
        .transact(Edit::new(
            "Replace properties",
            [Change::SetLayer {
                node,
                layer: replacement,
            }],
        ))
        .unwrap();
    assert_eq!(
        session.snapshot().node(node).unwrap().operations().as_ptr(),
        replacement_owner
    );
    assert_eq!(original.node(node).unwrap().name(), "Inserted");
    assert_eq!(
        session
            .history(History::Undo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .as_ptr(),
        inserted_owner
    );
    assert_eq!(
        session
            .history(History::Redo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .as_ptr(),
        replacement_owner
    );
}

#[test]
fn oversized_owned_operation_input_refuses_publication_and_preserves_both_history_sides() {
    let (mut session, node) = small_session();
    session
        .transact(Edit::new(
            "Place",
            [Change::SetTransform {
                node,
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, 2.0, 3.0]).unwrap(),
            }],
        ))
        .unwrap();
    session.history(History::Undo).unwrap();
    let before = session.snapshot();
    let history = (session.history_status().undo, session.history_status().redo);
    // This input owner exists before submission. This scenario verifies refusal
    // and rollback; the absence of a second bulk Vec follows from staging source,
    // not an allocator measurement or a global RSS assertion.
    let edit = Edit::new(
        "Oversized input",
        [Change::SetOperations {
            node,
            operations: oversized(),
        }],
    );
    assert!(matches!(
        session.transact(edit),
        Err(Error::LimitExceeded { .. })
    ));
    assert_eq!(session.snapshot().revision(), before.revision());
    assert!(
        session
            .snapshot()
            .node(node)
            .unwrap()
            .operations()
            .is_empty()
    );
    assert_eq!(
        (session.history_status().undo, session.history_status().redo),
        history
    );
    assert_eq!(
        session
            .history(History::Redo)
            .unwrap()
            .node(node)
            .unwrap()
            .transform()
            .affine()
            .as_coeffs()[4..],
        [2.0, 3.0]
    );
}

#[test]
fn oversized_layer_spec_refusal_keeps_live_allocator_watermarks_irreversible() {
    let (mut session, old) = small_session();
    let before = session.snapshot();
    let watermark = before.archive().unwrap().next_id;
    let parent = page_owner(&session);
    let mut rejected = LayerSpec::new("Rejected", Content::Group);
    rejected.operations = oversized();
    assert!(matches!(
        session.transact(Edit::new(
            "Rejected layer",
            [Change::InsertLayer {
                parent,
                index: 0,
                layer: rejected,
            }]
        )),
        Err(Error::LimitExceeded { .. })
    ));
    assert_eq!(session.snapshot().revision(), before.revision());
    assert_eq!(session.snapshot().layers().count(), 1);
    assert!(
        session
            .snapshot()
            .node(old)
            .unwrap()
            .operations()
            .is_empty()
    );
    assert!(before.archive().unwrap().next_id > watermark);
    let issued_after_failure = before.archive().unwrap().next_id;
    let next = insert_group(&mut session, LayerSpec::new("Accepted", Content::Group));
    assert_eq!(next.get(), issued_after_failure);
    assert!(next.get() > old.get());
}

#[test]
fn remove_then_insert_replacements_fit_count_and_byte_policy_without_summed_payload_refusal() {
    let mut session = session(Limits {
        max_metadata_bytes: METADATA_BYTES,
        max_layers: 1,
        max_operations: 256,
        ..Limits::default()
    });
    let mut original = LayerSpec::new("Original", Content::Group);
    original.operations = vec![LiveOperation::Exposure(0.25); 256];
    let original_owner = original.operations.as_ptr();
    let old = insert_group(&mut session, original);
    let parent = page_owner(&session);
    let mut replacement = LayerSpec::new("Replacement", Content::Group);
    replacement.operations = vec![LiveOperation::Exposure(-0.5); 256];
    let replacement_owner = replacement.operations.as_ptr();
    let created = session
        .transact(Edit::new(
            "Replace layer",
            [
                Change::RemoveLayer { node: old },
                Change::InsertLayer {
                    parent,
                    index: 0,
                    layer: replacement,
                },
            ],
        ))
        .unwrap();
    let new = created.nodes[0];
    assert!(new.get() > old.get());
    assert!(session.snapshot().node(old).is_none());
    assert_eq!(
        session.snapshot().node(new).unwrap().operations().as_ptr(),
        replacement_owner
    );
    assert!(session.resource_usage().unwrap().retained_metadata_bytes <= METADATA_BYTES);
    let undo = session.history(History::Undo).unwrap();
    assert!(undo.node(new).is_none());
    assert_eq!(
        undo.node(old).unwrap().operations().as_ptr(),
        original_owner
    );
    assert_eq!(
        session
            .history(History::Redo)
            .unwrap()
            .node(new)
            .unwrap()
            .operations()
            .as_ptr(),
        replacement_owner
    );
}

#[test]
fn final_state_validation_allows_superseded_owned_payloads_without_copying_or_sum_preflight() {
    let (mut session, node) = small_session();
    let final_operations = vec![LiveOperation::Exposure(0.75)];
    let final_owner = final_operations.as_ptr();
    // The first prebuilt owner exceeds the policy but is superseded before final
    // validation. It moves through staging, is dropped, and is never bulk-cloned.
    session
        .transact(Edit::new(
            "Final replacement",
            [
                Change::SetOperations {
                    node,
                    operations: oversized(),
                },
                Change::SetOperations {
                    node,
                    operations: final_operations,
                },
            ],
        ))
        .unwrap();
    assert_eq!(
        session.snapshot().node(node).unwrap().operations(),
        &[LiveOperation::Exposure(0.75)]
    );
    assert_eq!(
        session.snapshot().node(node).unwrap().operations().as_ptr(),
        final_owner
    );
    assert!(session.resource_usage().unwrap().retained_metadata_bytes <= METADATA_BYTES);
    assert!(
        session
            .history(History::Undo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .is_empty()
    );
}

#[test]
fn small_owned_payloads_with_large_spare_capacity_keep_previous_admission_behavior() {
    let (mut session, node) = small_session();
    let mut operations = Vec::with_capacity(OVERSIZED_OPERATION_COUNT);
    operations.push(LiveOperation::Exposure(0.25));
    assert!(operations.capacity() * size_of::<LiveOperation>() > METADATA_BYTES);
    session
        .transact(Edit::new(
            "Small live payload",
            [Change::SetOperations { node, operations }],
        ))
        .unwrap();
    assert_eq!(
        session.snapshot().node(node).unwrap().operations(),
        &[LiveOperation::Exposure(0.25)]
    );
    assert!(session.resource_usage().unwrap().retained_metadata_bytes <= METADATA_BYTES);
}

#[test]
fn failed_owned_gesture_payload_preserves_preview_owner_and_single_edit_commit() {
    let (mut session, node) = small_session();
    let before = session.snapshot();
    let undo_count = session.history_status().undo;
    session.begin_gesture("Live light").unwrap();
    let operations = vec![LiveOperation::Exposure(0.25), LiveOperation::Exposure(-0.5)];
    let owner = operations.as_ptr();
    let (preview, _) = session
        .update_gesture(Edit::new(
            "Preview",
            [Change::SetOperations { node, operations }],
        ))
        .unwrap();
    assert_eq!(preview.node(node).unwrap().operations().as_ptr(), owner);
    assert!(matches!(
        session.update_gesture(Edit::new(
            "Oversized preview",
            [Change::SetOperations {
                node,
                operations: oversized()
            }]
        )),
        Err(Error::LimitExceeded { .. })
    ));
    assert_eq!(session.snapshot().revision(), before.revision());
    assert_eq!(session.history_status().undo, undo_count);
    session.commit_gesture().unwrap();
    assert_eq!(
        session.snapshot().node(node).unwrap().operations().as_ptr(),
        owner
    );
    assert_eq!(session.history_status().undo, undo_count + 1);
    assert_eq!(session.history_status().undo_label, Some("Live light"));
    assert!(
        session
            .history(History::Undo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .is_empty()
    );
    assert_eq!(
        session
            .history(History::Redo)
            .unwrap()
            .node(node)
            .unwrap()
            .operations()
            .as_ptr(),
        owner
    );
}
