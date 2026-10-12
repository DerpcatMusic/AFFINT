use std::sync::{Arc, Barrier};

use affint_document::{
    Change, Color, Content, Edit, History, LayerSpec, Limits as DocumentLimits, LiveOperation,
    Mask, MaskMode, NodeId, PageSize, Parent, PixelSize, RasterAsset, Session, Transform,
    VectorContent,
};
use affint_render::{Error, Frame, Limits, RenderRequest, Renderer, View};
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

fn add(session: &mut Session, parent: Parent, layer: LayerSpec) -> NodeId {
    let snapshot = session.snapshot();
    let index = match parent {
        Parent::Page(id) => snapshot.page(id).unwrap().roots().len(),
        Parent::Layer(id) => snapshot.node(id).unwrap().children().len(),
    };
    session
        .transact(Edit::new(
            "Insert artwork",
            [Change::InsertLayer {
                parent,
                index,
                layer,
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

fn source(
    session: &mut Session,
    parent: Parent,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    transform: Transform,
) -> NodeId {
    let snapshot = session.snapshot();
    let index = match parent {
        Parent::Page(id) => snapshot.page(id).unwrap().roots().len(),
        Parent::Layer(id) => snapshot.node(id).unwrap().children().len(),
    };
    let asset = Arc::new(RasterAsset::new(PixelSize::new(width, height).unwrap(), bytes).unwrap());
    session
        .transact(Edit::new(
            "Import original pixels",
            [Change::InsertRaster {
                parent,
                index,
                name: "Original pixels".to_owned(),
                asset,
                transform,
            }],
        ))
        .unwrap()
        .nodes[0]
}

fn properties(session: &mut Session, id: NodeId, edit: impl FnOnce(&mut LayerSpec)) {
    let snapshot = session.snapshot();
    let layer = snapshot.node(id).unwrap();
    let mut spec = LayerSpec::new(layer.name(), layer.content().clone());
    spec.transform = layer.transform();
    spec.opacity = layer.opacity();
    spec.visible = layer.visible();
    spec.mask = layer.mask();
    spec.operations = layer.operations().to_vec();
    edit(&mut spec);
    session
        .transact(Edit::new(
            "Change editable properties",
            [Change::SetLayer {
                node: id,
                layer: spec,
            }],
        ))
        .unwrap();
}

fn render(renderer: &Renderer, session: &Session, width: u32, height: u32) -> Frame {
    let snapshot = session.snapshot();
    let page = snapshot.page_order()[0];
    let size = snapshot.page(page).unwrap().size();
    renderer
        .render(
            &snapshot,
            RenderRequest::new(
                page,
                width,
                height,
                View::Region(Rect::new(0.0, 0.0, size.width(), size.height())),
            )
            .unwrap(),
        )
        .unwrap()
}

fn renderer() -> Renderer {
    Renderer::new(Limits::default()).unwrap()
}

fn pixel(frame: &Frame, x: u32, y: u32) -> &[u8] {
    let offset = ((y * frame.width() + x) * 4) as usize;
    &frame.rgba8().unwrap()[offset..offset + 4]
}

#[test]
fn opaque_blue_under_half_red_uses_linear_light_source_over() {
    let mut session = session(1, 1);
    let parent = page(&session);
    source(
        &mut session,
        parent,
        1,
        1,
        vec![0, 0, 255, 255],
        Transform::IDENTITY,
    );
    source(
        &mut session,
        parent,
        1,
        1,
        vec![255, 0, 0, 128],
        Transform::IDENTITY,
    );
    // Independent IEC sRGB/Porter-Duff golden: encoded-channel blending would
    // produce [128, 0, 127], which is visibly and numerically wrong.
    assert_eq!(
        render(&renderer(), &session, 1, 1).rgba8().unwrap(),
        &[188, 0, 187, 255]
    );
}

#[test]
fn identity_sampling_roundtrips_every_encoded_channel_even_at_low_alpha() {
    let mut session = session(256, 1);
    let parent = page(&session);
    let bytes: Vec<u8> = (0u16..256)
        .flat_map(|value| {
            [
                value as u8,
                (255 - value) as u8,
                ((value * 73) % 256) as u8,
                (1 + value % 255) as u8,
            ]
        })
        .collect();
    source(
        &mut session,
        parent,
        256,
        1,
        bytes.clone(),
        Transform::IDENTITY,
    );
    assert_eq!(
        render(&renderer(), &session, 256, 1).rgba8().unwrap(),
        bytes
    );
}

#[test]
fn transparent_blue_does_not_color_bilinear_red_edges_and_border_is_transparent() {
    let mut session = session(4, 1);
    let parent = page(&session);
    source(
        &mut session,
        parent,
        2,
        1,
        vec![255, 0, 0, 255, 0, 0, 255, 0],
        Transform::new([2.0, 0.0, 0.0, 1.0, 0.0, 0.0]).unwrap(),
    );
    assert_eq!(
        render(&renderer(), &session, 4, 1).rgba8().unwrap(),
        &[255, 0, 0, 191, 255, 0, 0, 191, 255, 0, 0, 64, 0, 0, 0, 0]
    );
}

#[test]
fn raster_interpolation_happens_in_linear_light() {
    let mut session = session(4, 1);
    let parent = page(&session);
    source(
        &mut session,
        parent,
        2,
        1,
        vec![0, 0, 0, 255, 255, 255, 255, 255],
        Transform::new([2.0, 0.0, 0.0, 1.0, 0.0, 0.0]).unwrap(),
    );
    let frame = render(&renderer(), &session, 4, 1);
    // A quarter of unit linear white encodes to 137, rather than 64.
    assert_eq!(pixel(&frame, 1, 0), &[137, 137, 137, 255]);
    assert_eq!(pixel(&frame, 2, 0), &[225, 225, 225, 255]);
}

#[test]
fn nested_group_affines_preserve_child_local_geometry() {
    let mut session = session(6, 5);
    let parent = page(&session);
    let mut outer = LayerSpec::new("Translated", Content::Group);
    outer.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 4.0, 1.0]).unwrap();
    let outer = add(&mut session, parent, outer);
    let mut inner = LayerSpec::new("Quarter turn", Content::Group);
    inner.transform = Transform::new([0.0, 1.0, -1.0, 0.0, 0.0, 0.0]).unwrap();
    let inner = add(&mut session, Parent::Layer(outer), inner);
    add(
        &mut session,
        Parent::Layer(inner),
        LayerSpec::new(
            "Red",
            rectangle(Rect::new(0.0, 0.0, 2.0, 1.0), [1.0, 0.0, 0.0, 1.0]),
        ),
    );
    let frame = render(&renderer(), &session, 6, 5);
    let mut expected = vec![0; 6 * 5 * 4];
    for y in [1usize, 2] {
        expected[(y * 6 + 3) * 4..(y * 6 + 3) * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
    }
    assert_eq!(frame.rgba8().unwrap(), expected);
}

#[test]
fn group_opacity_applies_after_isolated_child_composition() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let mut group = LayerSpec::new("Isolated half", Content::Group);
    group.opacity = 0.5;
    let group = add(&mut session, parent, group);
    for color in [[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]] {
        add(
            &mut session,
            Parent::Layer(group),
            LayerSpec::new(
                "Opaque child",
                rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), color),
            ),
        );
    }
    assert_eq!(
        render(&renderer(), &session, 1, 1).rgba8().unwrap(),
        &[0, 0, 255, 128]
    );
}

#[test]
fn hidden_alpha_and_luminance_masks_use_alpha_linear_luminance_opacity_and_inversion() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let target = add(
        &mut session,
        parent,
        LayerSpec::new(
            "Green target",
            rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [0.0, 1.0, 0.0, 1.0]),
        ),
    );
    let mask = source(
        &mut session,
        parent,
        1,
        1,
        vec![255, 0, 0, 128],
        Transform::IDENTITY,
    );
    properties(&mut session, mask, |spec| spec.visible = false);
    let renderer = renderer();
    for (mode, inverted, expected) in [
        (MaskMode::Alpha, false, 128),
        (MaskMode::Luminance, false, 27),
        (MaskMode::Luminance, true, 228),
    ] {
        session
            .transact(Edit::new(
                "Mask mode",
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
        assert_eq!(
            render(&renderer, &session, 1, 1).rgba8().unwrap(),
            &[0, 255, 0, expected]
        );
    }
    properties(&mut session, mask, |spec| spec.opacity = 0.5);
    session
        .transact(Edit::new(
            "Half mask alpha",
            [Change::SetMask {
                node: target,
                mask: Some(Mask {
                    source: mask,
                    mode: MaskMode::Alpha,
                    inverted: false,
                }),
            }],
        ))
        .unwrap();
    assert_eq!(
        render(&renderer, &session, 1, 1).rgba8().unwrap(),
        &[0, 255, 0, 64]
    );
    session
        .transact(Edit::new(
            "Half mask luminance",
            [Change::SetMask {
                node: target,
                mask: Some(Mask {
                    source: mask,
                    mode: MaskMode::Luminance,
                    inverted: false,
                }),
            }],
        ))
        .unwrap();
    assert_eq!(
        render(&renderer, &session, 1, 1).rgba8().unwrap(),
        &[0, 255, 0, 14]
    );
}

#[test]
fn mask_source_uses_target_local_coordinates_and_its_own_affine_crop_and_opacity() {
    let mut session = session(6, 1);
    let parent = page(&session);
    let mut target = LayerSpec::new(
        "Placed target",
        rectangle(Rect::new(0.0, 0.0, 2.0, 1.0), [0.0, 0.0, 1.0, 1.0]),
    );
    target.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 3.0, 0.0]).unwrap();
    let target = add(&mut session, parent, target);
    let mut owner = LayerSpec::new("Hidden unrelated mask owner", Content::Group);
    owner.visible = false;
    owner.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 100.0, 0.0]).unwrap();
    let owner = add(&mut session, parent, owner);
    let mut mask = LayerSpec::new(
        "Hidden local coverage",
        rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0, 1.0, 1.0, 1.0]),
    );
    mask.visible = false;
    mask.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 1.0, 0.0]).unwrap();
    mask.opacity = 0.5;
    mask.operations = vec![LiveOperation::Crop(Rect::new(0.0, 0.0, 0.5, 1.0))];
    let mask = add(&mut session, Parent::Layer(owner), mask);
    session
        .transact(Edit::new(
            "Use local mask",
            [Change::SetMask {
                node: target,
                mask: Some(Mask {
                    source: mask,
                    mode: MaskMode::Alpha,
                    inverted: false,
                }),
            }],
        ))
        .unwrap();
    let frame = render(&renderer(), &session, 6, 1);
    let mut expected = vec![0; 24];
    expected[16..20].copy_from_slice(&[0, 0, 255, 64]);
    assert_eq!(frame.rgba8().unwrap(), expected);
}

#[test]
fn mask_source_evaluates_its_own_mask_dependency() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let target = add(
        &mut session,
        parent,
        LayerSpec::new(
            "White target",
            rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0; 4]),
        ),
    );
    let mut mask = LayerSpec::new(
        "Half white",
        rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0; 4]),
    );
    mask.visible = false;
    mask.opacity = 0.5;
    let mask = add(&mut session, parent, mask);
    let mut submask = LayerSpec::new(
        "Half coverage",
        rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0, 1.0, 1.0, 0.5]),
    );
    submask.visible = false;
    let submask = add(&mut session, parent, submask);
    session
        .transact(Edit::new(
            "Nested editable masks",
            [
                Change::SetMask {
                    node: mask,
                    mask: Some(Mask {
                        source: submask,
                        mode: MaskMode::Alpha,
                        inverted: false,
                    }),
                },
                Change::SetMask {
                    node: target,
                    mask: Some(Mask {
                        source: mask,
                        mode: MaskMode::Alpha,
                        inverted: false,
                    }),
                },
            ],
        ))
        .unwrap();
    assert_eq!(
        render(&renderer(), &session, 1, 1).rgba8().unwrap(),
        &[255, 255, 255, 64]
    );
}

#[test]
fn sequential_exposure_recovers_highlights_before_final_clipping() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let node = source(
        &mut session,
        parent,
        1,
        1,
        vec![200, 160, 80, 127],
        Transform::IDENTITY,
    );
    let renderer = renderer();
    session
        .transact(Edit::new(
            "Recover exposed source",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(4.0), LiveOperation::Exposure(-4.0)],
            }],
        ))
        .unwrap();
    assert_eq!(
        render(&renderer, &session, 1, 1).rgba8().unwrap(),
        &[200, 160, 80, 127]
    );
    session
        .transact(Edit::new(
            "Clip only for display",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(4.0)],
            }],
        ))
        .unwrap();
    assert_eq!(
        render(&renderer, &session, 1, 1).rgba8().unwrap(),
        &[255, 255, 255, 127]
    );
}

#[test]
fn live_crop_undo_restores_original_pixels_and_render_without_reimport() {
    let mut session = session(2, 1);
    let parent = page(&session);
    let node = source(
        &mut session,
        parent,
        2,
        1,
        vec![255, 0, 0, 255, 0, 255, 0, 255],
        Transform::IDENTITY,
    );
    let original = session.snapshot();
    let original_asset = original.assets().next().unwrap().1;
    let pointer = original_asset.pixels().as_ptr();
    session
        .transact(Edit::new(
            "Editable crop",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Crop(Rect::new(1.0, 0.0, 2.0, 1.0))],
            }],
        ))
        .unwrap();
    let renderer = renderer();
    assert_eq!(
        render(&renderer, &session, 2, 1).rgba8().unwrap(),
        &[0, 0, 0, 0, 0, 255, 0, 255]
    );
    session.history(History::Undo).unwrap();
    assert_eq!(
        render(&renderer, &session, 2, 1).rgba8().unwrap(),
        original_asset.pixels()
    );
    session.history(History::Redo).unwrap();
    assert_eq!(
        render(&renderer, &session, 2, 1).rgba8().unwrap(),
        &[0, 0, 0, 0, 0, 255, 0, 255]
    );
    assert_eq!(
        session
            .snapshot()
            .assets()
            .next()
            .unwrap()
            .1
            .pixels()
            .as_ptr(),
        pointer
    );
}

#[test]
fn quadratic_and_cubic_paths_remain_editable_and_nonzero_fill_holes_are_preserved() {
    let mut session = session(8, 5);
    let parent = page(&session);
    let mut path = BezPath::new();
    path.move_to((0.0, 3.0));
    path.quad_to((2.0, -3.0), (4.0, 3.0));
    path.curve_to((5.0, 3.0), (7.0, 3.0), (8.0, 3.0));
    path.line_to((8.0, 5.0));
    path.line_to((0.0, 5.0));
    path.close_path();
    let content =
        VectorContent::new(path.clone(), Color::new(1.0, 0.0, 0.0, 1.0).unwrap()).unwrap();
    let node = add(
        &mut session,
        parent,
        LayerSpec::new("Curves", Content::Vector(content)),
    );
    let renderer = renderer();
    let frame = render(&renderer, &session, 8, 5);
    // The exact quadratic is y=3*(1-x/2)^2. Its covered area in pixel
    // (0,0) is 4/(3*sqrt(3))-3/4 =0.0198003589, rounding to alpha5.
    // The earlier coarse geometric sampling lattice missed this real edge.
    assert_eq!(pixel(&frame, 0, 0), &[255, 0, 0, 5]);
    assert_eq!(pixel(&frame, 7, 0), &[0, 0, 0, 0]);
    assert_eq!(pixel(&frame, 2, 3), &[255, 0, 0, 255]);
    assert_eq!(pixel(&frame, 7, 4), &[255, 0, 0, 255]);
    match session.snapshot().node(node).unwrap().content() {
        Content::Vector(vector) => assert_eq!(vector.path(), &path),
        _ => panic!("original vector changed type"),
    }
    let mut hole = BezPath::new();
    hole.move_to((0.0, 0.0));
    hole.line_to((4.0, 0.0));
    hole.line_to((4.0, 4.0));
    hole.line_to((0.0, 4.0));
    hole.close_path();
    hole.move_to((1.0, 1.0));
    hole.line_to((1.0, 3.0));
    hole.line_to((3.0, 3.0));
    hole.line_to((3.0, 1.0));
    hole.close_path();
    properties(&mut session, node, |spec| {
        spec.content = Content::Vector(
            VectorContent::new(hole, Color::new(0.0, 1.0, 0.0, 1.0).unwrap()).unwrap(),
        )
    });
    let frame = render(&renderer, &session, 8, 5);
    assert_eq!(pixel(&frame, 0, 0), &[0, 255, 0, 255]);
    for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
        assert_eq!(pixel(&frame, x, y), &[0, 0, 0, 0]);
    }
}

#[test]
fn explicit_view_affine_and_region_return_exact_dimensions_and_page_clipping() {
    let mut session = session(2, 1);
    let parent = page(&session);
    add(
        &mut session,
        parent,
        LayerSpec::new(
            "Overflowing vector",
            rectangle(Rect::new(-2.0, -2.0, 5.0, 5.0), [1.0; 4]),
        ),
    );
    let snapshot = session.snapshot();
    let page = snapshot.page_order()[0];
    let renderer = renderer();
    let request = RenderRequest::new(
        page,
        4,
        1,
        View::Affine(Transform::new([1.0, 0.0, 0.0, 1.0, 1.0, 0.0]).unwrap()),
    )
    .unwrap();
    let frame = renderer.render(&snapshot, request).unwrap();
    assert_eq!(frame.request(), request);
    assert_eq!(frame.revision(), snapshot.revision());
    assert_eq!(
        frame.rgba8().unwrap(),
        &[
            0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0
        ]
    );
    let request =
        RenderRequest::new(page, 1, 1, View::Region(Rect::new(1.0, 0.0, 2.0, 1.0))).unwrap();
    assert_eq!(
        renderer
            .render(&snapshot, request)
            .unwrap()
            .rgba8()
            .unwrap(),
        &[255; 4]
    );
}

#[test]
fn tile_boundaries_agree_for_curves_crops_and_layer_opacity() {
    let mut session = session(517, 13);
    let parent = page(&session);
    let mut path = BezPath::new();
    path.move_to((0.0, 1.0));
    path.curve_to((190.0, 20.0), (295.0, -15.0), (517.0, 8.0));
    path.line_to((517.0, 13.0));
    path.line_to((0.0, 13.0));
    path.close_path();
    let mut spec = LayerSpec::new(
        "Across tiles",
        Content::Vector(
            VectorContent::new(path.clone(), Color::new(0.1, 0.4, 0.9, 1.0).unwrap()).unwrap(),
        ),
    );
    spec.opacity = 0.66;
    spec.operations = vec![LiveOperation::Crop(Rect::new(0.25, 0.5, 516.75, 12.5))];
    add(&mut session, parent, spec);
    let wide = Renderer::new(Limits {
        tile_size: 2048,
        ..Limits::default()
    })
    .unwrap();
    let expected = render(&wide, &session, 517, 13);
    // Independently finer device geometry is scanned over the ORIGINAL full
    // 517-pixel extent; it never shares the renderer's canonical block clips.
    let oracle = full_extent_curve_oracle(&path, 1.0 / 262_144.0, 8);
    assert_alpha_and_premultiplied_agree(expected.rgba8().unwrap(), &oracle);
    for size in [17, 256] {
        let tiled = Renderer::new(Limits {
            tile_size: size,
            ..Limits::default()
        })
        .unwrap();
        let actual = render(&tiled, &session, 517, 13);
        // The coverage grid is a fixed numerical policy. Memory tile settings
        // must not change any channel, including alpha-zero canonicalization.
        assert_eq!(
            actual.rgba8().unwrap(),
            expected.rgba8().unwrap(),
            "color tile size {size}"
        );
    }
    let snapshot = session.snapshot();
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        3,
        3,
        View::Affine(Transform::new([1.0, 0.0, 0.0, 1.0, -255.0, -5.0]).unwrap()),
    )
    .unwrap();
    let expected_window = wide.render(&snapshot, request).unwrap();
    let single = Renderer::new(Limits {
        tile_size: 1,
        ..Limits::default()
    })
    .unwrap();
    assert_eq!(
        single.render(&snapshot, request).unwrap().rgba8().unwrap(),
        expected_window.rgba8().unwrap()
    );
}

fn full_extent_curve_oracle(path: &BezPath, tolerance: f64, scale: u32) -> Vec<u8> {
    let mut builder = tiny_skia::PathBuilder::new();
    kurbo::flatten(
        path.elements().iter().copied(),
        tolerance,
        |element| match element {
            kurbo::PathEl::MoveTo(p) => builder.move_to(
                (p.x * f64::from(scale)) as f32,
                (p.y * f64::from(scale)) as f32,
            ),
            kurbo::PathEl::LineTo(p) => builder.line_to(
                (p.x * f64::from(scale)) as f32,
                (p.y * f64::from(scale)) as f32,
            ),
            kurbo::PathEl::ClosePath => builder.close(),
            _ => panic!("oracle geometry was not flattened"),
        },
    );
    let mut whole_curve = tiny_skia::Mask::new(517 * scale, 13 * scale).unwrap();
    whole_curve.fill_path(
        &builder.finish().unwrap(),
        tiny_skia::FillRule::Winding,
        true,
        tiny_skia::Transform::identity(),
    );
    let crop = tiny_skia::PathBuilder::from_rect(
        tiny_skia::Rect::from_ltrb(
            0.25 * scale as f32,
            0.5 * scale as f32,
            516.75 * scale as f32,
            12.5 * scale as f32,
        )
        .unwrap(),
    );
    let mut whole_crop = tiny_skia::Mask::new(517 * scale, 13 * scale).unwrap();
    whole_crop.fill_path(
        &crop,
        tiny_skia::FillRule::Winding,
        true,
        tiny_skia::Transform::identity(),
    );
    let reduce = |mask: &tiny_skia::Mask| -> Vec<u8> {
        (0..13)
            .flat_map(|y| {
                (0..517).map(move |x| {
                    let mut sum = 0u32;
                    for sy in 0..scale {
                        for sx in 0..scale {
                            sum += u32::from(
                                mask.data()
                                    [((y * scale + sy) * 517 * scale + x * scale + sx) as usize],
                            );
                        }
                    }
                    ((sum + scale * scale / 2) / (scale * scale)) as u8
                })
            })
            .collect()
    };
    let curve = reduce(&whole_curve);
    let crop = reduce(&whole_crop);
    curve
        .iter()
        .zip(&crop)
        .flat_map(|(curve, crop)| {
            let alpha = (f64::from(*curve) * f64::from(*crop) * 0.66 / 255.0).round() as u8;
            if alpha == 0 {
                [0, 0, 0, 0]
            } else {
                [89, 170, 243, alpha]
            }
        })
        .collect()
}

fn assert_alpha_and_premultiplied_agree(actual: &[u8], expected: &[u8]) {
    fn decode(value: u8) -> f64 {
        let encoded = f64::from(value) / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    }
    for (index, (a, b)) in actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .enumerate()
    {
        assert!(
            a[3].abs_diff(b[3]) <= 1,
            "alpha at ({},{}): {a:?}/{b:?}",
            index % 517,
            index / 517
        );
        for channel in 0..3 {
            // Compare visible linear contribution, including pixels on opposite
            // sides of quantized-zero alpha. Straight RGB alone has no meaning
            // there; the alpha error still must remain <=one output code.
            let difference =
                (decode(a[channel]) * f64::from(a[3]) - decode(b[channel]) * f64::from(b[3])).abs();
            assert!(
                difference <= 1.0,
                "premultiplied channel {channel} at ({},{}): {a:?}/{b:?}",
                index % 517,
                index / 517
            );
        }
    }
}

#[test]
fn held_frame_and_shared_renderer_clones_keep_output_budget_charged() {
    let session = session(1, 1);
    let renderer = Renderer::new(Limits {
        max_bytes: 107_158,
        tile_size: 1,
        ..Limits::default()
    })
    .unwrap();
    let clone = renderer.clone();
    let frame = Arc::new(render(&renderer, &session, 1, 1));
    assert_eq!(renderer.allocated_bytes(), 4);
    let held = Arc::clone(&frame);
    drop(frame);
    assert_eq!(clone.allocated_bytes(), 4);
    let snapshot = session.snapshot();
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        1,
        1,
        View::Region(Rect::new(0.0, 0.0, 1.0, 1.0)),
    )
    .unwrap();
    assert!(matches!(
        clone.render(&snapshot, request),
        Err(Error::BudgetExceeded { .. })
    ));
    drop(held);
    assert_eq!(renderer.allocated_bytes(), 0);
    let frame = clone.render(&snapshot, request).unwrap();
    drop(renderer);
    assert_eq!(clone.allocated_bytes(), 4);
    drop(frame);
    assert_eq!(clone.allocated_bytes(), 0);
}

#[test]
fn concurrent_jobs_share_one_budget_and_frame_lifetimes() {
    let session = session(1, 1);
    let renderer = Renderer::new(Limits {
        max_bytes: 107_158,
        tile_size: 1,
        ..Limits::default()
    })
    .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let finish = Arc::new(Barrier::new(3));
    let snapshot = session.snapshot();
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        1,
        1,
        View::Region(Rect::new(0.0, 0.0, 1.0, 1.0)),
    )
    .unwrap();
    let jobs: Vec<_> = (0..2)
        .map(|_| {
            let renderer = renderer.clone();
            let snapshot = snapshot.clone();
            let barrier = Arc::clone(&barrier);
            let finish = Arc::clone(&finish);
            std::thread::spawn(move || {
                barrier.wait();
                let result = renderer.render(&snapshot, request);
                finish.wait();
                result
            })
        })
        .collect();
    barrier.wait();
    finish.wait();
    let results: Vec<_> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Err(Error::BudgetExceeded { .. })))
    );
    assert_eq!(renderer.allocated_bytes(), 4);
    drop(results);
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn dimensions_depth_work_geometry_and_hdr_overflow_are_structured_refusals() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let snapshot = session.snapshot();
    let id = snapshot.page_order()[0];
    assert!(matches!(
        RenderRequest::new(id, 0, 1, View::Region(Rect::new(0.0, 0.0, 1.0, 1.0))),
        Err(Error::InvalidRequest("output dimensions"))
    ));
    assert!(matches!(
        RenderRequest::new(id, 1, 1, View::Region(Rect::new(0.0, 0.0, 0.0, 1.0))),
        Err(Error::InvalidRequest("view region"))
    ));
    let limited = Renderer::new(Limits {
        max_dimension: 4,
        max_pixels: 8,
        ..Limits::default()
    })
    .unwrap();
    let request =
        RenderRequest::new(id, 5, 1, View::Region(Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    assert!(matches!(
        limited.render(&snapshot, request),
        Err(Error::LimitExceeded {
            resource: "output dimension",
            ..
        })
    ));
    let request =
        RenderRequest::new(id, 4, 4, View::Region(Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    assert!(matches!(
        limited.render(&snapshot, request),
        Err(Error::LimitExceeded {
            resource: "output pixels",
            ..
        })
    ));
    assert_eq!(limited.allocated_bytes(), 0);
    let outer = add(
        &mut session,
        parent,
        LayerSpec::new("Outer", Content::Group),
    );
    let inner = add(
        &mut session,
        Parent::Layer(outer),
        LayerSpec::new("Inner", Content::Group),
    );
    let leaf = add(
        &mut session,
        Parent::Layer(inner),
        LayerSpec::new("Leaf", rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0; 4])),
    );
    let snapshot = session.snapshot();
    let request =
        RenderRequest::new(id, 1, 1, View::Region(Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    let shallow = Renderer::new(Limits {
        max_depth: 2,
        ..Limits::default()
    })
    .unwrap();
    assert_eq!(
        shallow.render(&snapshot, request).unwrap_err(),
        Error::DepthExceeded {
            node: leaf,
            actual: 3,
            limit: 2
        }
    );
    assert_eq!(shallow.allocated_bytes(), 0);
    let bounded = Renderer::new(Limits {
        max_work: 1,
        ..Limits::default()
    })
    .unwrap();
    assert!(matches!(
        bounded.render(&snapshot, request),
        Err(Error::LimitExceeded {
            resource: "render work",
            ..
        })
    ));
    properties(&mut session, leaf, |spec| {
        spec.transform = Transform::new([1.0, 0.0, 0.0, 1.0, 2_000_000.0, 0.0]).unwrap()
    });
    let renderer = renderer();
    assert!(matches!(
        renderer.render(&session.snapshot(), request),
        Err(Error::GeometryRange {
            resource: "device coverage point",
            ..
        })
    ));
    assert_eq!(renderer.allocated_bytes(), 0);
    properties(&mut session, leaf, |spec| {
        spec.transform = Transform::IDENTITY;
        spec.operations = vec![LiveOperation::Exposure(32.0); 5];
    });
    assert!(matches!(
        renderer.render(&session.snapshot(), request),
        Err(Error::ColorRange {
            operation: "exposure",
            ..
        })
    ));
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn canceled_render_releases_output_and_scratch_after_admission() {
    let session = session(64, 64);
    let snapshot = session.snapshot();
    let renderer = renderer();
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        64,
        64,
        View::Region(Rect::new(0.0, 0.0, 64.0, 64.0)),
    )
    .unwrap();
    let observed_live_job = std::cell::Cell::new(false);
    let result = renderer.render_with_cancel(&snapshot, request, || {
        if renderer.allocated_bytes() > 0 {
            observed_live_job.set(true);
            true
        } else {
            false
        }
    });
    assert_eq!(result.unwrap_err(), Error::Canceled);
    assert!(observed_live_job.get());
    assert_eq!(renderer.allocated_bytes(), 0);
    assert_eq!(
        renderer
            .render_with_cancel(&snapshot, request, || true)
            .unwrap_err(),
        Error::Canceled
    );
    assert_eq!(renderer.allocated_bytes(), 0);
    let completed = renderer
        .render_with_cancel(&snapshot, request, || false)
        .unwrap();
    assert_eq!(completed.rgba8().unwrap().len(), 64 * 64 * 4);
}

#[test]
fn group_adjustments_can_recover_hdr_child_intermediates() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let mut group = LayerSpec::new("Group recovery", Content::Group);
    group.operations = vec![LiveOperation::Exposure(-4.0)];
    let group = add(&mut session, parent, group);
    let child = source(
        &mut session,
        Parent::Layer(group),
        1,
        1,
        vec![200, 160, 80, 255],
        Transform::IDENTITY,
    );
    session
        .transact(Edit::new(
            "Child HDR exposure",
            [Change::SetOperations {
                node: child,
                operations: vec![LiveOperation::Exposure(4.0)],
            }],
        ))
        .unwrap();
    assert_eq!(
        render(&renderer(), &session, 1, 1).rgba8().unwrap(),
        &[200, 160, 80, 255]
    );
}

#[test]
fn nonrepresentable_f32_fill_is_refused_instead_of_becoming_black() {
    let mut session = session(1, 1);
    let parent = page(&session);
    let node = add(
        &mut session,
        parent,
        LayerSpec::new(
            "Subnormal source",
            rectangle(Rect::new(0.0, 0.0, 1.0, 1.0), [1.0e-100, 0.0, 0.0, 1.0]),
        ),
    );
    let renderer = renderer();
    let snapshot = session.snapshot();
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        1,
        1,
        View::Region(Rect::new(0.0, 0.0, 1.0, 1.0)),
    )
    .unwrap();
    assert_eq!(
        renderer.render(&snapshot, request).unwrap_err(),
        Error::ColorRange {
            node,
            operation: "color/opacity conversion"
        }
    );
    assert_eq!(renderer.allocated_bytes(), 0);
}

#[test]
fn excessive_derived_curve_geometry_is_refused_before_output_allocation() {
    let mut session = session(517, 13);
    let parent = page(&session);
    let mut path = BezPath::new();
    path.move_to((0.0, 1.0));
    path.curve_to((190.0, 20.0), (295.0, -15.0), (517.0, 8.0));
    path.line_to((517.0, 13.0));
    path.line_to((0.0, 13.0));
    path.close_path();
    add(
        &mut session,
        parent,
        LayerSpec::new(
            "Bounded derived geometry",
            Content::Vector(
                VectorContent::new(path, Color::new(1.0, 0.0, 0.0, 1.0).unwrap()).unwrap(),
            ),
        ),
    );
    let snapshot = session.snapshot();
    let renderer = Renderer::new(Limits {
        max_flattened_elements: 6,
        ..Limits::default()
    })
    .unwrap();
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        517,
        13,
        View::Region(Rect::new(0.0, 0.0, 517.0, 13.0)),
    )
    .unwrap();
    assert!(matches!(
        renderer.render(&snapshot, request),
        Err(Error::LimitExceeded {
            resource: "flattened path elements",
            ..
        })
    ));
    assert_eq!(renderer.allocated_bytes(), 0);
}
