//! Real-document F32 upload/display oracles; all readback is explicit diagnostics.
use std::{fmt::Write, path::Path, sync::Arc};

use affint_document::{
    Change, Color, Content, Edit, LayerSpec, LiveOperation, Mask, MaskMode, NodeId, PageSize,
    Parent, PixelSize, RasterAsset, Session, Transform, VectorContent,
};
use affint_render::{Frame, FramePixels, OutputFormat, RenderRequest, Renderer, View};
use kurbo::{Affine, BezPath, Rect};

use super::{
    CameraIntent, CanvasSet, Conversion, DisplayConverter, DisplayMode, DisplayPlan, Evaluation,
    OVERSCAN, RenderSize, RenderTargetConfig, Scene, SourceMode, TextureBindings, Viewport,
    append_canvas, append_canvas_region, extent, padded_request, readback, texture,
};
use crate::{
    editor::AppResult,
    upload::{self, Admission, Canvas, Context, Key, Worker},
};

const CPU_ERROR: f64 = 3e-7;

fn document(width: u32, height: u32) -> AppResult<Session> {
    Ok(Session::new(
        PageSize::new(f64::from(width), f64::from(height))?,
        crate::editor::document_limits(),
    )?)
}
fn page(session: &Session) -> AppResult<Parent> {
    Ok(Parent::Page(
        *session
            .snapshot()
            .page_order()
            .first()
            .ok_or("Proof page is missing.")?,
    ))
}
fn index(session: &Session, parent: Parent) -> AppResult<usize> {
    let snapshot = session.snapshot();
    Ok(match parent {
        Parent::Page(id) => snapshot
            .page(id)
            .ok_or("Proof page is missing.")?
            .roots()
            .len(),
        Parent::Layer(id) => snapshot
            .node(id)
            .ok_or("Proof group is missing.")?
            .children()
            .len(),
    })
}
fn add(session: &mut Session, parent: Parent, layer: LayerSpec) -> AppResult<NodeId> {
    let at = index(session, parent)?;
    let created = session.transact(Edit::new(
        "GPU proof editable layer",
        [Change::InsertLayer {
            parent,
            index: at,
            layer,
        }],
    ))?;
    created
        .nodes
        .first()
        .copied()
        .ok_or_else(|| "GPU proof layer was not created.".into())
}
fn import(session: &mut Session, parent: Parent, asset: Arc<RasterAsset>) -> AppResult<NodeId> {
    let at = index(session, parent)?;
    let created = session.transact(Edit::new(
        "GPU proof original raster",
        [Change::InsertRaster {
            parent,
            index: at,
            name: "Immutable diagnostic original".into(),
            asset,
            transform: Transform::IDENTITY,
        }],
    ))?;
    created
        .nodes
        .first()
        .copied()
        .ok_or_else(|| "GPU proof raster was not created.".into())
}
fn request(session: &Session, width: u32, height: u32) -> AppResult<RenderRequest> {
    let Parent::Page(page) = page(session)? else {
        return Err("Proof root is not a page.".into());
    };
    Ok(
        RenderRequest::new(page, width, height, View::Affine(Transform::IDENTITY))?
            .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?,
    )
}
fn decode(encoded: f64) -> f64 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}
fn encode(linear: f64) -> u8 {
    let linear = linear.clamp(0., 1.);
    let encoded = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1. / 2.4) - 0.055
    };
    (encoded * 255.).round() as u8
}
fn expected_display(working: [f64; 4], x: usize, y: usize) -> [u8; 4] {
    let checker = if (x / 8 + y / 8) & 1 == 0 { 0.78 } else { 0.90 };
    let background = decode(checker) * (1. - working[3]);
    [
        encode(working[0] + background),
        encode(working[1] + background),
        encode(working[2] + background),
        255,
    ]
}
fn check_cpu(frame: &Frame, oracle: &impl Fn(usize, usize) -> [f64; 4]) -> AppResult<f64> {
    let FramePixels::LinearSrgbRgbaF32Premultiplied(pixels) = frame.pixels() else {
        return Err("Document proof did not receive working F32 pixels.".into());
    };
    let width = usize::try_from(frame.width())?;
    if pixels.len()
        != width
            .checked_mul(usize::try_from(frame.height())?)
            .ok_or("Proof frame dimensions overflow.")?
    {
        return Err("Working proof pixels do not match exact dimensions.".into());
    }
    let mut maximum = 0f64;
    for (index, actual) in pixels.iter().enumerate() {
        let expected = oracle(index % width, index / width);
        for channel in 0..4 {
            maximum = maximum.max((f64::from(actual[channel]) - expected[channel]).abs());
        }
        if expected[3] == 0. && actual.map(f32::to_bits) != [0; 4] {
            return Err(format!("CPU working zero-alpha pixel {index} was not canonical.").into());
        }
        if expected[3] > 0.
            && expected[3] < f64::from(f32::MIN_POSITIVE)
            && actual[3].to_bits() != (expected[3] as f32).to_bits()
        {
            return Err(format!("CPU working subnormal alpha at {index} was lost.").into());
        }
    }
    if maximum > CPU_ERROR {
        return Err(format!("Document F32 pixels differ from independent f64 oracle by {maximum}; allowed {CPU_ERROR}.").into());
    }
    Ok(maximum)
}
fn check_display(
    bytes: &[u8],
    width: usize,
    oracle: &impl Fn(usize, usize) -> [f64; 4],
) -> AppResult<u8> {
    let (pixels, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return Err("Document proof returned an incomplete display pixel.".into());
    }
    let mut maximum = 0;
    for (index, actual) in pixels.iter().enumerate() {
        let expected = expected_display(
            oracle(index % width, index / width),
            index % width,
            index / width,
        );
        for channel in 0..4 {
            maximum = maximum.max(actual[channel].abs_diff(expected[channel]));
        }
        if actual[3] != 255 {
            return Err("Document proof display was not opaque.".into());
        }
    }
    Ok(maximum)
}
fn convert(context: &Context, converter: &DisplayConverter, canvas: &Canvas) -> AppResult<()> {
    upload::checked_gpu(&context.device, || {
        let bind = converter.bind(&context.device, &canvas.source_view);
        let mut encoder = context.device.create_command_encoder(&Default::default());
        converter.encode(
            &context.queue,
            &mut encoder,
            &bind,
            &canvas.display_view,
            Conversion {
                page: canvas.artboard,
                source: SourceMode::LinearPremultiplied,
                scale: 1.,
                offset: [0, 0],
            },
        );
        context.queue.submit([encoder.finish()]);
        Ok(())
    })
}
struct Proof<'a> {
    path: &'a Path,
    context: Context,
    worker: Worker,
    renderer: Renderer,
    converter: DisplayConverter,
    serial: u64,
    observations: String,
}
impl Proof<'_> {
    fn retained_camera(&mut self) -> AppResult<()> {
        const WIDTH: u32 = 64;
        const HEIGHT: u32 = 40;
        let mut session = document(128, 96)?;
        let pixel = |x: usize, y: usize| -> [u8; 4] {
            [
                ((x * 17 + y * 3) % 256) as u8,
                ((x * 5 + y * 11) % 256) as u8,
                ((x * 7 + y * 19) % 256) as u8,
                if (x + y).is_multiple_of(7) {
                    0
                } else if x.is_multiple_of(3) {
                    32
                } else {
                    127
                },
            ]
        };
        let mut original = Vec::with_capacity(128 * 96 * 4);
        for y in 0..96 {
            for x in 0..128 {
                original.extend_from_slice(&pixel(x, y));
            }
        }
        let asset = Arc::new(RasterAsset::new(PixelSize::new(128, 96)?, original)?);
        let pointer = asset.pixels().as_ptr();
        let parent = page(&session)?;
        let node = import(&mut session, parent, Arc::clone(&asset))?;
        session.transact(Edit::new(
            "Retained camera exposure",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(2.)],
            }],
        ))?;
        let viewport = Viewport {
            x: 0,
            y: 0,
            width: WIDTH,
            height: HEIGHT,
        };
        let camera_request = RenderRequest::new(
            request(&session, WIDTH, HEIGHT)?.page(),
            WIDTH,
            HEIGHT,
            View::Affine(Transform::new([1., 0., 0., 1., 12., 7.])?),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let initial =
            CameraIntent::new(session.snapshot().revision(), camera_request, viewport, 1.)?;
        let source_request = padded_request(camera_request, OVERSCAN)?;
        let frame = self.renderer.render(&session.snapshot(), source_request)?;
        let oracle = |x: i64, y: i64| -> [f64; 4] {
            if !(0..128).contains(&x) || !(0..96).contains(&y) {
                return [0.; 4];
            }
            let rgba = pixel(x as usize, y as usize);
            let alpha = f64::from(rgba[3]) / 255.;
            [
                decode(f64::from(rgba[0]) / 255.) * alpha * 4.,
                decode(f64::from(rgba[1]) / 255.) * alpha * 4.,
                decode(f64::from(rgba[2]) / 255.) * alpha * 4.,
                alpha,
            ]
        };
        let source_error = check_cpu(&frame, &|x, y| oracle(x as i64 - 76, y as i64 - 71))?;
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("Proof serial exhausted.")?;
        let key = Key {
            revision: frame.revision(),
            request: source_request,
            generation: self.context.generation,
            serial: self.serial,
        };
        let evaluation = Evaluation::new(key, initial, OVERSCAN)?;
        let wanted_request = RenderRequest::new(
            camera_request.page(),
            WIDTH,
            HEIGHT,
            View::Affine(Transform::new([1., 0., 0., 1., 20., 13.])?),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let intent =
            CameraIntent::new(session.snapshot().revision(), wanted_request, viewport, 1.)?;
        let source_page = Rect::new(76., 71., 204., 167.);
        let plan = super::camera::select_plan(
            key,
            source_page,
            Some(evaluation),
            None,
            self.context.generation,
            intent,
        )?
        .ok_or("Retained camera candidate plan is missing.")?;
        if plan.source_offset != [56, 58]
            || plan.evaluation.key != key
            || plan.mode != DisplayMode::RetainedAtNativeDensity
            || !plan.pending
        {
            return Err("Retained camera relabeled, rescaled, or lost its pending evaluated-source identity.".into());
        }
        // This separate fresh viewport oracle is a raster identity case. It does
        // not prove general vector AA covariance under a new COVERAGE256 view.
        let fresh = self.renderer.render(&session.snapshot(), wanted_request)?;
        let fresh_error = check_cpu(&fresh, &|x, y| oracle(x as i64 - 20, y as i64 - 13))?;
        let FramePixels::LinearSrgbRgbaF32Premultiplied(source) = frame.pixels() else {
            return Err("Retained source is not F32.".into());
        };
        let FramePixels::LinearSrgbRgbaF32Premultiplied(reference) = fresh.pixels() else {
            return Err("Fresh reference is not F32.".into());
        };
        for y in 0..HEIGHT as usize {
            for x in 0..WIDTH as usize {
                let preserved = source[(y + 58) * source_request.width() as usize + x + 56];
                if preserved.map(f32::to_bits)
                    != reference[y * WIDTH as usize + x].map(f32::to_bits)
                {
                    return Err("Identity raster retained crop differs from its independent fresh viewport reference.".into());
                }
            }
        }
        drop(fresh);
        let canvas =
            upload::diagnostic_upload(&self.worker, frame, key, source_page, self.context.clone())?;
        if self.renderer.allocated_bytes() != 0 {
            return Err("Retained camera upload kept a CPU frame lease.".into());
        }
        let stats = format!("{:?}", canvas.stats);
        let desired = Some((key.revision, key.request, key.generation, key.serial));
        let mut canvases = CanvasSet::new(self.context.clone(), self.worker.retirement());
        canvases.offer(canvas, desired)?;
        let canvas = canvases.sample().ok_or("Retained candidate is missing.")?;
        if canvas.key != plan.evaluation.key || canvases.accepted_key().is_some() {
            return Err(
                "Candidate planning and chrome identity differ before presentation.".into(),
            );
        }
        upload::checked_gpu(&self.context.device, || {
            let bind = self
                .converter
                .bind(&self.context.device, &canvas.source_view);
            let mut encoder = self
                .context
                .device
                .create_command_encoder(&Default::default());
            self.converter.encode_region(
                &self.context.queue,
                &mut encoder,
                &bind,
                &canvas.display_view,
                Conversion {
                    page: plan.artboard,
                    source: SourceMode::LinearPremultiplied,
                    scale: 1.,
                    offset: plan.source_offset,
                },
                Some(plan.paint_size),
            );
            self.context.queue.submit([encoder.finish()]);
            Ok(())
        })?;
        let full = readback(
            &self.context.device,
            &self.context.queue,
            &canvas.display,
            source_request.width(),
            source_request.height(),
        )?;
        let mut visible = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
        let mut maximum = 0u8;
        let mut dark = 0;
        let mut light = 0;
        let mut outside = 0;
        let mut zero = 0;
        for y in 0..HEIGHT as usize {
            for x in 0..WIDTH as usize {
                let working = oracle(x as i64 - 20, y as i64 - 13);
                let in_page = x >= 20 && y >= 13;
                let expected = if in_page {
                    expected_display(working, x, y)
                } else {
                    let grey = encode(decode(0.115));
                    [grey, grey, grey, 255]
                };
                let offset = (y * source_request.width() as usize + x) * 4;
                let actual = full
                    .get(offset..offset + 4)
                    .ok_or("Retained display row is missing.")?;
                visible.extend_from_slice(actual);
                for channel in 0..4 {
                    maximum = maximum.max(actual[channel].abs_diff(expected[channel]));
                }
                if !in_page {
                    outside += 1;
                } else if working[3] == 0. {
                    zero += 1;
                } else if (x / 8 + y / 8) & 1 == 0 {
                    dark += 1;
                } else {
                    light += 1;
                }
            }
        }
        if maximum > 1 || dark == 0 || light == 0 || outside == 0 || zero == 0 {
            return Err(format!("Retained camera display failed independent destination-checker/page oracle: {maximum}/255; dark/light/outside/zero counts {dark}/{light}/{outside}/{zero}.").into());
        }
        crate::report_durability(affint_storage::save_png(
            self.path.join("document-f32-retained-camera.png"),
            PixelSize::new(WIDTH, HEIGHT)?,
            &visible,
        )?);
        self.retained_hybrid(canvas, plan, &visible)?;
        // Unsampled/retry planning alone never changes current publication.
        if canvases.promote_after_submit(desired, false)? || canvases.accepted_key().is_some() {
            return Err("A planned retained candidate was published without sampling.".into());
        }
        if !canvases.promote_after_submit(desired, true)? || canvases.accepted_key() != Some(key) {
            return Err(
                "Validated retained camera candidate did not publish its real source key.".into(),
            );
        }
        canvases.clear();
        self.retirements("Retained camera final Hybrid sampling")?;
        if self.worker.accounting()?.0 != 0 || self.worker.accounting()?.1 != 0 {
            return Err(
                "Retained camera final sampling kept charged GPU pairs after completion.".into(),
            );
        }
        let (pixels, tail) = asset.pixels().as_chunks::<4>();
        if !tail.is_empty()
            || asset.pixels().as_ptr() != pointer
            || pixels
                .iter()
                .enumerate()
                .any(|(index, actual)| *actual != pixel(index % 128, index / 128))
        {
            return Err("Retained camera changed original raster storage or pixels.".into());
        }
        writeln!(
            &mut self.observations,
            "Retained camera: evaluated source {}x{} and visible {WIDTH}x{HEIGHT}; unchanged evaluation key {key:?}; integer crop56,58 at1:1 F32 density, translated page/checker recomposed in destination coordinates; truthful pending native-density mode; source CPU error {source_error}, separate identity-raster fresh reference error {fresh_error}, GPU <= {maximum}/255. General fresh viewport AA equivalence is not asserted. Upload {stats} (padded row span, not measured bus traffic); final Hybrid submission/retirement preserves zero charged pairs; original allocation and bytes retained.",
            source_request.width(),
            source_request.height()
        )?;
        Ok(())
    }
    fn retained_hybrid(
        &self,
        document_canvas: &Canvas,
        plan: DisplayPlan,
        visible: &[u8],
    ) -> AppResult<()> {
        use mui::prelude::*;
        let [width, height] = plan.paint_size;
        let target = texture(
            &self.context.device,
            width,
            height,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let (mut hybrid, mut resources) = super::Renderer::new(
            &self.context.device,
            &RenderTargetConfig {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width,
                height,
            },
        );
        let mut scene = Scene::new(u16::try_from(width)?, u16::try_from(height)?);
        let mut bindings = TextureBindings::new();
        append_canvas_region(
            &mut scene,
            &mut bindings,
            &document_canvas.display_view,
            plan.paint_size,
            plan.viewport,
        )?;
        let marker = plan.logical_affine(kurbo::Point::ZERO) * kurbo::Point::new(8., 10.);
        if marker != kurbo::Point::new(28., 23.) {
            return Err(
                "Retained selection chrome used another camera than the submitted pixels.".into(),
            );
        }
        let mut ui = mui::Ui::new(Theme::DEFAULT);
        let root = overlay([leaf(4., 4.)
            .fill(mui::scene::Color::srgb(0.1, 0.2, 0.8))
            .radius(0.)
            .anchor(Align::Start, Align::Start)
            .offset(marker.x, marker.y)
            .id("retained-selection")])
        .size(f64::from(width), f64::from(height))
        .no_fill();
        let frame = ui.frame(
            root,
            Some(Size::new(f64::from(width), f64::from(height))),
            Input::default(),
            0.,
        )?;
        let mut cache = mui::vello::Cache::default();
        {
            let mut gpu = mui::vello::Gpu {
                scene: &mut scene,
                resources: &mut resources,
                cache: &mut cache,
                atlas: Some(mui::vello::Atlas {
                    renderer: &mut hybrid,
                    device: &self.context.device,
                    queue: &self.context.queue,
                }),
            };
            mui::vello::paint(&mut gpu, frame.scene, Affine::IDENTITY)?;
        }
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        hybrid.render(
            &scene,
            &mut resources,
            &self.context.device,
            &self.context.queue,
            &mut encoder,
            &RenderSize { width, height },
            &target.create_view(&Default::default()),
            &bindings,
        )?;
        self.context.queue.submit([encoder.finish()]);
        let actual = readback(
            &self.context.device,
            &self.context.queue,
            &target,
            width,
            height,
        )?;
        let (pixels, tail) = actual.as_chunks::<4>();
        if !tail.is_empty() || actual.len() != visible.len() {
            return Err(
                "Retained Hybrid readback dimensions differ from its native-density crop.".into(),
            );
        }
        let mut maximum = 0u8;
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index % width as usize;
            let y = index / width as usize;
            for channel in 0..4 {
                let expected = if (28..32).contains(&x) && (23..27).contains(&y) {
                    [26, 51, 204, 255][channel]
                } else {
                    visible[index * 4 + channel]
                };
                maximum = maximum.max(pixel[channel].abs_diff(expected));
            }
        }
        crate::report_durability(affint_storage::save_png(
            self.path.join("document-f32-retained-hybrid.png"),
            PixelSize::new(width, height)?,
            &actual,
        )?);
        if maximum > 1 {
            return Err(format!(
                "Retained Hybrid/selection seam error {maximum}/255 exceeds1/255."
            )
            .into());
        }
        Ok(())
    }
    fn retirements(&mut self, label: &str) -> AppResult<()> {
        let acknowledged = upload::diagnostic_retirement(&self.worker);
        writeln!(
            &mut self.observations,
            "{label}: callback owner-drop acknowledgement {:?}; charged (texture bytes, pairs, staging bytes) {:?}.",
            acknowledged.as_ref(),
            self.worker.accounting()
        )?;
        std::fs::write(
            self.path.join("working-observations.txt"),
            &self.observations,
        )?;
        acknowledged
    }
    fn show(
        &mut self,
        session: &Session,
        dimensions: [u32; 2],
        name: &str,
        oracle: impl Fn(usize, usize) -> [f64; 4],
    ) -> AppResult<Canvas> {
        let request = request(session, dimensions[0], dimensions[1])?;
        let frame = self.renderer.render(&session.snapshot(), request)?;
        let cpu_error = check_cpu(&frame, &oracle)?;
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("Proof serial exhausted.")?;
        let key = Key {
            revision: frame.revision(),
            request,
            generation: self.context.generation,
            serial: self.serial,
        };
        let canvas = upload::diagnostic_upload(
            &self.worker,
            frame,
            key,
            Rect::new(0., 0., f64::from(dimensions[0]), f64::from(dimensions[1])),
            self.context.clone(),
        )?;
        if self.renderer.allocated_bytes() != 0 {
            return Err("Completed upload retained a CPU working pixel owner.".into());
        }
        convert(&self.context, &self.converter, &canvas)?;
        let result = readback(
            &self.context.device,
            &self.context.queue,
            &canvas.display,
            dimensions[0],
            dimensions[1],
        )?;
        if result.len() != usize::try_from(u64::from(dimensions[0]) * u64::from(dimensions[1]) * 4)?
        {
            return Err("Working display readback does not match exact dimensions.".into());
        }
        crate::report_durability(affint_storage::save_png(
            self.path.join(format!("{name}.png")),
            PixelSize::new(dimensions[0], dimensions[1])?,
            &result,
        )?);
        let gpu_error = check_display(&result, usize::try_from(dimensions[0])?, &oracle)?;
        writeln!(
            &mut self.observations,
            "{name}: exact {}x{}; CPU maximum absolute working error {cpu_error}, limit {CPU_ERROR}; GPU display maximum error {gpu_error}/255, limit 1/255; upload row-band accounting {:?} (padded_bytes=stride*rows, not measured bus traffic); charged (texture bytes, pairs, staging bytes) {:?}; readback {} bytes; CPU leased bytes after upload 0.",
            dimensions[0],
            dimensions[1],
            canvas.stats,
            self.worker.accounting()?,
            result.len()
        )?;
        writeln!(
            &mut self.observations,
            "{name}: actual dark/light-cell first-row samples {:?}/{:?}; independent oracle {:?}/{:?}.",
            result.get(0..4),
            result.get(32..36),
            expected_display(oracle(0, 0), 0, 0),
            expected_display(oracle(8, 0), 8, 0)
        )?;
        std::fs::write(
            self.path.join("working-observations.txt"),
            &self.observations,
        )?;
        if gpu_error > 1 {
            return Err(format!("{name} differs from independent display oracle by {gpu_error}/255; inspect working-observations.txt and {name}.png.").into());
        }
        Ok(canvas)
    }
    fn exposure(&mut self) -> AppResult<(Session, Canvas)> {
        let mut session = document(32, 16)?;
        let mut bytes = Vec::with_capacity(32 * 16 * 4);
        for _y in 0..16 {
            for x in 0..32 {
                bytes.extend_from_slice(if x < 16 {
                    &[255, 255, 255, 32]
                } else {
                    &[0, 255, 255, 0]
                });
            }
        }
        let asset = Arc::new(RasterAsset::new(PixelSize::new(32, 16)?, bytes)?);
        let pointer = asset.pixels().as_ptr();
        let parent = page(&session)?;
        let node = import(&mut session, parent, Arc::clone(&asset))?;
        session.transact(Edit::new(
            "Real translucent exposure",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(2.)],
            }],
        ))?;
        let request = request(&session, 32, 16)?;
        let legacy = self.renderer.render(
            &session.snapshot(),
            request.with_format(OutputFormat::SrgbRgba8Straight)?,
        )?;
        let source = texture(
            &self.context.device,
            32,
            16,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let display = texture(
            &self.context.device,
            32,
            16,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        self.context.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            legacy
                .rgba8()
                .ok_or("Legacy diagnostic has no RGBA8 pixels.")?,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32 * 4),
                rows_per_image: Some(16),
            },
            extent(32, 16),
        );
        let bind = self.converter.bind(
            &self.context.device,
            &source.create_view(&Default::default()),
        );
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        self.converter.encode(
            &self.context.queue,
            &mut encoder,
            &bind,
            &display.create_view(&Default::default()),
            Conversion {
                page: Rect::new(0., 0., 32., 16.),
                source: SourceMode::StraightSrgb,
                scale: 1.,
                offset: [0, 0],
            },
        );
        self.context.queue.submit([encoder.finish()]);
        let old = readback(&self.context.device, &self.context.queue, &display, 32, 16)?;
        drop(legacy);
        let legacy_error = check_display(&old, 32, &|x, _| {
            if x < 16 { [32. / 255.; 4] } else { [0.; 4] }
        })?;
        crate::report_durability(affint_storage::save_png(
            self.path.join("document-legacy-rgba8.png"),
            PixelSize::new(32, 16)?,
            &old,
        )?);
        writeln!(
            &mut self.observations,
            "Translucent white original alpha32 +2EV: independent dark-cell old route predicts207, F32 predicts255; actual legacy dark/light {:?}/{:?}; legacy maximum error {legacy_error}/255.",
            &old[0..4],
            &old[8 * 4..9 * 4]
        )?;
        if legacy_error > 1 || old[0].abs_diff(207) > 1 {
            return Err(
                "Legacy diagnostic does not match the independent clipped RGBA8 oracle.".into(),
            );
        }
        let canvas = self.show(&session, [32, 16], "document-f32-exposure", |x, _| {
            if x < 16 {
                [128. / 255., 128. / 255., 128. / 255., 32. / 255.]
            } else {
                [0.; 4]
            }
        })?;
        if asset.pixels().as_ptr() != pointer
            || asset
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .any(|(index, pixel)| {
                    *pixel
                        != if index % 32 < 16 {
                            [255, 255, 255, 32]
                        } else {
                            [0, 255, 255, 0]
                        }
                })
        {
            return Err("Working exposure changed original source storage or bytes.".into());
        }
        Ok((session, canvas))
    }
    fn subnormal(&mut self) -> AppResult<()> {
        let mut session = document(32, 16)?;
        let mut path = BezPath::new();
        path.move_to((0., 0.));
        path.line_to((32., 0.));
        path.line_to((32., 16.));
        path.line_to((0., 16.));
        path.close_path();
        let alpha = 2.0f64.powi(-130);
        let mut layer = LayerSpec::new(
            "Normal RGB over positive subnormal alpha",
            Content::Vector(VectorContent::new(path, Color::new(1., 1., 1., alpha)?)?),
        );
        layer.operations = vec![LiveOperation::Exposure(32.); 4];
        let parent = page(&session)?;
        add(&mut session, parent, layer)?;
        drop(
            self.show(&session, [32, 16], "document-f32-subnormal", |_, _| {
                [0.25, 0.25, 0.25, alpha]
            })?,
        );
        writeln!(
            &mut self.observations,
            "Subnormal fixture: white vector alpha2^-130 with four ordered +32EV operations; independent RGB0.25 and alpha2^-130. WGSL permits arithmetic/comparison flush-to-zero; trusted working RGB must not be conditioned on an arithmetic alpha>0 test. This is a portability oracle, not a claimed prior adapter failure."
        )?;
        Ok(())
    }
    fn masks(&mut self) -> AppResult<()> {
        let mut session = document(32, 16)?;
        let parent = page(&session)?;
        let mut group = LayerSpec::new("Isolated editable exposure", Content::Group);
        group.opacity = 0.5;
        group.operations = vec![
            LiveOperation::Exposure(2.),
            LiveOperation::Crop(Rect::new(0., 0., 32., 16.)),
        ];
        let group = add(&mut session, parent, group)?;
        let blue = Arc::new(RasterAsset::new(
            PixelSize::new(32, 16)?,
            [0, 0, 255, 255].repeat(32 * 16),
        )?);
        let red = Arc::new(RasterAsset::new(
            PixelSize::new(32, 16)?,
            [255, 0, 0, 128].repeat(32 * 16),
        )?);
        import(&mut session, Parent::Layer(group), Arc::clone(&blue))?;
        import(&mut session, Parent::Layer(group), Arc::clone(&red))?;
        let mut hidden = LayerSpec::new("Unrelated hidden mask parent", Content::Group);
        hidden.visible = false;
        hidden.transform = Transform::new([1., 0., 0., 1., 2000., 0.])?;
        let hidden = add(&mut session, parent, hidden)?;
        let mask = import(&mut session, Parent::Layer(hidden), Arc::clone(&red))?;
        let snapshot = session.snapshot();
        let mut layer =
            crate::editor::layer_spec(snapshot.node(mask).ok_or("Proof mask is missing.")?);
        layer.visible = false;
        layer.opacity = 0.5;
        layer.operations = vec![
            LiveOperation::Exposure(1.),
            LiveOperation::Exposure(-1.),
            LiveOperation::Crop(Rect::new(0., 0., 32., 16.)),
        ];
        session.transact(Edit::new(
            "Hidden target-local mask",
            [Change::SetLayer { node: mask, layer }],
        ))?;
        let red_alpha = 128. / 255.;
        for (name, mode, inverted, coverage) in [
            ("alpha", MaskMode::Alpha, false, red_alpha * 0.5),
            (
                "luminance",
                MaskMode::Luminance,
                false,
                0.2126 * red_alpha * 0.5,
            ),
            (
                "inverted-luminance",
                MaskMode::Luminance,
                true,
                1. - 0.2126 * red_alpha * 0.5,
            ),
        ] {
            session.transact(Edit::new(
                "Editable mask interpretation",
                [Change::SetMask {
                    node: group,
                    mask: Some(Mask {
                        source: mask,
                        mode,
                        inverted,
                    }),
                }],
            ))?;
            let expected = [
                4. * red_alpha * coverage * 0.5,
                0.,
                4. * (1. - red_alpha) * coverage * 0.5,
                coverage * 0.5,
            ];
            drop(self.show(
                &session,
                [32, 16],
                &format!("document-f32-mask-{name}"),
                |_, _| expected,
            )?);
        }
        if blue
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [0, 0, 255, 255])
            || red
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| *pixel != [255, 0, 0, 128])
        {
            return Err("Mask/isolation proof changed immutable originals.".into());
        }
        Ok(())
    }
    fn bands(&mut self) -> AppResult<()> {
        const WIDTH: u32 = 1281;
        const HEIGHT: u32 = 1100;
        let pattern = |x: usize, y: usize| {
            [
                ((x * 3 + y * 5) % 224) as u8,
                ((x * 11 + y * 7) % 224) as u8,
                ((x * 17 + y * 13) % 224) as u8,
                (64 + (x / 17 + y / 19) % 160) as u8,
            ]
        };
        let mut bytes = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
        for y in 0..HEIGHT as usize {
            for x in 0..WIDTH as usize {
                bytes.extend_from_slice(&pattern(x, y));
            }
        }
        let asset = Arc::new(RasterAsset::new(PixelSize::new(WIDTH, HEIGHT)?, bytes)?);
        let pointer = asset.pixels().as_ptr();
        let mut session = document(WIDTH, HEIGHT)?;
        let parent = page(&session)?;
        let node = import(&mut session, parent, Arc::clone(&asset))?;
        session.transact(Edit::new(
            "Nonuniform row-band exposure",
            [Change::SetOperations {
                node,
                operations: vec![LiveOperation::Exposure(1.)],
            }],
        ))?;
        let canvas = self.show(
            &session,
            [WIDTH, HEIGHT],
            "document-f32-row-bands",
            |x, y| {
                let source = pattern(x, y);
                let alpha = f64::from(source[3]) / 255.;
                [
                    decode(f64::from(source[0]) / 255.) * alpha * 2.,
                    decode(f64::from(source[1]) / 255.) * alpha * 2.,
                    decode(f64::from(source[2]) / 255.) * alpha * 2.,
                    alpha,
                ]
            },
        )?;
        let expected_bytes = WIDTH as usize * HEIGHT as usize * 16;
        if canvas.stats.bands < 3
            || canvas.stats.copied_bytes != expected_bytes
            || !canvas.stats.row_stride.is_multiple_of(256)
            || canvas.stats.row_stride <= WIDTH as usize * 16
        {
            return Err("Nonuniform working proof did not cross three padded upload bands and reuse a buffer.".into());
        }
        if asset.pixels().as_ptr() != pointer
            || asset
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .any(|(index, pixel)| {
                    *pixel != pattern(index % WIDTH as usize, index / WIDTH as usize)
                })
        {
            return Err("Row-band proof changed original storage or pixels.".into());
        }
        drop(canvas);
        let request = request(&session, WIDTH, HEIGHT)?;
        let frame = self.renderer.render(&session.snapshot(), request)?;
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("Proof serial exhausted.")?;
        let key = Key {
            revision: frame.revision(),
            request,
            generation: self.context.generation,
            serial: self.serial,
        };
        upload::diagnostic_cancel(
            &self.worker,
            frame,
            key,
            Rect::new(0., 0., f64::from(WIDTH), f64::from(HEIGHT)),
            self.context.clone(),
        )?;
        self.retirements("Canceled row-band upload")?;
        if self.worker.take()?.is_some()
            || self.worker.accounting()?.0 != 0
            || self.renderer.allocated_bytes() != 0
        {
            return Err("Canceled real upload retained a published result, GPU pair, or CPU pixel lease after completion.".into());
        }
        writeln!(
            &mut self.observations,
            "Real row-band upload canceled through ordinary desire/submit mailbox: no obsolete completion or CPU/GPU pair remains after completion. Scheduling may cancel before, during, or after copying; no claim that this run necessarily canceled a partial band."
        )?;
        Ok(())
    }
    fn hybrid_sample(&self, canvas: &Canvas) -> AppResult<()> {
        let width = canvas.key.request.width();
        let height = canvas.key.request.height();
        let target = texture(
            &self.context.device,
            width,
            height,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let (mut hybrid, mut resources) = super::Renderer::new(
            &self.context.device,
            &RenderTargetConfig {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width,
                height,
            },
        );
        let mut scene = Scene::new(u16::try_from(width)?, u16::try_from(height)?);
        let mut bindings = TextureBindings::new();
        append_canvas(
            &mut scene,
            &mut bindings,
            &canvas.display_view,
            [width, height],
            Viewport {
                x: 0,
                y: 0,
                width,
                height,
            },
        )?;
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        hybrid.render(
            &scene,
            &mut resources,
            &self.context.device,
            &self.context.queue,
            &mut encoder,
            &RenderSize { width, height },
            &target.create_view(&Default::default()),
            &bindings,
        )?;
        self.context.queue.submit([encoder.finish()]);
        let actual = readback(
            &self.context.device,
            &self.context.queue,
            &target,
            width,
            height,
        )?;
        let maximum = check_display(&actual, width as usize, &|x, _| {
            if x < 16 {
                [128. / 255., 128. / 255., 128. / 255., 32. / 255.]
            } else {
                [0.; 4]
            }
        })?;
        crate::report_durability(affint_storage::save_png(
            self.path.join("document-f32-hybrid.png"),
            PixelSize::new(width, height)?,
            &actual,
        )?);
        if maximum > 1 {
            return Err(format!(
                "Real-document Hybrid texture seam error {maximum}/255 exceeds 1/255."
            )
            .into());
        }
        Ok(())
    }
    fn publication(&mut self, session: &Session, first: Canvas) -> AppResult<()> {
        self.hybrid_sample(&first)?;
        let first_key = first.key;
        let desired = |key: Key| Some((key.revision, key.request, key.generation, key.serial));
        let mut canvases = CanvasSet::new(self.context.clone(), self.worker.retirement());
        canvases.offer(first, desired(first_key))?;
        if canvases.accepted_key().is_some()
            || !canvases.promote_after_submit(desired(first_key), true)?
        {
            return Err(
                "First candidate was published before submission or failed to publish afterward."
                    .into(),
            );
        }
        let second = self.show(session, [32, 16], "document-f32-candidate", |x, _| {
            if x < 16 {
                [128. / 255., 128. / 255., 128. / 255., 32. / 255.]
            } else {
                [0.; 4]
            }
        })?;
        let second_key = second.key;
        canvases.offer(second, desired(second_key))?;
        if self.worker.admission(second_key.request)? != Admission::Pending
            || self.worker.accounting()?.1 != 2
        {
            return Err("Two admitted GPU pairs did not apply backpressure.".into());
        }
        // Surface retry/occlusion does not invoke successful publication. An absent
        // viewport also cannot publish a texture which has never been sampled.
        if canvases.promote_after_submit(desired(second_key), false)?
            || canvases.accepted_key() != Some(first_key)
        {
            return Err("Private unsampled candidate replaced the accepted canvas.".into());
        }
        let changed = Key {
            serial: second_key.serial + 1,
            ..second_key
        };
        if canvases
            .promote_after_submit(desired(changed), true)
            .is_ok()
        {
            return Err("Late obsolete serial was accepted for publication.".into());
        }
        canvases.keep_desired(desired(changed));
        if canvases.candidate().is_some() || canvases.accepted_key() != Some(first_key) {
            return Err("Obsolete candidate changed accepted publication.".into());
        }
        self.retirements("Obsolete private candidate")?;
        let replacement = self.show(session, [32, 16], "document-f32-replacement", |x, _| {
            if x < 16 {
                [128. / 255., 128. / 255., 128. / 255., 32. / 255.]
            } else {
                [0.; 4]
            }
        })?;
        self.hybrid_sample(&replacement)?;
        let accepted = replacement.key;
        canvases.offer(replacement, desired(accepted))?;
        if canvases.accepted_key() != Some(first_key)
            || !canvases.promote_after_submit(desired(accepted), true)?
            || canvases.accepted_key() != Some(accepted)
        {
            return Err("Validated replacement did not publish atomically.".into());
        }
        self.retirements("Replaced sampled canvas")?;
        if self.worker.accounting()?.1 != 1 {
            return Err(
                "Old sampled canvas did not release its slot after final-submission completion."
                    .into(),
            );
        }
        let wrong_device = self.show(
            session,
            [32, 16],
            "document-f32-device-candidate",
            |x, _| {
                if x < 16 {
                    [128. / 255., 128. / 255., 128. / 255., 32. / 255.]
                } else {
                    [0.; 4]
                }
            },
        )?;
        let key = wrong_device.key;
        if canvases
            .offer(
                wrong_device,
                desired(Key {
                    generation: key.generation + 1,
                    ..key
                }),
            )
            .is_ok()
            || canvases.accepted_key() != Some(accepted)
        {
            return Err("Canvas publication accepted a stale device generation.".into());
        }
        canvases.clear();
        self.retirements("Cleared private/current canvas")?;
        if self.worker.accounting()?.0 != 0 || self.worker.accounting()?.1 != 0 {
            return Err(
                "Completed private/current canvas retirement leaked charged GPU pairs.".into(),
            );
        }
        writeln!(
            &mut self.observations,
            "Real-document Hybrid seam <=1/255. Publication: accepted only after successful conversion/Hybrid submission; unsampled/retry candidate retained privately; two pairs backpressure; stale serial and device generation preserve prior accepted key; completion releases all texture leases. Fixed staging remains16MiB while the upload worker is alive."
        )?;
        Ok(())
    }
}

pub(super) fn run(
    path: &Path,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> AppResult<()> {
    let context = Context::new(adapter, device.clone(), queue.clone(), 1)?;
    let mut proof = Proof {
        path,
        converter: DisplayConverter::new(device),
        context,
        worker: Worker::diagnostic()?,
        renderer: Renderer::new(affint_render::Limits {
            max_bytes: 256 * 1024 * 1024,
            ..Default::default()
        })?,
        serial: 0,
        observations: String::from(
            "Real document -> leased linear-premultiplied F32 Frame -> ordinary fixed worker/two8MiB upload bands -> RGBA32Float textureLoad -> opaque RGBA8 display. Independent scalar f64 oracles; exact dimensions and immutable originals. All readback here is explicit diagnostics. No performance claim.\n",
        ),
    };
    let (session, first) = proof.exposure()?;
    proof.publication(&session, first)?;
    proof.retained_camera()?;
    proof.subnormal()?;
    proof.masks()?;
    proof.bands()?;
    std::fs::write(path.join("working-observations.txt"), &proof.observations)?;
    std::fs::write(
        path.join("working-proof.txt"),
        "Real working-document exposure, positive subnormal alpha with normal RGB, target-local alpha/luminance/inverted masks, group isolation and exact nonuniform three-band upload pass independent CPU absolute3e-7 and GPU display<=1/255 oracles. Retained-camera translation preserves evaluated-source F32 texels at1:1 physical density, recomposes destination checker/page, keeps pending source identity and positions actual MUI/Hybrid chrome through that same plan; separate identity-raster fresh reference agreement does not assert general viewport vector AA covariance. Zero-alpha working pixels are canonical. Original pixels and storage pointers preserved. Explicit RGBA8 PNG policy is distinct. Private publication, device/serial rejection, two-pair admission and final-sampling completion retirement verified.\n",
    )?;
    Ok(())
}
