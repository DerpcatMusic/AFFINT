//! Working-output acceptance through real public document/render interfaces.
//! Scalar bounds below apply to these finite fixtures, not arbitrary HDR graphs.

use std::sync::{Arc, Barrier};

use affint_document::{
    Change, Color, Content, Edit, History, LayerSpec, Limits as DocumentLimits, LiveOperation,
    Mask, MaskMode, NodeId, PageSize, Parent, PixelSize, RasterAsset, Session, Transform,
    VectorContent,
};
use affint_render::{
    Error, Frame, FramePixels, Limits, OutputFormat, RenderRequest, Renderer, View,
};
use kurbo::{BezPath, Rect};

fn session(width: u32, height: u32) -> Session {
    Session::new(
        PageSize::new(f64::from(width), f64::from(height)).unwrap(),
        DocumentLimits::default(),
    )
    .unwrap()
}

fn page(session: &Session) -> Parent {
    Parent::Page(session.snapshot().page_order()[0])
}

fn index(session: &Session, parent: Parent) -> usize {
    let snapshot = session.snapshot();
    match parent {
        Parent::Page(id) => snapshot.page(id).unwrap().roots().len(),
        Parent::Layer(id) => snapshot.node(id).unwrap().children().len(),
    }
}

fn add(session: &mut Session, parent: Parent, layer: LayerSpec) -> NodeId {
    let index = index(session, parent);
    session
        .transact(Edit::new(
            "Fixture layer",
            [Change::InsertLayer {
                parent,
                index,
                layer,
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn import(
    session: &mut Session,
    parent: Parent,
    asset: Arc<RasterAsset>,
    transform: Transform,
) -> NodeId {
    let index = index(session, parent);
    session
        .transact(Edit::new(
            "Import original fixture",
            [Change::InsertRaster {
                parent,
                index,
                name: "Immutable fixture pixels".to_owned(),
                asset,
                transform,
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn asset(width: u32, height: u32, pixels: Vec<u8>) -> Arc<RasterAsset> {
    Arc::new(RasterAsset::new(PixelSize::new(width, height).unwrap(), pixels).unwrap())
}

fn request(session: &Session, width: u32, height: u32, format: OutputFormat) -> RenderRequest {
    RenderRequest::new(
        session.snapshot().page_order()[0],
        width,
        height,
        View::Affine(Transform::IDENTITY),
    )
    .unwrap()
    .with_format(format)
    .unwrap()
}

fn working(frame: &Frame) -> &[[f32; 4]] {
    match frame.pixels() {
        FramePixels::LinearSrgbRgbaF32Premultiplied(pixels) => pixels,
        FramePixels::SrgbRgba8Straight(_) => panic!("expected working f32 output"),
    }
}

fn assert_close(actual: [f32; 4], expected: [f64; 4], absolute: f64) {
    for (channel, (&actual, &expected)) in actual.iter().zip(&expected).enumerate() {
        assert!(
            (f64::from(actual) - expected).abs() <= absolute,
            "channel {channel}: actual={actual}, oracle={expected}, fixture bound={absolute}"
        );
    }
}

#[test]
fn request_format_changes_identity_and_frame_borrows_one_matching_representation() {
    let session = session(2, 3);
    let snapshot = session.snapshot();
    let rgba8 = RenderRequest::new(
        snapshot.page_order()[0],
        2,
        3,
        View::Affine(Transform::IDENTITY),
    )
    .unwrap();
    assert_eq!(rgba8.format(), OutputFormat::SrgbRgba8Straight);
    let float = rgba8
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
        .unwrap();
    assert_ne!(rgba8, float);
    assert_eq!(float.page(), rgba8.page());
    assert_eq!(float.view(), rgba8.view());
    assert_eq!(
        float.with_format(OutputFormat::SrgbRgba8Straight).unwrap(),
        rgba8
    );
    let renderer = Renderer::new(Limits::default()).unwrap();
    let bytes = renderer.render(&snapshot, rgba8).unwrap();
    let floats = renderer.render(&snapshot, float).unwrap();
    assert_eq!(bytes.request(), rgba8);
    assert_eq!(floats.request(), float);
    assert_eq!(floats.revision(), snapshot.revision());
    assert_eq!(floats.width(), 2);
    assert_eq!(floats.height(), 3);
    assert!(matches!(bytes.pixels(), FramePixels::SrgbRgba8Straight(pixels) if pixels.len() == 24));
    assert_eq!(bytes.rgba8().unwrap(), [0; 24]);
    assert_eq!(floats.rgba8(), None);
    assert_eq!(working(&floats), [[0.0; 4]; 6]);
    assert_eq!(bytes.byte_len(), 24);
    assert_eq!(floats.byte_len(), 96);
    assert_eq!(renderer.allocated_bytes(), 120);
    drop(bytes);
    assert_eq!(renderer.allocated_bytes(), 96);
    drop(floats);
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn translucent_exposure_retains_premultiplied_hdr_while_rgba8_export_stays_clipped() {
    let mut session = session(4, 1);
    let parent = page(&session);
    let original = vec![
        255, 255, 255, 32, 255, 255, 255, 128, 255, 255, 255, 255, 0, 255, 255, 0,
    ];
    let source = asset(4, 1, original.clone());
    let original_pointer = source.pixels().as_ptr();
    let node = import(
        &mut session,
        parent,
        Arc::clone(&source),
        Transform::IDENTITY,
    );
    session
        .transact(Edit::new(
            "Translucent +2EV",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(2.0)],
            }],
        ))
        .unwrap();
    let renderer = Renderer::new(Limits::default()).unwrap();
    let float = renderer
        .render(
            &session.snapshot(),
            request(&session, 4, 1, OutputFormat::LinearSrgbRgbaF32Premultiplied),
        )
        .unwrap();
    // Independent rational interior goldens, with no coverage/interpolation ambiguity.
    assert_close(
        working(&float)[0],
        [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 32.0 / 255.0],
        1e-7,
    );
    assert_close(
        working(&float)[1],
        [512.0 / 255.0, 512.0 / 255.0, 512.0 / 255.0, 128.0 / 255.0],
        2e-7,
    );
    assert_eq!(working(&float)[2], [4.0, 4.0, 4.0, 1.0]);
    assert_eq!(working(&float)[3].map(f32::to_bits), [0; 4]);
    let bytes = renderer
        .render(
            &session.snapshot(),
            request(&session, 4, 1, OutputFormat::SrgbRgba8Straight),
        )
        .unwrap();
    assert_eq!(
        bytes.rgba8().unwrap(),
        &[
            255, 255, 255, 32, 255, 255, 255, 128, 255, 255, 255, 255, 0, 0, 0, 0
        ]
    );
    assert_eq!(source.pixels(), original);
    assert_eq!(source.pixels().as_ptr(), original_pointer);
    assert_eq!(float.byte_len(), 64);
    assert_eq!(bytes.byte_len(), 16);
}

#[test]
fn working_output_retains_f32_highlights_above_half_float_range_and_recovers_exposure() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let node = import(
        &mut session,
        parent,
        asset(1, 1, vec![255; 4]),
        Transform::IDENTITY,
    );
    let renderer = Renderer::new(Limits::default()).unwrap();
    let request = request(&session, 1, 1, OutputFormat::LinearSrgbRgbaF32Premultiplied);
    for (operations, expected) in [
        (vec![LiveOperation::Exposure(16.0)], 65_536.0f32),
        (vec![LiveOperation::Exposure(32.0)], 4_294_967_296.0f32),
        (
            vec![
                LiveOperation::Exposure(16.0),
                LiveOperation::Exposure(-16.0),
                LiveOperation::Exposure(32.0),
                LiveOperation::Exposure(-32.0),
            ],
            1.0f32,
        ),
    ] {
        session
            .transact(Edit::new(
                "Finite working HDR",
                [Change::SetOperations { node, operations }],
            ))
            .unwrap();
        let frame = renderer.render(&session.snapshot(), request).unwrap();
        assert_eq!(working(&frame)[0], [expected, expected, expected, 1.0]);
    }
    session
        .transact(Edit::new(
            "Refuse overflowing HDR",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(32.0); 5],
            }],
        ))
        .unwrap();
    assert_eq!(
        renderer.render(&session.snapshot(), request).unwrap_err(),
        Error::ColorRange {
            node,
            operation: "exposure"
        }
    );
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn working_mask_and_group_isolation_match_independent_linear_interior_oracles() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let mut isolated = LayerSpec::new("Isolated exposure and opacity", Content::Group);
    isolated.opacity = 0.5;
    isolated.operations = vec![
        LiveOperation::Exposure(2.0),
        LiveOperation::Crop(Rect::new(0.0, 0.0, 1.0, 1.0)),
    ];
    let isolated = add(&mut session, parent, isolated);
    import(
        &mut session,
        Parent::Layer(isolated),
        asset(1, 1, vec![0, 0, 255, 255]),
        Transform::IDENTITY,
    );
    import(
        &mut session,
        Parent::Layer(isolated),
        asset(1, 1, vec![255, 0, 0, 128]),
        Transform::IDENTITY,
    );
    let mut owner = LayerSpec::new("Hidden unrelated mask owner", Content::Group);
    owner.visible = false;
    owner.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 2000.0, 0.0]).unwrap();
    let owner = add(&mut session, parent, owner);
    let mask = import(
        &mut session,
        Parent::Layer(owner),
        asset(1, 1, vec![255, 0, 0, 128]),
        Transform::IDENTITY,
    );
    let snapshot = session.snapshot();
    let layer = snapshot.node(mask).unwrap();
    let mut spec = LayerSpec::new(layer.name(), layer.content().clone());
    spec.visible = false;
    spec.opacity = 0.5;
    spec.operations = vec![
        LiveOperation::Exposure(1.0),
        LiveOperation::Exposure(-1.0),
        LiveOperation::Crop(Rect::new(0.0, 0.0, 1.0, 1.0)),
    ];
    session
        .transact(Edit::new(
            "Hidden editable mask properties",
            [Change::SetLayer {
                node: mask,
                layer: spec,
            }],
        ))
        .unwrap();
    let renderer = Renderer::new(Limits::default()).unwrap();
    let request = request(&session, 1, 1, OutputFormat::LinearSrgbRgbaF32Premultiplied);
    let red = 128.0 / 255.0;
    for (mode, inverted, coverage) in [
        (MaskMode::Alpha, false, red * 0.5),
        (MaskMode::Luminance, false, 0.2126 * red * 0.5),
        (MaskMode::Luminance, true, 1.0 - 0.2126 * red * 0.5),
    ] {
        session
            .transact(Edit::new(
                "Mask interpretation",
                [Change::SetMask {
                    node: isolated,
                    mask: Some(Mask {
                        source: mask,
                        mode,
                        inverted,
                    }),
                }],
            ))
            .unwrap();
        let frame = renderer.render(&session.snapshot(), request).unwrap();
        // Blue under translucent red, then isolated +2EV, target-local mask,
        // finally group opacity. All arithmetic below is independent f64.
        assert_close(
            working(&frame)[0],
            [
                4.0 * red * coverage * 0.5,
                0.0,
                4.0 * (1.0 - red) * coverage * 0.5,
                coverage * 0.5,
            ],
            2e-7,
        );
    }
}

#[test]
fn working_bilinear_border_is_premultiplied_and_crop_undo_preserves_originals() {
    let mut session = session(4, 1);
    let parent = page(&session);
    let original = asset(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]);
    let pointer = original.pixels().as_ptr();
    let node = import(
        &mut session,
        parent,
        Arc::clone(&original),
        Transform::new([2.0, 0.0, 0.0, 1.0, 0.0, 0.0]).unwrap(),
    );
    session
        .transact(Edit::new(
            "Live highlight",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(2.0)],
            }],
        ))
        .unwrap();
    let renderer = Renderer::new(Limits::default()).unwrap();
    let request = request(&session, 4, 1, OutputFormat::LinearSrgbRgbaF32Premultiplied);
    let before = renderer.render(&session.snapshot(), request).unwrap();
    assert_eq!(
        working(&before),
        [
            [3.0, 0.0, 0.0, 0.75],
            [3.0, 0.0, 0.0, 0.75],
            [1.0, 0.0, 0.0, 0.25],
            [0.0; 4]
        ]
    );
    session
        .transact(Edit::new(
            "Non-destructive local crop",
            [Change::SetOperations {
                node,
                operations: vec![
                    LiveOperation::Exposure(2.0),
                    LiveOperation::Crop(Rect::new(0.0, 0.0, 1.0, 1.0)),
                ],
            }],
        ))
        .unwrap();
    let cropped = renderer.render(&session.snapshot(), request).unwrap();
    assert_eq!(working(&cropped)[0], working(&before)[0]);
    assert_eq!(working(&cropped)[1], working(&before)[1]);
    assert_eq!(working(&cropped)[2], [0.0; 4]);
    session.history(History::Undo).unwrap();
    let restored = renderer.render(&session.snapshot(), request).unwrap();
    for (actual, expected) in working(&restored).iter().zip(working(&before)) {
        assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
    }
    assert_eq!(original.pixels(), [255, 0, 0, 255, 0, 0, 255, 0]);
    assert_eq!(original.pixels().as_ptr(), pointer);
}

#[test]
fn four_and_sixteen_byte_outputs_share_exact_admission_and_arc_lifetimes() {
    let session = session(1, 1);
    let snapshot = session.snapshot();
    let rgba8 = request(&session, 1, 1, OutputFormat::SrgbRgba8Straight);
    let float = rgba8
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
        .unwrap();
    // The unchanged conservative scratch reservation is 107154 bytes here.
    let too_small = Renderer::new(Limits {
        max_bytes: 107_169,
        tile_size: 1,
        ..Limits::default()
    })
    .unwrap();
    assert!(matches!(
        too_small.render(&snapshot, float),
        Err(Error::BudgetExceeded {
            requested: 107_170,
            ..
        })
    ));
    assert_eq!(too_small.allocated_bytes(), 0);
    let renderer = Renderer::new(Limits {
        max_bytes: 107_170,
        tile_size: 1,
        ..Limits::default()
    })
    .unwrap();
    let bytes = renderer.render(&snapshot, rgba8).unwrap();
    assert_eq!(renderer.allocated_bytes(), 4);
    assert!(matches!(
        renderer.render(&snapshot, float),
        Err(Error::BudgetExceeded { used: 4, .. })
    ));
    assert_eq!(renderer.allocated_bytes(), 4);
    drop(bytes);
    let frame = Arc::new(renderer.render(&snapshot, float).unwrap());
    let held = Arc::clone(&frame);
    let clone = renderer.clone();
    assert_eq!(clone.allocated_bytes(), 16);
    assert_eq!(working(&frame).as_ptr(), working(&held).as_ptr());
    drop(frame);
    assert_eq!(clone.allocated_bytes(), 16);
    assert!(matches!(
        clone.render(&snapshot, rgba8),
        Err(Error::BudgetExceeded { used: 16, .. })
    ));
    drop(renderer);
    assert_eq!(clone.allocated_bytes(), 16);
    drop(held);
    assert_eq!(clone.allocated_bytes(), 0);
}

#[test]
fn concurrent_mixed_formats_and_float_cancellation_release_every_owner() {
    let session = session(1, 1);
    let snapshot = session.snapshot();
    let rgba8 = request(&session, 1, 1, OutputFormat::SrgbRgba8Straight);
    let float = rgba8
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
        .unwrap();
    let renderer = Renderer::new(Limits {
        max_bytes: 107_170,
        tile_size: 1,
        ..Limits::default()
    })
    .unwrap();
    let start = Arc::new(Barrier::new(3));
    let finish = Arc::new(Barrier::new(3));
    let jobs: Vec<_> = [rgba8, float]
        .into_iter()
        .map(|request| {
            let renderer = renderer.clone();
            let snapshot = snapshot.clone();
            let start = Arc::clone(&start);
            let finish = Arc::clone(&finish);
            std::thread::spawn(move || {
                start.wait();
                let result = renderer.render(&snapshot, request);
                finish.wait();
                result
            })
        })
        .collect();
    start.wait();
    finish.wait();
    let results: Vec<_> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Err(Error::BudgetExceeded { .. })))
    );
    let owned: usize = results
        .iter()
        .filter_map(|result| result.as_ref().ok())
        .map(Frame::byte_len)
        .sum();
    assert_eq!(renderer.allocated_bytes(), owned);
    drop(results);
    assert_eq!(renderer.allocated_bytes(), 0);
    assert_eq!(
        renderer
            .render_with_cancel(&snapshot, float, || renderer.allocated_bytes() > 0)
            .unwrap_err(),
        Error::Canceled
    );
    assert_eq!(renderer.allocated_bytes(), 0);
    let completed = renderer.render(&snapshot, float).unwrap();
    assert_eq!(completed.byte_len(), 16);
    drop(completed);
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn format_selection_refuses_sixteen_byte_arithmetic_overflow_without_allocation() {
    let session = session(1, 1);
    let (width, height) = if usize::BITS == 64 {
        (u32::MAX, u32::MAX / 4)
    } else {
        (u32::MAX / 8, 1)
    };
    let request = request(&session, width, height, OutputFormat::SrgbRgba8Straight);
    assert_eq!(
        request
            .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
            .unwrap_err(),
        Error::Overflow("output bytes")
    );
}

#[test]
fn subnormal_positive_alpha_carries_finite_hdr_rgb_and_rgba8_zero_alpha_is_canonical() {
    for (alpha, operations, expected_rgb) in [
        (
            2.0f64.powi(-130),
            vec![LiveOperation::Exposure(32.0); 4],
            0.25f32,
        ),
        (1e-4f64, Vec::new(), 1e-4f32),
    ] {
        let mut session = session(1, 1);
        let parent = page(&session);
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((1.0, 0.0));
        path.line_to((1.0, 1.0));
        path.line_to((0.0, 1.0));
        path.close_path();
        let mut layer = LayerSpec::new(
            "Tiny-alpha finite HDR",
            Content::Vector(
                VectorContent::new(path, Color::new(1.0, 1.0, 1.0, alpha).unwrap()).unwrap(),
            ),
        );
        layer.operations = operations;
        add(&mut session, parent, layer);
        let renderer = Renderer::new(Limits::default()).unwrap();
        let float = renderer
            .render(
                &session.snapshot(),
                request(&session, 1, 1, OutputFormat::LinearSrgbRgbaF32Premultiplied),
            )
            .unwrap();
        // Exact power-of-two source/gains establish the subnormal oracle without
        // a float formula mirroring the compositor. Positive alpha remains owned.
        assert_eq!(
            working(&float)[0].map(f32::to_bits),
            [expected_rgb, expected_rgb, expected_rgb, alpha as f32].map(f32::to_bits)
        );
        assert!(working(&float)[0][3] > 0.0);
        let rgba8 = renderer
            .render(
                &session.snapshot(),
                request(&session, 1, 1, OutputFormat::SrgbRgba8Straight),
            )
            .unwrap();
        assert_eq!(rgba8.rgba8().unwrap(), [0; 4]);
    }
}
