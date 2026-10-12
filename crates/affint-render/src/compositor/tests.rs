//! Optimized and original evaluation share all numerical primitives. The private
//! switch disables only support rejection and full-rectangle coverage proofs.

use std::sync::Arc;

use affint_document::{
    Change, Color, Content, Edit, LayerSpec, Limits as DocumentLimits, LiveOperation, Mask,
    MaskMode, NodeId, PageSize, Parent, PixelSize, RasterAsset, Session, Transform, VectorContent,
};
use kurbo::{BezPath, Rect};

use super::{Optimizations, render_impl};
use crate::{Error, Frame, FramePixels, Limits, OutputFormat, RenderRequest, Renderer, View};

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
            "Original fixture layer",
            [Change::InsertLayer {
                parent,
                index,
                layer,
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn source(
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
                name: "Original RGBA8".to_owned(),
                asset,
                transform,
            }],
        ))
        .unwrap()
        .nodes[0]
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
    path.move_to((-252.375, 0.25));
    path.curve_to((-78.0, 19.0), (37.0, -15.0), (268.75, 8.125));
    path.line_to((268.75, 24.0));
    path.line_to((-252.375, 24.0));
    path.close_path();
    Content::Vector(VectorContent::new(path, Color::new(0.1, 0.4, 0.9, 0.83).unwrap()).unwrap())
}

fn request(session: &Session, width: u32, height: u32, view: View) -> RenderRequest {
    RenderRequest::new(session.snapshot().page_order()[0], width, height, view).unwrap()
}

fn pixel(frame: &Frame, x: u32, y: u32) -> &[u8] {
    let offset = ((y * frame.width() + x) * 4) as usize;
    &frame.rgba8().unwrap()[offset..offset + 4]
}

fn compare(session: &Session, request: RenderRequest) {
    let snapshot = session.snapshot();
    for format in [
        OutputFormat::SrgbRgba8Straight,
        OutputFormat::LinearSrgbRgbaF32Premultiplied,
    ] {
        let request = request.with_format(format).unwrap();
        let original = Renderer::new(Limits::default()).unwrap();
        let expected = render_impl(
            &original,
            &snapshot,
            request,
            &|| false,
            Optimizations::DISABLED,
        )
        .unwrap();
        for tile_size in [17, 256, 2048] {
            let renderer = Renderer::new(Limits {
                tile_size,
                ..Limits::default()
            })
            .unwrap();
            let actual = renderer.render(&snapshot, request).unwrap();
            match (actual.pixels(), expected.pixels()) {
                (
                    FramePixels::SrgbRgba8Straight(actual),
                    FramePixels::SrgbRgba8Straight(expected),
                ) => {
                    assert_eq!(actual, expected, "optimized tile {tile_size}");
                }
                (
                    FramePixels::LinearSrgbRgbaF32Premultiplied(actual),
                    FramePixels::LinearSrgbRgbaF32Premultiplied(expected),
                ) => {
                    assert_eq!(actual.len(), expected.len());
                    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                        assert_eq!(
                            actual.map(f32::to_bits),
                            expected.map(f32::to_bits),
                            "working pixel {index}, tile {tile_size}"
                        );
                    }
                }
                _ => panic!("output format does not match request"),
            }
            assert_eq!(renderer.allocated_bytes(), actual.byte_len());
            drop(actual);
            assert_eq!(renderer.allocated_bytes(), 0);
        }
        assert_eq!(original.allocated_bytes(), expected.byte_len());
        drop(expected);
        assert_eq!(original.allocated_bytes(), 0);
    }
}

#[test]
fn curves_group_crops_and_hidden_local_masks_match_forced_original_evaluation() {
    let mut session = session(517, 21);
    let parent = page(&session);
    let asset = Arc::new(
        RasterAsset::new(
            PixelSize::new(5, 3).unwrap(),
            (0u8..15)
                .flat_map(|i| {
                    [
                        i * 13,
                        255 - i * 11,
                        i * 7,
                        if i % 4 == 0 { 0 } else { 127 + i * 8 },
                    ]
                })
                .collect(),
        )
        .unwrap(),
    );
    let original_pixels = asset.pixels().to_vec();
    let original_pointer = asset.pixels().as_ptr();
    source(
        &mut session,
        parent,
        Arc::clone(&asset),
        Transform::new([101.125, 0.003, -0.04, 6.0, -0.35, 0.25]).unwrap(),
    );
    let mut group = LayerSpec::new("Placed isolated group", Content::Group);
    group.transform = Transform::new([1.0, 0.012, 0.035, 1.0, 251.25, 0.125]).unwrap();
    group.opacity = 0.71;
    group.operations = vec![
        LiveOperation::Crop(Rect::new(-1000.0, -1000.0, 1000.0, 1000.0)),
        LiveOperation::Exposure(2.0),
    ];
    let group = add(&mut session, parent, group);
    add(
        &mut session,
        Parent::Layer(group),
        LayerSpec::new("Cross-block editable cubic", curve()),
    );
    let mut nested = LayerSpec::new("Mirrored nested group", Content::Group);
    nested.transform = Transform::new([-1.0, 0.0, 0.0, 1.0, 259.5, 0.0]).unwrap();
    nested.operations = vec![LiveOperation::Crop(Rect::new(-5.0, -5.0, 520.0, 18.875))];
    let nested = add(&mut session, Parent::Layer(group), nested);
    add(
        &mut session,
        Parent::Layer(nested),
        LayerSpec::new(
            "Subpixel strip",
            rectangle(Rect::new(0.0, 3.3125, 517.0, 3.6875), [0.9, 0.2, 0.1, 0.6]),
        ),
    );
    let mut offscreen = LayerSpec::new("Wholly offscreen cubic", curve());
    offscreen.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 2048.0, 0.0]).unwrap();
    add(&mut session, parent, offscreen);
    let mut owner = LayerSpec::new("Unrelated hidden owner", Content::Group);
    owner.visible = false;
    owner.transform = Transform::new([0.7, 0.4, -0.2, 1.2, 5000.0, 5000.0]).unwrap();
    let owner = add(&mut session, parent, owner);
    let mut mask = LayerSpec::new(
        "Hidden target-local mask",
        rectangle(Rect::new(-0.1, -1.0, 250.625, 20.0), [0.7, 0.2, 0.4, 0.8]),
    );
    mask.visible = false;
    mask.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 1.25, 0.375]).unwrap();
    mask.opacity = 0.43;
    mask.operations = vec![
        LiveOperation::Exposure(2.0),
        LiveOperation::Exposure(-2.0),
        LiveOperation::Crop(Rect::new(0.0, 0.0, 260.0, 20.0)),
    ];
    let mask = add(&mut session, Parent::Layer(owner), mask);
    let mut target = LayerSpec::new(
        "Inverted masked target",
        rectangle(Rect::new(-200.0, -10.0, 600.0, 25.0), [0.3, 0.8, 0.1, 0.17]),
    );
    target.transform = Transform::new([0.99, 0.01, -0.03, 1.0, 0.3, 0.3]).unwrap();
    target.mask = Some(Mask {
        source: mask,
        mode: MaskMode::Luminance,
        inverted: true,
    });
    add(&mut session, parent, target);
    for transform in [
        Transform::IDENTITY,
        Transform::new([1.0, 0.0, 0.0, 1.0, -0.03125, 0.125]).unwrap(),
    ] {
        compare(
            &session,
            request(&session, 517, 21, View::Affine(transform)),
        );
    }
    let hdr = Renderer::new(Limits::default())
        .unwrap()
        .render(
            &session.snapshot(),
            request(&session, 517, 21, View::Affine(Transform::IDENTITY))
                .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)
                .unwrap(),
        )
        .unwrap();
    assert!(
        matches!(hdr.pixels(), FramePixels::LinearSrgbRgbaF32Premultiplied(pixels)
        if pixels.iter().any(|pixel| pixel[..3].iter().any(|channel| *channel > 1.0)))
    );
    assert_eq!(asset.pixels(), original_pixels);
    assert_eq!(asset.pixels().as_ptr(), original_pointer);
}

#[test]
fn raster_half_pixel_border_survives_rejection_across_canonical_edges() {
    let mut session = session(517, 1);
    let parent = page(&session);
    let asset =
        Arc::new(RasterAsset::new(PixelSize::new(1, 1).unwrap(), vec![255, 0, 0, 255]).unwrap());
    source(
        &mut session,
        parent,
        asset,
        Transform::new([1.0, 0.0, 0.0, 1.0, 255.75, 0.0]).unwrap(),
    );
    let request = request(&session, 517, 1, View::Affine(Transform::IDENTITY));
    compare(&session, request);
    let renderer = Renderer::new(Limits::default()).unwrap();
    let frame = renderer.render(&session.snapshot(), request).unwrap();
    assert_eq!(pixel(&frame, 254, 0), [0, 0, 0, 0]);
    assert_eq!(pixel(&frame, 255, 0), [255, 0, 0, 64]);
    assert_eq!(pixel(&frame, 256, 0), [255, 0, 0, 191]);
    assert_eq!(pixel(&frame, 257, 0), [0, 0, 0, 0]);
}

#[test]
fn full_axis_rectangles_and_fractional_page_edges_match_scanned_coverage() {
    let mut session = session(517, 9);
    let parent = page(&session);
    let mut layer = LayerSpec::new(
        "Full content with exact crop",
        rectangle(Rect::new(-2.0, -2.0, 519.0, 11.0), [1.0; 4]),
    );
    layer.operations = vec![LiveOperation::Crop(Rect::new(0.0, 0.0, 517.0, 9.0))];
    add(&mut session, parent, layer);
    for transform in [
        Transform::IDENTITY,
        Transform::new([-1.0, 0.0, 0.0, -1.0, 517.0, 9.0]).unwrap(),
        Transform::new([1.0, 0.0, 0.0, 1.0, 0.03125, -0.0625]).unwrap(),
    ] {
        compare(&session, request(&session, 517, 9, View::Affine(transform)));
    }
    let renderer = Renderer::new(Limits::default()).unwrap();
    let frame = renderer
        .render(
            &session.snapshot(),
            request(&session, 517, 9, View::Affine(Transform::IDENTITY)),
        )
        .unwrap();
    assert!(frame.rgba8().unwrap().iter().all(|byte| *byte == 255));
}

#[test]
fn rotated_page_bbox_does_not_replace_polygon_coverage() {
    let mut session = session(64, 64);
    let parent = page(&session);
    add(
        &mut session,
        parent,
        LayerSpec::new(
            "White page",
            rectangle(Rect::new(0.0, 0.0, 64.0, 64.0), [1.0; 4]),
        ),
    );
    let rotation = std::f64::consts::FRAC_1_SQRT_2;
    let view = View::Affine(
        Transform::new([
            rotation,
            rotation,
            -rotation,
            rotation,
            32.0,
            32.0 - 64.0 * rotation,
        ])
        .unwrap(),
    );
    let request = request(&session, 64, 64, view);
    compare(&session, request);
    let frame = Renderer::new(Limits::default())
        .unwrap()
        .render(&session.snapshot(), request)
        .unwrap();
    assert_eq!(pixel(&frame, 0, 0), [0, 0, 0, 0]);
    assert_eq!(pixel(&frame, 32, 32), [255; 4]);
}

#[test]
fn rejected_hidden_mask_is_zero_before_alpha_or_luminance_inversion() {
    let mut session = session(4, 1);
    let parent = page(&session);
    let mut owner = LayerSpec::new("Hidden translated owner", Content::Group);
    owner.visible = false;
    owner.transform = Transform::new([1.0, 0.0, 0.0, 1.0, -1024.0, 0.0]).unwrap();
    let owner = add(&mut session, parent, owner);
    let mut mask = LayerSpec::new(
        "Hidden offscreen mask",
        rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0; 4]),
    );
    mask.visible = false;
    mask.transform = Transform::new([2.0, 0.0, 0.0, 1.0, 1024.0, 0.0]).unwrap();
    mask.opacity = 0.25;
    let mask = add(&mut session, Parent::Layer(owner), mask);
    let mut target = LayerSpec::new(
        "Mirrored green target",
        rectangle(Rect::new(0.0, 0.0, 4.0, 1.0), [0.0, 1.0, 0.0, 1.0]),
    );
    target.transform = Transform::new([-1.0, 0.0, 0.0, 1.0, 4.0, 0.0]).unwrap();
    let target = add(&mut session, parent, target);
    let renderer = Renderer::new(Limits::default()).unwrap();
    for mode in [MaskMode::Alpha, MaskMode::Luminance] {
        for inverted in [false, true] {
            session
                .transact(Edit::new(
                    "Mask coverage",
                    [Change::SetMask {
                        node: target,
                        mask: Some(Mask {
                            source: mask,
                            mode,
                            inverted,
                        }),
                    }],
                ))
                .unwrap();
            let request = request(&session, 4, 1, View::Affine(Transform::IDENTITY));
            compare(&session, request);
            let frame = renderer.render(&session.snapshot(), request).unwrap();
            let expected = if inverted { [0, 255, 0, 255] } else { [0; 4] };
            for x in 0..4 {
                assert_eq!(pixel(&frame, x, 0), expected);
            }
        }
    }
}

#[test]
fn rejected_target_still_checks_its_target_local_mask_dependency() {
    let mut session = session(8, 1);
    let parent = page(&session);
    let mut mask = LayerSpec::new(
        "Visible-in-target mask overflow",
        rectangle(Rect::new(0.0, 0.0, 8.0, 1.0), [1.0; 4]),
    );
    mask.visible = false;
    mask.transform = Transform::new([1.0, 0.0, 0.0, 1.0, -1024.0, 0.0]).unwrap();
    mask.operations = vec![LiveOperation::Exposure(32.0); 5];
    let mask = add(&mut session, parent, mask);
    let mut target = LayerSpec::new(
        "Empty offscreen target",
        rectangle(Rect::new(0.0, 0.0, 8.0, 1.0), [1.0; 4]),
    );
    target.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 1024.0, 0.0]).unwrap();
    target.mask = Some(Mask {
        source: mask,
        mode: MaskMode::Alpha,
        inverted: true,
    });
    add(&mut session, parent, target);
    let renderer = Renderer::new(Limits::default()).unwrap();
    let snapshot = session.snapshot();
    let request = request(&session, 8, 1, View::Affine(Transform::IDENTITY));
    for flags in [Optimizations::ENABLED, Optimizations::DISABLED] {
        assert_eq!(
            render_impl(&renderer, &snapshot, request, &|| false, flags).unwrap_err(),
            Error::ColorRange {
                node: mask,
                operation: "exposure"
            }
        );
        assert_eq!(renderer.allocated_bytes(), 0);
    }
}

#[test]
fn rejected_geometry_preserves_range_errors_and_releases_its_bounds_owner() {
    let mut session = session(8, 1);
    let parent = page(&session);
    let mut layer = LayerSpec::new(
        "Invalid distant crop",
        rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0; 4]),
    );
    layer.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 1024.0, 0.0]).unwrap();
    layer.operations = vec![LiveOperation::Crop(Rect::new(
        2_000_000.0,
        0.0,
        2_000_001.0,
        1.0,
    ))];
    let node = add(&mut session, parent, layer);
    let renderer = Renderer::new(Limits::default()).unwrap();
    let snapshot = session.snapshot();
    let request = request(&session, 8, 1, View::Affine(Transform::IDENTITY));
    for flags in [Optimizations::ENABLED, Optimizations::DISABLED] {
        assert_eq!(
            render_impl(&renderer, &snapshot, request, &|| false, flags).unwrap_err(),
            Error::GeometryRange {
                node: Some(node),
                resource: "device coverage point"
            }
        );
        assert_eq!(renderer.allocated_bytes(), 0);
    }
}

#[test]
fn offscreen_rejection_eliminates_coverage_work_but_retains_admission_and_cancellation() {
    let mut session = session(257, 3);
    let parent = page(&session);
    let mut layer = LayerSpec::new("Offscreen curve", curve());
    layer.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 2048.0, 0.0]).unwrap();
    add(&mut session, parent, layer);
    let snapshot = session.snapshot();
    let request = request(&session, 257, 3, View::Affine(Transform::IDENTITY));
    let renderer = Renderer::new(Limits {
        max_work: 10_000,
        ..Limits::default()
    })
    .unwrap();
    let frame = renderer.render(&snapshot, request).unwrap();
    assert!(frame.rgba8().unwrap().iter().all(|byte| *byte == 0));
    assert_eq!(renderer.allocated_bytes(), frame.rgba8().unwrap().len());
    drop(frame);
    assert_eq!(renderer.allocated_bytes(), 0);
    assert!(matches!(
        render_impl(
            &renderer,
            &snapshot,
            request,
            &|| false,
            Optimizations::DISABLED
        ),
        Err(Error::LimitExceeded {
            resource: "render work",
            ..
        })
    ));
    assert_eq!(renderer.allocated_bytes(), 0);
    // Cancel only after output/scratch admission, not when the small bounds owner
    // is allocated in preflight. All owners must release on this path as well.
    assert_eq!(
        renderer
            .render_with_cancel(&snapshot, request, || renderer.allocated_bytes()
                > super::FLATTEN_WORKSPACE + 1024)
            .unwrap_err(),
        Error::Canceled
    );
    assert_eq!(renderer.allocated_bytes(), 0);
    let limited = Renderer::new(Limits {
        max_bytes: 128,
        ..Limits::default()
    })
    .unwrap();
    assert!(matches!(
        limited.render(&snapshot, request),
        Err(Error::BudgetExceeded { .. })
    ));
    assert_eq!(limited.allocated_bytes(), 0);
}

#[test]
fn offscreen_fill_and_opacity_conversion_are_still_refused() {
    for opacity in [false, true] {
        let mut session = session(1, 1);
        let parent = page(&session);
        let color = if opacity {
            [1.0; 4]
        } else {
            [1e-100, 0.0, 0.0, 1.0]
        };
        let mut layer = LayerSpec::new(
            "Unrepresentable offscreen scalar",
            rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), color),
        );
        layer.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 1024.0, 0.0]).unwrap();
        if opacity {
            layer.opacity = 1e-100;
        }
        let node = add(&mut session, parent, layer);
        let renderer = Renderer::new(Limits::default()).unwrap();
        let request = request(&session, 1, 1, View::Affine(Transform::IDENTITY));
        assert_eq!(
            renderer.render(&session.snapshot(), request).unwrap_err(),
            Error::ColorRange {
                node,
                operation: "color/opacity conversion"
            }
        );
        assert_eq!(renderer.allocated_bytes(), 0);
    }
}
