//! Atomic edits, reversible artwork, and validated archive scenarios.

use std::sync::Arc;

use affint_document::{
    Change, Color, Content, Edit, Error, History, LayerSpec, Limits, LiveOperation, Mask, MaskMode,
    NodeId, PageSize, Parent, PixelSize, RasterAsset, Session, Snapshot, Transform, VectorContent,
};
use kurbo::{BezPath, Rect};
use proptest::prelude::*;

fn session() -> Session {
    Session::new(PageSize::new(256.0, 128.0).unwrap(), Limits::default()).unwrap()
}

fn page_owner(session: &Session) -> Parent {
    Parent::Page(session.snapshot().page_order()[0])
}

fn group(session: &mut Session, parent: Parent, name: &str) -> NodeId {
    session
        .transact(Edit::new(
            "Group",
            [Change::InsertLayer {
                parent,
                index: 0,
                layer: LayerSpec::new(name, Content::Group),
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn artwork(snapshot: &Snapshot) -> serde_json::Value {
    let mut value = serde_json::to_value(snapshot.archive().unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("next_id");
    value
}

fn raster() -> Arc<RasterAsset> {
    Arc::new(
        RasterAsset::new(
            PixelSize::new(2, 2).unwrap(),
            vec![
                255, 0, 17, 255, 0, 251, 128, 129, 73, 15, 200, 1, 99, 104, 0, 0,
            ],
        )
        .unwrap(),
    )
}

#[test]
fn moving_nested_artwork_preserves_identity_and_history_restores_exact_order() {
    let mut session = session();
    let page = page_owner(&session);
    let outer = group(&mut session, page, "Outer");
    let inner = group(&mut session, Parent::Layer(outer), "Inner");
    let sibling = group(&mut session, page, "Sibling");
    let before = session.snapshot();
    session
        .transact(Edit::new(
            "Move subtree",
            [Change::MoveLayer {
                node: inner,
                parent: Parent::Layer(sibling),
                index: 0,
            }],
        ))
        .unwrap();
    let moved = session.snapshot();
    assert_eq!(moved.node(inner).unwrap().parent(), Parent::Layer(sibling));
    assert!(moved.node(outer).unwrap().children().is_empty());
    assert_eq!(moved.node(sibling).unwrap().children(), &[inner]);
    assert_eq!(
        artwork(&session.history(History::Undo).unwrap()),
        artwork(&before)
    );
    assert_eq!(
        artwork(&session.history(History::Redo).unwrap()),
        artwork(&moved)
    );
    assert_eq!(before.node(inner).unwrap().parent(), Parent::Layer(outer));
}

#[test]
fn a_rejected_batch_leaves_artwork_revision_and_redo_intact() {
    let mut session = session();
    let page = page_owner(&session);
    let node = group(&mut session, page, "Original");
    session
        .transact(Edit::new(
            "Place",
            [Change::SetTransform {
                node,
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, 12.0, 24.0]).unwrap(),
            }],
        ))
        .unwrap();
    session.history(History::Undo).unwrap();
    let before = session.snapshot();
    let counts = (session.history_status().undo, session.history_status().redo);
    let mut invalid = LayerSpec::new("Invalid", Content::Group);
    invalid.opacity = f64::NAN;
    assert!(matches!(
        session.transact(Edit::new(
            "Broken batch",
            [
                Change::SetTransform {
                    node,
                    transform: Transform::new([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]).unwrap()
                },
                Change::SetLayer {
                    node,
                    layer: invalid
                },
            ]
        )),
        Err(Error::InvalidValue("layer opacity"))
    ));
    assert_eq!(session.snapshot().revision(), before.revision());
    assert_eq!(artwork(&session.snapshot()), artwork(&before));
    assert_eq!(
        (session.history_status().undo, session.history_status().redo),
        counts
    );
    assert_eq!(
        session
            .history(History::Redo)
            .unwrap()
            .node(node)
            .unwrap()
            .transform()
            .affine()
            .as_coeffs()[4],
        12.0
    );
}

#[test]
fn reparenting_an_ancestor_under_its_descendant_is_rejected_atomically() {
    let mut session = session();
    let page = page_owner(&session);
    let outer = group(&mut session, page, "Outer");
    let inner = group(&mut session, Parent::Layer(outer), "Inner");
    let before = session.snapshot();
    assert_eq!(
        session.transact(Edit::new(
            "Cycle",
            [Change::MoveLayer {
                node: outer,
                parent: Parent::Layer(inner),
                index: 0,
            }]
        )),
        Err(Error::Cycle)
    );
    assert_eq!(artwork(&session.snapshot()), artwork(&before));
    assert_eq!(session.snapshot().revision(), before.revision());
}

#[test]
fn combined_parent_and_mask_dependency_cycles_are_rejected() {
    let mut session = session();
    let page = page_owner(&session);
    let outer = group(&mut session, page, "Outer");
    let inner = group(&mut session, Parent::Layer(outer), "Inner");
    let mask = |source| Mask {
        source,
        mode: MaskMode::Alpha,
        inverted: false,
    };
    assert_eq!(
        session.transact(Edit::new(
            "Ancestor mask",
            [Change::SetMask {
                node: inner,
                mask: Some(mask(outer)),
            }]
        )),
        Err(Error::Cycle)
    );
    let independent = group(&mut session, page, "Independent");
    assert_eq!(
        session.transact(Edit::new(
            "Mutual mask",
            [
                Change::SetMask {
                    node: inner,
                    mask: Some(mask(independent))
                },
                Change::SetMask {
                    node: independent,
                    mask: Some(mask(inner))
                },
            ]
        )),
        Err(Error::Cycle)
    );
    session
        .transact(Edit::new(
            "Descendant mask",
            [Change::SetMask {
                node: outer,
                mask: Some(mask(inner)),
            }],
        ))
        .unwrap();
    assert_eq!(
        session.snapshot().node(outer).unwrap().mask(),
        Some(mask(inner))
    );
}

#[test]
fn removing_a_mask_source_requires_repointing_the_dependency_in_the_same_batch() {
    let mut session = session();
    let page = page_owner(&session);
    let target = group(&mut session, page, "Target");
    let source = group(&mut session, page, "Coverage");
    session
        .transact(Edit::new(
            "Mask",
            [Change::SetMask {
                node: target,
                mask: Some(Mask {
                    source,
                    mode: MaskMode::Luminance,
                    inverted: true,
                }),
            }],
        ))
        .unwrap();
    let before = session.snapshot();
    assert!(
        matches!(session.transact(Edit::new("Delete source", [Change::RemoveLayer { node: source }])),
        Err(Error::LayerInUse { target: removed, user }) if removed == source && user == target)
    );
    assert_eq!(artwork(&session.snapshot()), artwork(&before));
    session
        .transact(Edit::new(
            "Remove mask",
            [
                Change::RemoveLayer { node: source },
                Change::SetMask {
                    node: target,
                    mask: None,
                },
            ],
        ))
        .unwrap();
    assert!(session.snapshot().node(source).is_none());
    assert!(session.snapshot().node(target).unwrap().mask().is_none());
    assert_eq!(
        artwork(&session.history(History::Undo).unwrap()),
        artwork(&before)
    );
}

#[test]
fn source_pixels_keep_their_exact_allocation_through_live_edits_history_and_archive() {
    let mut session = session();
    let page = page_owner(&session);
    let source = raster();
    let bytes = source.pixels().as_ptr();
    let expected = source.pixels().to_vec();
    let created = session
        .transact(Edit::new(
            "Import source",
            [Change::InsertRaster {
                parent: page,
                index: 0,
                name: "Photo".to_owned(),
                asset: Arc::clone(&source),
                transform: Transform::IDENTITY,
            }],
        ))
        .unwrap();
    let node = created.nodes[0];
    let asset = created.assets[0];
    let original = session.snapshot();
    session
        .transact(Edit::new(
            "Live crop and light",
            [
                Change::SetOperations {
                    node,
                    operations: vec![
                        LiveOperation::Crop(Rect::new(0.25, 0.0, 1.75, 2.0)),
                        LiveOperation::Exposure(1.5),
                    ],
                },
                Change::SetTransform {
                    node,
                    transform: Transform::new([0.0, 1.0, -1.0, 0.0, 3.0, 4.0]).unwrap(),
                },
            ],
        ))
        .unwrap();
    for snapshot in [
        original,
        session.snapshot(),
        session.history(History::Undo).unwrap(),
        session.history(History::Redo).unwrap(),
    ] {
        let stored = snapshot.asset(asset).unwrap();
        assert_eq!(stored.size(), PixelSize::new(2, 2).unwrap());
        assert_eq!(stored.pixels(), expected);
        assert_eq!(stored.pixels().as_ptr(), bytes);
        assert_eq!(
            snapshot.archive().unwrap().assets[0]
                .asset
                .pixels()
                .as_ptr(),
            bytes
        );
    }
    assert_eq!(
        session.resource_usage().unwrap().pixel_bytes,
        expected.len()
    );
}

#[test]
fn identities_survive_branching_and_serialization_of_an_older_snapshot() {
    let mut session = session();
    let page = page_owner(&session);
    let old = session.snapshot();
    let first = group(&mut session, page, "First branch");
    session.history(History::Undo).unwrap();
    let second = group(&mut session, page, "Second branch");
    assert!(second.get() > first.get());
    assert_eq!(session.history_status().redo, 0);
    let mut restored = Session::from_archive(old.archive().unwrap(), Limits::default()).unwrap();
    let restored_page = page_owner(&restored);
    let third = group(&mut restored, restored_page, "Reopened old artwork");
    assert!(third.get() > second.get());
}

#[test]
fn gestures_publish_unique_previews_then_commit_one_edit_or_cancel_without_id_reuse() {
    let mut session = session();
    let page = page_owner(&session);
    let node = group(&mut session, page, "Placed");
    let base = session.snapshot();
    let undo_count = session.history_status().undo;
    session.begin_gesture("Drag").unwrap();
    let (first, _) = session
        .update_gesture(Edit::new(
            "Move",
            [Change::SetTransform {
                node,
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, 4.0, 0.0]).unwrap(),
            }],
        ))
        .unwrap();
    let (last, _) = session
        .update_gesture(Edit::new(
            "Move",
            [Change::SetTransform {
                node,
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, 7.0, 0.0]).unwrap(),
            }],
        ))
        .unwrap();
    assert_ne!(base.revision(), first.revision());
    assert_ne!(first.revision(), last.revision());
    assert_eq!(session.snapshot().revision(), base.revision());
    assert_eq!(
        session.history(History::Undo).unwrap_err(),
        Error::GestureActive
    );
    session.commit_gesture().unwrap();
    assert_ne!(session.snapshot().revision(), last.revision());
    assert_eq!(session.history_status().undo, undo_count + 1);
    assert_eq!(session.history_status().undo_label, Some("Drag"));
    assert_eq!(
        artwork(&session.history(History::Undo).unwrap()),
        artwork(&base)
    );
    let before_cancel = session.snapshot();
    let history = (session.history_status().undo, session.history_status().redo);
    session.begin_gesture("Temporary group").unwrap();
    let (preview, created) = session
        .update_gesture(Edit::new(
            "Insert",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Temporary", Content::Group),
            }],
        ))
        .unwrap();
    let canceled = created.nodes[0];
    session.cancel_gesture().unwrap();
    assert_eq!(artwork(&session.snapshot()), artwork(&before_cancel));
    assert_eq!(session.snapshot().revision(), before_cancel.revision());
    assert_eq!(
        (session.history_status().undo, session.history_status().redo),
        history
    );
    assert!(preview.node(canceled).is_some());
    assert!(!session.is_current(preview.revision()));
    let next = group(&mut session, page, "Final");
    assert!(next.get() > canceled.get());
}

#[test]
fn a_failed_gesture_update_keeps_the_last_valid_preview_and_final_undo_target() {
    let mut session = session();
    let page = page_owner(&session);
    let node = group(&mut session, page, "Group");
    let base = session.snapshot();
    session.begin_gesture("Gesture").unwrap();
    let (preview, _) = session
        .update_gesture(Edit::new(
            "Place",
            [Change::SetTransform {
                node,
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, 5.0, 0.0]).unwrap(),
            }],
        ))
        .unwrap();
    assert_eq!(
        session
            .update_gesture(Edit::new(
                "Cycle",
                [Change::SetMask {
                    node,
                    mask: Some(Mask {
                        source: node,
                        mode: MaskMode::Alpha,
                        inverted: false
                    }),
                }]
            ))
            .unwrap_err(),
        Error::Cycle
    );
    session.commit_gesture().unwrap();
    assert_eq!(artwork(&session.snapshot()), artwork(&preview));
    assert_eq!(
        artwork(&session.history(History::Undo).unwrap()),
        artwork(&base)
    );
}

#[test]
fn import_refuses_duplicate_ids_stale_watermarks_nonfinite_opacity_and_unknown_fields() {
    let mut session = session();
    let page = page_owner(&session);
    let node = group(&mut session, page, "Group");
    let valid = session.snapshot().archive().unwrap();
    let mut duplicate = valid.clone();
    duplicate.layers.push(duplicate.layers[0].clone());
    assert!(matches!(
        Session::from_archive(duplicate, Limits::default()),
        Err(Error::InvalidIdentity(_))
    ));
    let mut stale = valid.clone();
    stale.next_id = node.get();
    assert_eq!(
        Session::from_archive(stale, Limits::default()).unwrap_err(),
        Error::InvalidIdentity(node.get())
    );
    let mut nonfinite = valid.clone();
    nonfinite.layers[0].opacity = f64::INFINITY;
    assert_eq!(
        Session::from_archive(nonfinite, Limits::default()).unwrap_err(),
        Error::InvalidValue("layer opacity")
    );
    let mut json = serde_json::to_value(valid).unwrap();
    json.as_object_mut().unwrap().insert(
        "unsupported_text".to_owned(),
        serde_json::json!({"contents":"retain me"}),
    );
    assert!(serde_json::from_value::<affint_document::Archive>(json).is_err());
}

#[test]
fn composed_transform_overflow_is_rejected_even_when_both_local_transforms_are_finite() {
    let mut session = session();
    let page = page_owner(&session);
    let parent = group(&mut session, page, "Parent");
    let child = group(&mut session, Parent::Layer(parent), "Child");
    let huge = Transform::new([1.0e150, 0.0, 0.0, 1.0e150, 0.0, 0.0]).unwrap();
    let before = session.snapshot();
    assert_eq!(
        session.transact(Edit::new(
            "Overflow world transform",
            [
                Change::SetTransform {
                    node: parent,
                    transform: huge
                },
                Change::SetTransform {
                    node: child,
                    transform: huge
                },
            ]
        )),
        Err(Error::InvalidValue("affine transform"))
    );
    assert_eq!(artwork(&session.snapshot()), artwork(&before));
}

#[test]
fn finite_dimensions_and_geometry_are_checked_before_becoming_content() {
    assert!(PageSize::new(f64::NAN, 12.0).is_err());
    assert!(PixelSize::new(0, 12).is_err());
    assert!(Transform::new([0.0; 6]).is_err());
    assert!(Color::new(0.0, f64::INFINITY, 0.0, 1.0).is_err());
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((f64::NAN, 1.0));
    assert!(VectorContent::new(path, Color::new(1.0, 0.0, 0.0, 1.0).unwrap()).is_err());
    assert!(RasterAsset::new(PixelSize::new(2, 2).unwrap(), vec![0; 15]).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn reordering_matches_a_plain_order_model_and_undo_restores_each_step(moves in prop::collection::vec((0usize..6, 0usize..6), 1..40)) {
        let mut session = session();
        let page = page_owner(&session);
        let ids: Vec<NodeId> = (0..6).map(|index| group(&mut session, page, &index.to_string())).collect();
        let mut order: Vec<NodeId> = ids.iter().copied().rev().collect();
        let mut expected = vec![order.clone()];
        for (from, to) in moves {
            let id = order.remove(from);
            order.insert(to, id);
            session.transact(Edit::new("Reorder", [Change::MoveLayer { node: id, parent: page, index: to }])).unwrap();
            let snapshot = session.snapshot();
            prop_assert_eq!(snapshot.page(snapshot.page_order()[0]).unwrap().roots(), order.as_slice());
            expected.push(order.clone());
        }
        expected.pop();
        for order in expected.into_iter().rev() {
            let snapshot = session.history(History::Undo).unwrap();
            prop_assert_eq!(snapshot.page(snapshot.page_order()[0]).unwrap().roots(), order.as_slice());
        }
    }

    #[test]
    fn arbitrary_valid_placement_sequences_undo_and_redo_exactly(placements in prop::collection::vec((-1000i32..1000, -1000i32..1000), 1..40)) {
        let mut session = session();
        let page = page_owner(&session);
        let node = group(&mut session, page, "Group");
        let mut states = vec![artwork(&session.snapshot())];
        for (x, y) in placements {
            session.transact(Edit::new("Place", [Change::SetTransform {
                node, transform: Transform::new([1.0, 0.0, 0.0, 1.0, f64::from(x), f64::from(y)]).unwrap(),
            }])).unwrap();
            states.push(artwork(&session.snapshot()));
        }
        for state in states[..states.len() - 1].iter().rev() {
            prop_assert_eq!(&artwork(&session.history(History::Undo).unwrap()), state);
        }
        for state in &states[1..] {
            prop_assert_eq!(&artwork(&session.history(History::Redo).unwrap()), state);
        }
    }
}
