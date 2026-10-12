//! Resource admission and lifetime accounting through the public document seam.

use std::sync::Arc;

use affint_document::{
    Change, Color, Content, Edit, Error, History, LayerSpec, Limits, LiveOperation, Mask, MaskMode,
    PageSize, Parent, PixelSize, RasterAsset, Session, Transform, VectorContent,
};
use kurbo::{BezPath, Rect};

fn new(limits: Limits) -> Session {
    Session::new(PageSize::new(64.0, 64.0).unwrap(), limits).unwrap()
}

fn parent(session: &Session) -> Parent {
    Parent::Page(session.snapshot().page_order()[0])
}

fn pixels() -> Arc<RasterAsset> {
    Arc::new(RasterAsset::new(PixelSize::new(2, 2).unwrap(), vec![37; 16]).unwrap())
}

fn insert(
    session: &mut Session,
    source: Arc<RasterAsset>,
) -> Result<affint_document::Created, Error> {
    let parent = parent(session);
    session.transact(Edit::new(
        "Photo",
        [Change::InsertRaster {
            parent,
            index: 0,
            name: "Source".to_owned(),
            asset: source,
            transform: Transform::IDENTITY,
        }],
    ))
}

#[test]
fn outstanding_branch_snapshots_keep_their_source_allocation_in_the_budget() {
    let limits = Limits {
        max_pixel_bytes: 16,
        ..Limits::default()
    };
    let mut session = new(limits);
    let first = insert(&mut session, pixels()).unwrap();
    let outstanding = session.snapshot();
    session.history(History::Undo).unwrap();
    let before = session.snapshot();
    assert!(matches!(
        insert(&mut session, pixels()),
        Err(Error::LimitExceeded {
            resource: "retained pixel bytes",
            actual: 32,
            limit: 16
        })
    ));
    assert_eq!(session.snapshot().revision(), before.revision());
    assert_eq!(session.history_status().redo, 1);
    assert_eq!(
        outstanding.asset(first.assets[0]).unwrap().pixels(),
        &[37; 16]
    );
    // Even dropping the external snapshot leaves the redo side owning the source.
    drop(outstanding);
    assert_eq!(session.resource_usage().unwrap().pixel_bytes, 16);
    let page = parent(&session);
    session
        .transact(Edit::new(
            "Branch without source",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Empty group", Content::Group),
            }],
        ))
        .unwrap();
    assert_eq!(session.resource_usage().unwrap().pixel_bytes, 0);
    insert(&mut session, pixels()).unwrap();
}

#[test]
fn canceled_previews_count_sources_until_their_outstanding_handles_are_dropped() {
    let mut session = new(Limits {
        max_pixel_bytes: 16,
        ..Limits::default()
    });
    let page = parent(&session);
    session.begin_gesture("Preview import").unwrap();
    let (preview, _) = session
        .update_gesture(Edit::new(
            "Import",
            [Change::InsertRaster {
                parent: page,
                index: 0,
                name: "Preview".to_owned(),
                asset: pixels(),
                transform: Transform::IDENTITY,
            }],
        ))
        .unwrap();
    session.cancel_gesture().unwrap();
    assert_eq!(session.resource_usage().unwrap().pixel_bytes, 16);
    assert!(matches!(
        insert(&mut session, pixels()),
        Err(Error::LimitExceeded {
            resource: "retained pixel bytes",
            ..
        })
    ));
    drop(preview);
    assert_eq!(session.resource_usage().unwrap().pixel_bytes, 0);
    insert(&mut session, pixels()).unwrap();
}

#[test]
fn shared_sources_under_multiple_asset_ids_count_one_owner() {
    let mut session = new(Limits {
        max_pixel_bytes: 16,
        ..Limits::default()
    });
    let source = pixels();
    let first = insert(&mut session, Arc::clone(&source)).unwrap();
    let second = insert(&mut session, Arc::new(source.as_ref().clone())).unwrap();
    assert_ne!(first.assets[0], second.assets[0]);
    let snapshot = session.snapshot();
    assert_eq!(
        snapshot.asset(first.assets[0]).unwrap().pixels().as_ptr(),
        snapshot.asset(second.assets[0]).unwrap().pixels().as_ptr()
    );
    assert_eq!(session.resource_usage().unwrap().pixel_bytes, 16);
}

#[test]
fn source_capacity_is_charged_even_when_declared_pixels_are_small() {
    let mut session = new(Limits {
        max_pixel_bytes: 63,
        ..Limits::default()
    });
    let mut bytes = Vec::with_capacity(64);
    bytes.resize(16, 0);
    let source = Arc::new(RasterAsset::new(PixelSize::new(2, 2).unwrap(), bytes).unwrap());
    assert!(matches!(
        insert(&mut session, source),
        Err(Error::LimitExceeded {
            resource: "source pixel bytes",
            actual: 64,
            limit: 63
        })
    ));
    assert!(session.snapshot().assets().next().is_none());
    assert_eq!(session.history_status().undo, 0);
}

#[test]
fn retained_history_obeys_entry_and_byte_limits_on_commit_undo_and_redo() {
    let mut session = new(Limits {
        max_history_entries: 2,
        ..Limits::default()
    });
    let page = parent(&session);
    for number in 0..4 {
        session
            .transact(Edit::new(
                format!("Group {number}"),
                [Change::InsertLayer {
                    parent: page,
                    index: 0,
                    layer: LayerSpec::new(number.to_string(), Content::Group),
                }],
            ))
            .unwrap();
    }
    assert_eq!(session.history_status().undo, 2);
    session.history(History::Undo).unwrap();
    session.history(History::Undo).unwrap();
    assert_eq!(
        session.history(History::Undo).unwrap_err(),
        Error::EmptyHistory("undo")
    );
    assert_eq!(session.snapshot().layers().count(), 2);
    session.history(History::Redo).unwrap();
    session.history(History::Redo).unwrap();
    assert_eq!(session.snapshot().layers().count(), 4);

    let initial_charge = new(Limits::default())
        .resource_usage()
        .unwrap()
        .current_metadata_bytes;
    let byte_limit = initial_charge + "Small edit".len() + 128;
    let mut session = new(Limits {
        max_history_metadata_bytes: byte_limit,
        ..Limits::default()
    });
    let page = parent(&session);
    session
        .transact(Edit::new(
            "Small edit",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Now a bigger state", Content::Group),
            }],
        ))
        .unwrap();
    assert_eq!(session.history_status().undo, 1);
    session.history(History::Undo).unwrap();
    // The larger redo state cannot fit this deliberately small history budget.
    assert_eq!(session.history_status().redo, 0);
    assert!(session.resource_usage().unwrap().history_metadata_bytes <= byte_limit);
}

#[test]
fn imports_enforce_pixel_graph_depth_and_geometry_budgets_before_publication() {
    let mut session = new(Limits::default());
    insert(&mut session, pixels()).unwrap();
    let archive = session.snapshot().archive().unwrap();
    assert!(matches!(
        Session::from_archive(
            archive,
            Limits {
                max_pixel_bytes: 15,
                ..Limits::default()
            }
        ),
        Err(Error::LimitExceeded { .. })
    ));

    let mut session = new(Limits::default());
    let mut owner = parent(&session);
    for _ in 0..4 {
        let node = session
            .transact(Edit::new(
                "Nested",
                [Change::InsertLayer {
                    parent: owner,
                    index: 0,
                    layer: LayerSpec::new("Nested", Content::Group),
                }],
            ))
            .unwrap()
            .nodes[0];
        owner = Parent::Layer(node);
    }
    assert!(matches!(
        Session::from_archive(
            session.snapshot().archive().unwrap(),
            Limits {
                max_depth: 3,
                ..Limits::default()
            }
        ),
        Err(Error::LimitExceeded {
            resource: "dependency depth",
            actual: 4,
            limit: 3
        })
    ));

    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close_path();
    let vector = VectorContent::new(path, Color::new(0.25, 0.5, 0.75, 1.0).unwrap()).unwrap();
    let page = parent(&session);
    session
        .transact(Edit::new(
            "Vector",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Triangle", Content::Vector(vector)),
            }],
        ))
        .unwrap();
    assert!(matches!(
        Session::from_archive(
            session.snapshot().archive().unwrap(),
            Limits {
                max_path_elements: 3,
                ..Limits::default()
            }
        ),
        Err(Error::LimitExceeded {
            resource: "path elements",
            actual: 4,
            limit: 3
        })
    ));
}

#[test]
fn vector_geometry_remains_shared_when_layer_properties_and_history_change() {
    let mut session = new(Limits::default());
    let page = parent(&session);
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.curve_to((3.0, 0.0), (4.0, 3.0), (0.0, 4.0));
    path.close_path();
    let vector = VectorContent::new(path, Color::new(0.25, 0.5, 0.75, 1.0).unwrap()).unwrap();
    let address = vector.path() as *const BezPath;
    let node = session
        .transact(Edit::new(
            "Curve",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Curve", Content::Vector(vector)),
            }],
        ))
        .unwrap()
        .nodes[0];
    let original = session.snapshot();
    session
        .transact(Edit::new(
            "Crop",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Crop(Rect::new(0.0, 0.0, 2.0, 3.0))],
            }],
        ))
        .unwrap();
    for snapshot in [
        original,
        session.snapshot(),
        session.history(History::Undo).unwrap(),
        session.history(History::Redo).unwrap(),
    ] {
        let Content::Vector(vector) = snapshot.node(node).unwrap().content() else {
            panic!("lost vector content")
        };
        assert_eq!(vector.path() as *const BezPath, address);
        assert_eq!(vector.path().elements().len(), 3);
    }
}

#[test]
fn hidden_mask_sources_keep_editable_dependency_data_independent_of_visibility() {
    let mut session = new(Limits::default());
    let page = parent(&session);
    let target = session
        .transact(Edit::new(
            "Target",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Target", Content::Group),
            }],
        ))
        .unwrap()
        .nodes[0];
    let mut source = LayerSpec::new("Hidden mask source", Content::Group);
    source.visible = false;
    source.opacity = 0.5;
    source.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 2.0, 3.0]).unwrap();
    source.operations = vec![LiveOperation::Crop(Rect::new(0.0, 0.0, 5.0, 5.0))];
    let source = session
        .transact(Edit::new(
            "Source",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: source,
            }],
        ))
        .unwrap()
        .nodes[0];
    let mask = Mask {
        source,
        mode: MaskMode::Alpha,
        inverted: false,
    };
    session
        .transact(Edit::new(
            "Mask",
            [Change::SetMask {
                node: target,
                mask: Some(mask),
            }],
        ))
        .unwrap();
    let reopened = Session::from_archive(session.snapshot().archive().unwrap(), Limits::default())
        .unwrap()
        .snapshot();
    assert!(!reopened.node(source).unwrap().visible());
    assert_eq!(reopened.node(source).unwrap().opacity(), 0.5);
    assert_eq!(reopened.node(target).unwrap().mask(), Some(mask));
    assert_eq!(
        reopened
            .node(source)
            .unwrap()
            .transform()
            .affine()
            .as_coeffs()[4..],
        [2.0, 3.0]
    );
    assert_eq!(reopened.node(source).unwrap().operations().len(), 1);
}

#[test]
fn accepted_gesture_updates_have_a_bounded_total_change_count() {
    let mut session = new(Limits {
        max_changes: 2,
        ..Limits::default()
    });
    let page = parent(&session);
    let node = session
        .transact(Edit::new(
            "Group",
            [Change::InsertLayer {
                parent: page,
                index: 0,
                layer: LayerSpec::new("Group", Content::Group),
            }],
        ))
        .unwrap()
        .nodes[0];
    session.begin_gesture("Long drag").unwrap();
    for _ in 0..2 {
        session
            .update_gesture(Edit::new(
                "Move",
                [Change::SetTransform {
                    node,
                    transform: Transform::IDENTITY,
                }],
            ))
            .unwrap();
    }
    assert!(matches!(
        session.update_gesture(Edit::new(
            "Move",
            [Change::SetTransform {
                node,
                transform: Transform::IDENTITY
            }]
        )),
        Err(Error::LimitExceeded {
            resource: "gesture changes",
            actual: 3,
            limit: 2
        })
    ));
    session.commit_gesture().unwrap();
    assert_eq!(session.history_status().undo_label, Some("Long drag"));
}

#[test]
fn snapshots_are_send_sync_and_independent_sessions_never_share_revision_tokens() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<affint_document::Snapshot>();
    let first = new(Limits::default());
    let second = new(Limits::default());
    assert_ne!(first.snapshot().revision(), second.snapshot().revision());
}
