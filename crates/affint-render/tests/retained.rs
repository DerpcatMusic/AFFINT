//! Retained-work acceptance through the public document and renderer interfaces.

use std::cell::Cell;
use std::sync::{Arc, Barrier};

use affint_document::{
    Change, Color, Content, Edit, History, LayerSpec, Limits as DocumentLimits, LiveOperation,
    Mask, MaskMode, NodeId, PageSize, Parent, PixelSize, RasterAsset, Session, Snapshot, Transform,
    VectorContent,
};
use affint_render::{
    Error, Frame, FramePixels, Limits, OutputFormat, RenderRequest, Renderer, View,
};
use kurbo::{BezPath, Rect};

const MIB: usize = 1024 * 1024;

fn session(width: f64, height: f64) -> Session {
    Session::new(
        PageSize::new(width, height).unwrap(),
        DocumentLimits::default(),
    )
    .unwrap()
}

fn page(session: &Session) -> Parent {
    Parent::Page(session.snapshot().page_order()[0])
}

fn add(session: &mut Session, parent: Parent, layer: LayerSpec) -> NodeId {
    let snapshot = session.snapshot();
    let index = match parent {
        Parent::Page(id) => snapshot.page(id).unwrap().roots().len(),
        Parent::Layer(id) => snapshot.node(id).unwrap().children().len(),
    };
    session
        .transact(Edit::new(
            "Retained fixture",
            [Change::InsertLayer {
                parent,
                index,
                layer,
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn properties(session: &mut Session, id: NodeId, change: impl FnOnce(&mut LayerSpec)) {
    let snapshot = session.snapshot();
    let layer = snapshot.node(id).unwrap();
    let mut spec = LayerSpec::new(layer.name(), layer.content().clone());
    spec.transform = layer.transform();
    spec.opacity = layer.opacity();
    spec.visible = layer.visible();
    spec.mask = layer.mask();
    spec.operations = layer.operations().to_vec();
    change(&mut spec);
    session
        .transact(Edit::new(
            "Change retained fixture",
            [Change::SetLayer {
                node: id,
                layer: spec,
            }],
        ))
        .unwrap();
}

fn rectangle(rect: Rect, color: [f64; 4]) -> Content {
    let mut path = BezPath::new();
    path.move_to((rect.x0, rect.y0));
    path.line_to((rect.x1, rect.y0));
    path.line_to((rect.x1, rect.y1));
    path.line_to((rect.x0, rect.y1));
    path.close_path();
    Content::Vector(
        VectorContent::new(
            path,
            Color::new(color[0], color[1], color[2], color[3]).unwrap(),
        )
        .unwrap(),
    )
}

fn curve() -> Content {
    let mut path = BezPath::new();
    path.move_to((2.125, 3.5));
    path.curve_to((12.25, -8.0), (43.75, 40.5), (68.125, 9.5));
    path.line_to((67.25, 26.125));
    path.quad_to((27.0, 19.0), (2.125, 3.5));
    path.close_path();
    Content::Vector(VectorContent::new(path, Color::new(0.17, 0.61, 0.93, 0.19).unwrap()).unwrap())
}

fn make_request(session: &Session, width: u32, height: u32, transform: Transform) -> RenderRequest {
    RenderRequest::new(
        session.snapshot().page_order()[0],
        width,
        height,
        View::Affine(transform),
    )
    .unwrap()
    .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
    .unwrap()
}

fn retained(tile_size: u32) -> Renderer {
    Renderer::new_retained(
        Limits {
            tile_size,
            max_bytes: 128 * MIB,
            ..Limits::default()
        },
        64 * MIB,
    )
    .unwrap()
}

fn assert_pixels(actual: &Frame, expected: &Frame) {
    assert_eq!(actual.request(), expected.request());
    assert_eq!(actual.revision(), expected.revision());
    match (actual.pixels(), expected.pixels()) {
        (FramePixels::SrgbRgba8Straight(a), FramePixels::SrgbRgba8Straight(b)) => assert_eq!(a, b),
        (
            FramePixels::LinearSrgbRgbaF32Premultiplied(a),
            FramePixels::LinearSrgbRgbaF32Premultiplied(b),
        ) => {
            assert_eq!(a.len(), b.len());
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_eq!(
                    a.map(f32::to_bits),
                    b.map(f32::to_bits),
                    "working pixel {index}"
                );
            }
        }
        _ => panic!("output format differs"),
    }
}

fn oracle(renderer: &Renderer, snapshot: &Snapshot, request: RenderRequest) -> Frame {
    let fresh = Renderer::new(renderer.limits().clone()).unwrap();
    let expected = fresh.render(snapshot, request).unwrap();
    let actual = renderer.render(snapshot, request).unwrap();
    assert_pixels(&actual, &expected);
    drop(expected);
    assert_eq!(fresh.allocated_bytes(), 0);
    actual
}

fn both(renderer: &Renderer, session: &Session, request: RenderRequest) {
    for format in [
        OutputFormat::SrgbRgba8Straight,
        OutputFormat::LinearSrgbRgbaF32Premultiplied,
    ] {
        drop(oracle(
            renderer,
            &session.snapshot(),
            request.with_format(format).unwrap(),
        ));
    }
}

#[test]
fn changing_one_of_64_curves_reuses_all_unchanged_layer_tiles_and_geometry() {
    let mut session = session(96.0, 40.0);
    let parent = page(&session);
    let ids: Vec<_> = (0..64)
        .map(|_| {
            add(
                &mut session,
                parent,
                LayerSpec::new("Original curve", curve()),
            )
        })
        .collect();
    let renderer = retained(256);
    let request = make_request(&session, 96, 40, Transform::IDENTITY);
    both(&renderer, &session, request);
    let warm = renderer.retained_stats();
    assert_eq!(warm.prepared_curves, 64);
    session
        .transact(Edit::new(
            "Move only one curve",
            [Change::SetTransform {
                node: ids[17],
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, 0.125, -0.25]).unwrap(),
            }],
        ))
        .unwrap();
    drop(oracle(&renderer, &session.snapshot(), request));
    let changed = renderer.retained_stats();
    assert_eq!(changed.prepared_curves - warm.prepared_curves, 1);
    assert_eq!(changed.evaluated_tiles - warm.evaluated_tiles, 1);
    assert_eq!(changed.tile_hits - warm.tile_hits, 63);
    assert_eq!(changed.geometry_hits - warm.geometry_hits, 63);
    renderer.clear_retained();
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn same_path_owner_different_fill_invalidates_paint_but_reuses_geometry() {
    let mut session = session(96.0, 40.0);
    let parent = page(&session);
    let id = add(
        &mut session,
        parent,
        LayerSpec::new("Editable fill", curve()),
    );
    let renderer = retained(17);
    let request = make_request(&session, 96, 40, Transform::IDENTITY);
    both(&renderer, &session, request);
    let before = renderer.retained_stats();
    let snapshot = session.snapshot();
    let Content::Vector(original) = snapshot.node(id).unwrap().content() else {
        panic!("vector fixture")
    };
    let path_pointer = original.path().elements().as_ptr();
    properties(&mut session, id, |spec| {
        let Content::Vector(vector) = &spec.content else {
            panic!("vector fixture")
        };
        spec.content =
            Content::Vector(vector.with_fill(Color::new(0.93, 0.12, 0.08, 0.71).unwrap()));
    });
    let current = session.snapshot();
    let Content::Vector(changed) = current.node(id).unwrap().content() else {
        panic!("vector fixture")
    };
    assert_eq!(changed.path().elements().as_ptr(), path_pointer);
    both(&renderer, &session, request);
    assert_eq!(
        renderer.retained_stats().prepared_curves,
        before.prepared_curves
    );
    assert!(renderer.retained_stats().evaluated_tiles > before.evaluated_tiles);
    assert!(renderer.retained_stats().geometry_hits > before.geometry_hits);
    // Fractional camera miss still prepares the exact curve once for all color tiles.
    let before = renderer.retained_stats();
    let shifted = request
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
        .unwrap();
    let shifted = RenderRequest::new(
        shifted.page(),
        shifted.width(),
        shifted.height(),
        View::Affine(Transform::new([1.0, 0.0, 0.0, 1.0, -3.25, 0.125]).unwrap()),
    )
    .unwrap()
    .with_format(shifted.format())
    .unwrap();
    both(&renderer, &session, shifted);
    assert_eq!(
        renderer.retained_stats().prepared_curves - before.prepared_curves,
        1
    );
}

#[test]
fn complete_grid_translation_reuses_exact_line_and_raster_tiles_and_phase_changes_miss() {
    let mut session = session(1024.0, 512.0);
    let parent = page(&session);
    let pixels: Vec<_> = (0..1024 * 512)
        .flat_map(|i| [(i % 251) as u8, (i % 127) as u8, (i % 61) as u8, 173])
        .collect();
    let asset = Arc::new(RasterAsset::new(PixelSize::new(1024, 512).unwrap(), pixels).unwrap());
    session
        .transact(Edit::new(
            "Original raster",
            [Change::InsertRaster {
                parent,
                index: 0,
                name: "Source".to_owned(),
                asset,
                transform: Transform::IDENTITY,
            }],
        ))
        .unwrap();
    add(
        &mut session,
        parent,
        LayerSpec::new(
            "Line path",
            rectangle(
                Rect::new(140.125, 32.75, 690.5, 225.125),
                [0.8, 0.1, 0.4, 0.37],
            ),
        ),
    );
    let renderer = retained(256);
    let base = make_request(&session, 512, 256, Transform::IDENTITY);
    both(&renderer, &session, base);
    let before = renderer.retained_stats();
    let pan = make_request(
        &session,
        512,
        256,
        Transform::new([1.0, 0.0, 0.0, 1.0, -256.0, 0.0]).unwrap(),
    );
    drop(oracle(&renderer, &session.snapshot(), pan));
    let after = renderer.retained_stats();
    assert_eq!(after.tile_hits - before.tile_hits, 2);
    assert_eq!(after.evaluated_tiles - before.evaluated_tiles, 2);
    both(&renderer, &session, pan);
    for affine in [
        [1.0, 0.0, 0.0, 1.0, -256.125, 0.0],
        [1.0001, 0.0, 0.0, 1.0001, -256.0, 0.0],
        [1.0, 0.003, 0.001, 1.0, -256.0, 0.0],
    ] {
        let before = renderer.retained_stats();
        both(
            &renderer,
            &session,
            make_request(&session, 512, 256, Transform::new(affine).unwrap()),
        );
        assert!(renderer.retained_stats().evaluated_tiles > before.evaluated_tiles);
    }
}

#[test]
fn crop_exposure_opacity_path_and_hidden_target_local_mask_edits_match_fresh_output() {
    let mut session = session(80.0, 48.0);
    let parent = page(&session);
    let mut mask_group = LayerSpec::new("Hidden mask owner", Content::Group);
    mask_group.visible = false;
    mask_group.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 900.0, 1200.0]).unwrap();
    let owner = add(&mut session, parent, mask_group);
    let mut mask = LayerSpec::new(
        "Hidden mask source",
        rectangle(Rect::new(5.25, 1.75, 47.5, 38.125), [0.2, 0.7, 0.1, 0.63]),
    );
    mask.visible = false;
    let source = add(&mut session, Parent::Layer(owner), mask);
    let target = add(&mut session, parent, LayerSpec::new("Target", curve()));
    let renderer = retained(17);
    let request = make_request(&session, 80, 48, Transform::IDENTITY);
    properties(&mut session, target, |spec| {
        spec.mask = Some(Mask {
            source,
            mode: MaskMode::Alpha,
            inverted: false,
        })
    });
    both(&renderer, &session, request);
    let before = renderer.retained_stats();
    session
        .transact(Edit::new(
            "Move normal mask ancestor",
            [Change::SetTransform {
                node: owner,
                transform: Transform::new([1.0, 0.0, 0.0, 1.0, -500.0, 700.0]).unwrap(),
            }],
        ))
        .unwrap();
    drop(oracle(&renderer, &session.snapshot(), request));
    assert_eq!(
        renderer.retained_stats().evaluated_tiles,
        before.evaluated_tiles
    );
    for (mode, inverted) in [
        (MaskMode::Luminance, false),
        (MaskMode::Luminance, true),
        (MaskMode::Alpha, true),
    ] {
        properties(&mut session, target, |spec| {
            spec.mask = Some(Mask {
                source,
                mode,
                inverted,
            })
        });
        both(&renderer, &session, request);
    }
    properties(&mut session, source, |spec| {
        spec.content = rectangle(Rect::new(7.125, 2.0, 62.25, 40.5), [0.9, 0.13, 0.47, 0.41]);
        spec.transform = Transform::new([1.0, 0.02, 0.015, 1.0, 3.25, -1.5]).unwrap();
    });
    both(&renderer, &session, request);
    properties(&mut session, target, |spec| {
        spec.operations = vec![
            LiveOperation::Exposure(2.0),
            LiveOperation::Crop(Rect::new(3.125, 1.75, 62.5, 27.5)),
        ]
    });
    both(&renderer, &session, request);
    properties(&mut session, target, |spec| {
        spec.opacity = 0.43;
        spec.operations.reverse();
    });
    both(&renderer, &session, request);
    properties(&mut session, target, |spec| {
        spec.content = rectangle(Rect::new(0.5, 4.25, 61.125, 43.25), [0.4, 0.1, 0.9, 0.83])
    });
    both(&renderer, &session, request);
}

#[test]
fn root_and_group_order_isolation_visibility_undo_and_page_clip_are_fresh() {
    let mut session = session(64.0, 40.0);
    let parent = page(&session);
    let a = add(
        &mut session,
        parent,
        LayerSpec::new(
            "A",
            rectangle(Rect::new(0.125, 0.25, 61.25, 38.5), [0.9, 0.1, 0.3, 0.6]),
        ),
    );
    let mut group = LayerSpec::new("Isolated group", Content::Group);
    group.opacity = 0.47;
    let group = add(&mut session, parent, group);
    let b = add(
        &mut session,
        Parent::Layer(group),
        LayerSpec::new(
            "B",
            rectangle(Rect::new(2.75, 1.125, 52.5, 35.25), [0.1, 0.9, 0.3, 0.7]),
        ),
    );
    add(
        &mut session,
        Parent::Layer(group),
        LayerSpec::new(
            "C",
            rectangle(Rect::new(4.125, 5.5, 63.0, 39.125), [0.3, 0.1, 0.9, 0.8]),
        ),
    );
    let renderer = retained(256);
    let request = make_request(&session, 64, 40, Transform::IDENTITY);
    both(&renderer, &session, request);
    let before = renderer.retained_stats();
    session
        .transact(Edit::new(
            "Reorder roots",
            [Change::MoveLayer {
                node: a,
                parent,
                index: 1,
            }],
        ))
        .unwrap();
    drop(oracle(&renderer, &session.snapshot(), request));
    assert_eq!(
        renderer.retained_stats().evaluated_tiles,
        before.evaluated_tiles
    );
    session
        .transact(Edit::new(
            "Reorder group",
            [Change::MoveLayer {
                node: b,
                parent: Parent::Layer(group),
                index: 1,
            }],
        ))
        .unwrap();
    both(&renderer, &session, request);
    session
        .transact(Edit::new(
            "Reparent with new isolation",
            [Change::MoveLayer {
                node: b,
                parent,
                index: 0,
            }],
        ))
        .unwrap();
    both(&renderer, &session, request);
    session.history(History::Undo).unwrap();
    both(&renderer, &session, request);
    session.history(History::Redo).unwrap();
    both(&renderer, &session, request);
    properties(&mut session, group, |spec| spec.visible = false);
    both(&renderer, &session, request);
    session.history(History::Undo).unwrap();
    both(&renderer, &session, request);
    session
        .transact(Edit::new(
            "New page clipping",
            [Change::SetPage {
                page: request.page(),
                name: "Small page".to_owned(),
                size: PageSize::new(24.25, 31.5).unwrap(),
            }],
        ))
        .unwrap();
    both(&renderer, &session, request);
    let new_page = session
        .transact(Edit::new(
            "Distinct empty page",
            [Change::AddPage {
                name: "Empty".to_owned(),
                size: PageSize::new(64.0, 40.0).unwrap(),
            }],
        ))
        .unwrap()
        .pages[0];
    let empty = RenderRequest::new(new_page, 64, 40, request.view())
        .unwrap()
        .with_format(request.format())
        .unwrap();
    both(&renderer, &session, empty);
    both(&renderer, &session, request);
}

#[test]
fn one_original_capacity_is_charged_once_for_many_tiles_layers_and_concurrent_clones() {
    let mut session = session(512.0, 256.0);
    let parent = page(&session);
    let mut pixels = Vec::with_capacity(256 * 1024);
    pixels.resize(4 * 4 * 4, 127);
    let asset = Arc::new(RasterAsset::new(PixelSize::new(4, 4).unwrap(), pixels).unwrap());
    let capacity = asset.allocation_bytes();
    let pointer = asset.pixels().as_ptr();
    let created = session
        .transact(Edit::new(
            "Shared original",
            [Change::InsertRaster {
                parent,
                index: 0,
                name: "Original".to_owned(),
                asset: Arc::clone(&asset),
                transform: Transform::new([128.0, 0.0, 0.0, 64.0, 0.0, 0.0]).unwrap(),
            }],
        ))
        .unwrap();
    add(
        &mut session,
        parent,
        LayerSpec::new(
            "Same original again",
            Content::Raster {
                asset: created.assets[0],
            },
        ),
    );
    let renderer = retained(256);
    let request = make_request(&session, 512, 256, Transform::IDENTITY);
    let snapshot = session.snapshot();
    let barrier = Barrier::new(3);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let renderer = renderer.clone();
                let snapshot = &snapshot;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    renderer.render(snapshot, request).unwrap()
                })
            })
            .collect();
        barrier.wait();
        for handle in handles {
            drop(handle.join().unwrap());
        }
    });
    both(&renderer, &session, request);
    assert_eq!(renderer.retained_stats().source_bytes, capacity);
    assert!(capacity > asset.pixels().len());
    assert_eq!(asset.pixels().as_ptr(), pointer);
    assert!(renderer.retained_stats().resident_bytes <= 64 * MIB);
    renderer.clear_retained();
    assert_eq!(renderer.retained_stats().source_bytes, 0);
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn evicted_active_tile_remains_charged_and_cancellation_publishes_no_frame() {
    let mut session = session(32.0, 24.0);
    let parent = page(&session);
    let asset = Arc::new(RasterAsset::new(PixelSize::new(2, 2).unwrap(), vec![127; 16]).unwrap());
    session
        .transact(Edit::new(
            "Owned original",
            [Change::InsertRaster {
                parent,
                index: 0,
                name: "Source".to_owned(),
                asset,
                transform: Transform::IDENTITY,
            }],
        ))
        .unwrap();
    let renderer = retained(256);
    let request = make_request(&session, 32, 24, Transform::IDENTITY);
    let frame = oracle(&renderer, &session.snapshot(), request);
    let output_bytes = frame.byte_len();
    let hits = renderer.retained_stats().tile_hits;
    let observed = Cell::new(false);
    let result = renderer.render_with_cancel(&session.snapshot(), request, || {
        if renderer.retained_stats().tile_hits > hits {
            renderer.clear_retained();
            assert!(renderer.retained_stats().resident_bytes > 0);
            assert!(renderer.retained_stats().source_bytes > 0);
            observed.set(true);
            true
        } else {
            false
        }
    });
    assert!(matches!(result, Err(Error::Canceled)));
    assert!(observed.get());
    renderer.clear_retained();
    assert_eq!(renderer.retained_stats().resident_bytes, 0);
    assert_eq!(renderer.allocated_bytes(), output_bytes);
    let clone = renderer.clone();
    drop(renderer);
    assert_eq!(clone.allocated_bytes(), output_bytes);
    drop(frame);
    assert_eq!(clone.allocated_bytes(), 0);
}

#[test]
fn immediate_cancel_failed_evaluation_and_retention_refusal_preserve_exact_policy() {
    let mut session = session(16.0, 12.0);
    let parent = page(&session);
    let id = add(
        &mut session,
        parent,
        LayerSpec::new(
            "Finite source",
            rectangle(Rect::new(0.0, 0.0, 16.0, 12.0), [1.0, 0.3, 0.8, 1.0]),
        ),
    );
    let renderer = retained(256);
    let request = make_request(&session, 16, 12, Transform::IDENTITY);
    assert!(matches!(
        renderer.render_with_cancel(&session.snapshot(), request, || true),
        Err(Error::Canceled)
    ));
    assert_eq!(renderer.retained_stats().resident_bytes, 0);
    assert_eq!(renderer.allocated_bytes(), 0);
    both(&renderer, &session, request);
    properties(&mut session, id, |spec| {
        spec.operations = vec![LiveOperation::Exposure(32.0); 4]
    });
    let fresh = Renderer::new(renderer.limits().clone()).unwrap();
    assert!(matches!(
        fresh.render(&session.snapshot(), request),
        Err(Error::ColorRange { .. })
    ));
    assert!(matches!(
        renderer.render(&session.snapshot(), request),
        Err(Error::ColorRange { .. })
    ));
    renderer.clear_retained();
    assert_eq!(renderer.allocated_bytes(), 0);
    session.history(History::Undo).unwrap();
    let tiny = Renderer::new_retained(Limits::default(), 128).unwrap();
    both(&tiny, &session, request);
    assert!(tiny.retained_stats().refused > 0);
    tiny.clear_retained();
    assert_eq!(tiny.allocated_bytes(), 0);
    assert_eq!(request.width(), 16);
    assert_eq!(request.height(), 12);
}

#[test]
fn source_spare_capacity_refusal_and_cached_map_eviction_restore_uncached_admission() {
    let mut session = session(16.0, 12.0);
    let parent = page(&session);
    let mut pixels = Vec::with_capacity(2 * MIB);
    pixels.resize(16, 127);
    let asset = Arc::new(RasterAsset::new(PixelSize::new(2, 2).unwrap(), pixels).unwrap());
    session
        .transact(Edit::new(
            "Large original capacity",
            [Change::InsertRaster {
                parent,
                index: 0,
                name: "Source".to_owned(),
                asset,
                transform: Transform::IDENTITY,
            }],
        ))
        .unwrap();
    let renderer = Renderer::new_retained(Limits::default(), MIB).unwrap();
    let request = make_request(&session, 16, 12, Transform::IDENTITY);
    both(&renderer, &session, request);
    assert_eq!(renderer.retained_stats().source_bytes, 0);
    assert!(renderer.retained_stats().refused > 0);
    renderer.clear_retained();
    assert_eq!(renderer.allocated_bytes(), 0);
    let limit = Renderer::new_retained(
        Limits {
            max_bytes: 8 * MIB,
            ..Limits::default()
        },
        6 * MIB,
    )
    .unwrap();
    // A later output has a larger simultaneous peak. Retained owners and map
    // capacity are evicted if they prevent ordinary exact output admission.
    both(&limit, &session, request);
    let larger = RenderRequest::new(request.page(), 256, 256, request.view())
        .unwrap()
        .with_format(request.format())
        .unwrap();
    drop(oracle(&limit, &session.snapshot(), larger));
    assert!(limit.retained_stats().evictions > 0);
    limit.clear_retained();
    assert_eq!(limit.allocated_bytes(), 0);
}

#[test]
fn old_original_owner_is_bounded_after_session_drop_and_same_local_ids_do_not_alias() {
    let renderer = retained(256);
    let mut old = session(8.0, 8.0);
    let parent = page(&old);
    let asset = Arc::new(RasterAsset::new(PixelSize::new(2, 2).unwrap(), vec![211; 16]).unwrap());
    let capacity = asset.allocation_bytes();
    old.transact(Edit::new(
        "Old original",
        [Change::InsertRaster {
            parent,
            index: 0,
            name: "Source".to_owned(),
            asset,
            transform: Transform::IDENTITY,
        }],
    ))
    .unwrap();
    let request = make_request(&old, 8, 8, Transform::IDENTITY);
    both(&renderer, &old, request);
    drop(old);
    assert_eq!(renderer.retained_stats().source_bytes, capacity);
    let mut new = session(8.0, 8.0);
    let parent = page(&new);
    let asset = Arc::new(RasterAsset::new(PixelSize::new(2, 2).unwrap(), vec![37; 16]).unwrap());
    new.transact(Edit::new(
        "Distinct original",
        [Change::InsertRaster {
            parent,
            index: 0,
            name: "Source".to_owned(),
            asset,
            transform: Transform::IDENTITY,
        }],
    ))
    .unwrap();
    let new_request = make_request(&new, 8, 8, Transform::IDENTITY);
    assert_eq!(request.page(), new_request.page());
    let before = renderer.retained_stats();
    drop(oracle(&renderer, &new.snapshot(), new_request));
    assert!(renderer.retained_stats().evaluated_tiles > before.evaluated_tiles);
    assert_eq!(renderer.retained_stats().source_bytes, 2 * capacity);
    for (width, height) in [(9, 8), (8, 9), (17, 11)] {
        let changed = RenderRequest::new(new_request.page(), width, height, new_request.view())
            .unwrap()
            .with_format(new_request.format())
            .unwrap();
        both(&renderer, &new, changed);
    }
    renderer.clear_retained();
    assert_eq!(renderer.retained_stats().resident_bytes, 0);
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn optional_prepared_memo_does_not_refuse_the_uncached_admission_threshold() {
    let mut session = session(256.0, 256.0);
    let parent = page(&session);
    add(
        &mut session,
        parent,
        LayerSpec::new(
            "Exact coverage",
            rectangle(Rect::new(0.125, 0.25, 250.25, 250.75), [0.3, 0.7, 0.1, 0.8]),
        ),
    );
    let request = make_request(&session, 256, 256, Transform::IDENTITY);
    let probe = Renderer::new(Limits {
        max_bytes: MIB,
        ..Limits::default()
    })
    .unwrap();
    let threshold = match probe.render(&session.snapshot(), request) {
        Err(Error::BudgetExceeded {
            requested, used, ..
        }) => requested.checked_add(used).unwrap(),
        result => panic!("expected exact admission probe refusal, got {result:?}"),
    };
    assert_eq!(probe.allocated_bytes(), 0);
    let limits = Limits {
        max_bytes: threshold,
        ..Limits::default()
    };
    let fresh = Renderer::new(limits.clone()).unwrap();
    let retained = Renderer::new_retained(limits, MIB).unwrap();
    let expected = fresh.render(&session.snapshot(), request).unwrap();
    let actual = retained.render(&session.snapshot(), request).unwrap();
    assert_pixels(&actual, &expected);
    drop(expected);
    drop(actual);
    retained.clear_retained();
    assert_eq!(fresh.allocated_bytes(), 0);
    assert_eq!(retained.allocated_bytes(), 0);
}

#[test]
fn cached_curve_memo_is_reclaimed_before_mandatory_preflight_workspace() {
    let mut session = Session::new(
        PageSize::new(96.0, 40.0).unwrap(),
        DocumentLimits {
            max_history_entries: 0,
            max_history_metadata_bytes: 0,
            ..DocumentLimits::default()
        },
    )
    .unwrap();
    let parent = page(&session);
    let original = curve();
    for _ in 0..256 {
        add(
            &mut session,
            parent,
            LayerSpec::new("Warm geometry", original.clone()),
        );
    }
    let invalid = add(
        &mut session,
        parent,
        LayerSpec::new(
            "Late unsupported working scalar",
            rectangle(Rect::new(0.0, 0.0, 8.0, 8.0), [1e-300, 0.3, 0.7, 1.0]),
        ),
    );
    let prime = session.snapshot();
    properties(&mut session, invalid, |spec| spec.visible = false);
    // Hidden artwork still needs the support-index slots, but does not change
    // the actual visible evaluation or its largest coverage workspace.
    for _ in 0..512 {
        let mut hidden = LayerSpec::new("Unreached geometry metadata", original.clone());
        hidden.visible = false;
        add(&mut session, parent, hidden);
    }
    add(
        &mut session,
        parent,
        LayerSpec::new("Guaranteed cold visible suffix", original),
    );
    let request = make_request(&session, 12, 8, Transform::IDENTITY);
    let snapshot = session.snapshot();
    let mut threshold = MIB;
    let mut admitted = false;
    for _ in 0..4 {
        let fresh = Renderer::new(Limits {
            max_bytes: threshold,
            ..Limits::default()
        })
        .unwrap();
        match fresh.render(&snapshot, request) {
            Err(Error::BudgetExceeded {
                requested, used, ..
            }) => threshold = requested.checked_add(used).unwrap(),
            Ok(frame) => {
                drop(frame);
                admitted = true;
                break;
            }
            result => panic!("unexpected admission probe {result:?}"),
        }
    }
    assert!(
        admitted,
        "uncached admission must reach a finite stable peak"
    );
    let limits = Limits {
        max_bytes: threshold,
        ..Limits::default()
    };
    let renderer = Renderer::new_retained(limits, threshold).unwrap();
    assert!(matches!(
        renderer.render(&prime, request),
        Err(Error::ColorRange { .. })
    ));
    assert!(renderer.retained_stats().prepared_curves > 0);
    assert!(renderer.retained_stats().prepared_curves < 256);
    assert!(renderer.retained_stats().resident_bytes > 0);
    drop(oracle(&renderer, &snapshot, request));
    assert!(renderer.retained_stats().geometry_hits > 0);
    assert!(renderer.retained_stats().preflight_reclaims > 0);
    renderer.clear_retained();
    assert_eq!(renderer.allocated_bytes(), 0);
}
