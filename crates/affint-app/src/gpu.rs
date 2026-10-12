//! Same-device display conversion, external Hybrid sampling, and native lifecycle.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use affint_render::{RenderRequest, View};
use kurbo::{Affine, Rect, Shape};
use mui::scene::ResolvedScene;
use vello_common::{
    TextureId,
    geometry::{RectU16, SizeU16},
    peniko::ImageQuality,
};
use vello_hybrid::{
    AtlasConfig, LayersConfig, MemorySettings, RenderSettings, RenderSize, RenderTargetConfig,
    Renderer, Resources, SampleRect, Scene, TextureBindings,
};
use winit::{
    event_loop::{EventLoopProxy, OwnedDisplayHandle},
    window::Window,
};

use crate::{Wake, editor::AppResult, upload};

mod camera;
mod canvas;
mod document_proof;
pub(crate) use camera::prefetch_fits;
pub use camera::{CameraIntent, DisplayMode, DisplayPlan, Evaluation, OVERSCAN, padded_request};
use canvas::CanvasSet;

const CANVAS_ID: TextureId = TextureId(0xAFF1_0001);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Viewport {
    pub fn rect(self) -> Rect {
        Rect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.x + self.width),
            f64::from(self.y + self.height),
        )
    }
}

pub struct DisplayConverter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}
#[derive(Clone, Copy)]
#[repr(u32)]
enum SourceMode {
    StraightSrgb = 0,
    LinearPremultiplied = 1,
}
struct Conversion {
    page: Rect,
    source: SourceMode,
    scale: f32,
    offset: [i32; 2],
}
impl DisplayConverter {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AFFINT display inputs"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(32),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("AFFINT display layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("AFFINT linear display conversion"),
            source: wgpu::ShaderSource::Wgsl(include_str!("display.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("AFFINT display conversion"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AFFINT display parameters"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            uniform,
        }
    }
    fn bind(&self, device: &wgpu::Device, source: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AFFINT display bindings"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        })
    }
    fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bind: &wgpu::BindGroup,
        target: &wgpu::TextureView,
        options: Conversion,
    ) {
        self.encode_region(queue, encoder, bind, target, options, None);
    }
    fn encode_region(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bind: &wgpu::BindGroup,
        target: &wgpu::TextureView,
        options: Conversion,
        visible: Option<[u32; 2]>,
    ) {
        let mut bytes = [0u8; 32];
        for (index, value) in [
            options.page.x0 as f32,
            options.page.y0 as f32,
            options.page.x1 as f32,
            options.page.y1 as f32,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[16..20].copy_from_slice(&(options.source as u32).to_le_bytes());
        bytes[20..24].copy_from_slice(&(8. * options.scale).to_le_bytes());
        bytes[24..28].copy_from_slice(&options.offset[0].to_le_bytes());
        bytes[28..32].copy_from_slice(&options.offset[1].to_le_bytes());
        queue.write_buffer(&self.uniform, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("AFFINT opaque display pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind, &[]);
        if let Some([width, height]) = visible {
            pass.set_scissor_rect(0, 0, width, height);
        }
        pass.draw(0..3, 0..1);
    }
}

pub enum Present {
    Presented,
    Retry,
    Hidden,
}

/// CPU wall times for one presentation attempt, never GPU or optical latency.
#[derive(Debug, Clone, Copy)]
pub struct PresentTiming {
    pub started_at: Instant,
    pub finished_at: Instant,
    pub poll: Duration,
    pub acquire: Duration,
    pub compose: Duration,
    /// Scope push/pop and guard overhead, excluding measured compose work.
    pub gpu_checks: Duration,
    pub conversion_encode: Duration,
    pub canvas_encode: Duration,
    pub chrome_paint: Duration,
    pub hybrid_encode: Duration,
    pub submit: Duration,
    pub surface_present: Duration,
    pub sampled_key: Option<upload::Key>,
    pub accepted_key: Option<upload::Key>,
    pub converted: bool,
    pub plan: Option<DisplayPlan>,
}
impl PresentTiming {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            started_at: now,
            finished_at: now,
            poll: Duration::ZERO,
            acquire: Duration::ZERO,
            compose: Duration::ZERO,
            gpu_checks: Duration::ZERO,
            conversion_encode: Duration::ZERO,
            canvas_encode: Duration::ZERO,
            chrome_paint: Duration::ZERO,
            hybrid_encode: Duration::ZERO,
            submit: Duration::ZERO,
            surface_present: Duration::ZERO,
            sampled_key: None,
            accepted_key: None,
            converted: false,
            plan: None,
        }
    }
}
/// Bounded presentation counters; Instants are recorded only with AFFINT_DIAGNOSTICS=1.
#[derive(Debug, Clone, Copy, Default)]
pub struct PresentStats {
    pub attempts: u64,
    pub presented: u64,
    pub retries: u64,
    pub hidden: u64,
    pub failed: u64,
    pub conversions_encoded: u64,
    pub latest: Option<PresentTiming>,
}
fn stage_start(timing: &Option<PresentTiming>) -> Option<Instant> {
    timing.as_ref().map(|_| Instant::now())
}

pub struct Host {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    hybrid: Renderer,
    resources: Resources,
    cache: mui::vello::Cache,
    scene: Scene,
    converter: DisplayConverter,
    canvases: CanvasSet,
    pub lost: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    retries: u8,
    timing_enabled: bool,
    present_stats: PresentStats,
    evaluation: Option<Evaluation>,
    receipt: Option<DisplayPlan>,
    converted_for: Option<DisplayStamp>,
}
#[derive(Clone, Copy, PartialEq)]
struct DisplayStamp {
    key: upload::Key,
    offset: [i32; 2],
    size: [u32; 2],
    artboard: Rect,
    scale: f64,
}
impl From<DisplayPlan> for DisplayStamp {
    fn from(plan: DisplayPlan) -> Self {
        Self {
            key: plan.evaluation.key,
            offset: plan.source_offset,
            size: plan.paint_size,
            artboard: plan.artboard,
            scale: plan.scale,
        }
    }
}
impl Host {
    /// Runs on the fixed startup/recovery worker; window ownership is safe and shared.
    pub fn open(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
        proxy: EventLoopProxy<Wake>,
        generation: u64,
        retirement: upload::Retirement,
    ) -> AppResult<Self> {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)),
        );
        let surface = instance.create_surface(wgpu::SurfaceTarget::from_window_without_display(
            Arc::clone(&window),
        ))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let info = adapter.get_info();
        eprintln!("AFFINT GPU: {info:?}; MUI same-device native canvas; CPU reference evaluation");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let context = upload::Context::new(&adapter, device.clone(), queue.clone(), generation)?;
        let caps = surface.get_capabilities(&adapter);
        let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm].into_iter().find(|format| caps.formats.contains(format)).ok_or("The display exposes no supported non-sRGB 8-bit surface. Try another graphics backend.")?;
        let size = window.inner_size();
        validate_size(size.width.max(1), size.height.max(1), &device)?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("The graphics adapter cannot configure this display.")?;
        config.format = format;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        let atlas = AtlasConfig {
            initial_atlas_count: 0,
            max_atlases: 1,
            atlas_size: (512, 512),
            auto_grow: false,
            ..Default::default()
        };
        let settings = RenderSettings {
            memory_settings: MemorySettings {
                image_atlas_config: atlas,
                layers_config: LayersConfig {
                    max_textures: Some(4),
                    min_texture_size: SizeU16::new(256),
                    max_texture_size: SizeU16::new(1024),
                },
            },
            ..Default::default()
        };
        let (hybrid, resources) = Renderer::new_with(
            &device,
            &RenderTargetConfig {
                format,
                width: config.width,
                height: config.height,
            },
            settings,
        );
        let cache = mui::vello::Cache::for_atlas(atlas);
        let scene = Scene::new(config.width as u16, config.height as u16);
        let converter = DisplayConverter::new(&device);
        let lost = Arc::new(AtomicBool::new(false));
        let lost_flag = Arc::clone(&lost);
        let lost_proxy = proxy.clone();
        device.set_device_lost_callback(move |reason, message| {
            eprintln!("AFFINT device loss: {reason:?}: {message}");
            lost_flag.store(true, Ordering::Release);
            let _ = lost_proxy.send_event(Wake);
        });
        let error = Arc::new(Mutex::new(None));
        let error_slot = Arc::clone(&error);
        device.on_uncaptured_error(Arc::new(move |failure: wgpu::Error| { if let Ok(mut slot) = error_slot.lock() { *slot = Some(format!("Graphics operation failed: {failure}. Your artwork is preserved; save the document and reopen AFFINT.")); } let _ = proxy.send_event(Wake); }));
        let ui_atlas = hybrid.atlas_texture();
        let atlas_bytes = u64::from(ui_atlas.width())
            * u64::from(ui_atlas.height())
            * u64::from(ui_atlas.depth_or_array_layers())
            * 4;
        eprintln!(
            "AFFINT texture admission: canvas <=320 MiB and two total F32/display pairs; upload staging <=16 MiB; MUI image atlas {atlas_bytes} bytes now, <=1 MiB configured; MUI layer textures <=16 MiB; surface/backbuffers and Hybrid private bookkeeping are separate."
        );
        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            hybrid,
            resources,
            cache,
            scene,
            converter,
            canvases: CanvasSet::new(context, retirement),
            lost,
            error,
            retries: 0,
            timing_enabled: crate::jobs::diagnostics_enabled(),
            present_stats: PresentStats::default(),
            evaluation: None,
            receipt: None,
            converted_for: None,
        })
    }
    pub fn take_error(&self) -> Option<String> {
        match self.error.lock() {
            Ok(mut error) => error.take(),
            Err(_) => Some("The graphics error channel stopped unexpectedly.".into()),
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) -> AppResult<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        validate_size(width, height, &self.device)?;
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.scene.reset_and_resize(width as u16, height as u16);
        }
        Ok(())
    }
    pub fn context(&self) -> upload::Context {
        self.canvases.context().clone()
    }
    pub fn accepted_key(&self) -> Option<upload::Key> {
        self.canvases.accepted_key()
    }
    pub fn has_candidate(&self) -> bool {
        self.canvases.candidate().is_some()
    }
    pub fn register_evaluation(&mut self, evaluation: Evaluation) {
        self.evaluation = Some(evaluation);
    }
    pub fn receipt(&self) -> Option<DisplayPlan> {
        self.receipt
    }
    pub fn plan(&self, intent: Option<CameraIntent>) -> AppResult<Option<DisplayPlan>> {
        let (Some(intent), Some(canvas)) = (intent, self.canvases.sample()) else {
            return Ok(None);
        };
        camera::select_plan(
            canvas.key,
            canvas.artboard,
            self.evaluation,
            self.receipt,
            self.canvases.context().generation,
            intent,
        )
    }
    pub fn offer(
        &mut self,
        canvas: upload::Canvas,
        desired: Option<(affint_document::Revision, RenderRequest, u64, u64)>,
    ) -> AppResult<()> {
        self.canvases.offer(canvas, desired)
    }
    pub fn keep_desired(
        &mut self,
        desired: Option<(affint_document::Revision, RenderRequest, u64, u64)>,
    ) {
        self.canvases.keep_desired(desired);
    }
    pub fn clear_canvas(&mut self) {
        self.canvases.clear();
        self.receipt = None;
        self.evaluation = None;
        self.converted_for = None;
    }
    pub fn present(
        &mut self,
        chrome: &ResolvedScene,
        plan: Option<DisplayPlan>,
        desired: Option<(affint_document::Revision, RenderRequest, u64, u64)>,
        scale: f64,
    ) -> AppResult<Present> {
        let mut timing = self.timing_enabled.then(PresentTiming::new);
        let result = self.present_inner(chrome, plan, desired, scale, &mut timing);
        self.present_stats.attempts = self.present_stats.attempts.saturating_add(1);
        match &result {
            Ok(Present::Presented) => {
                self.present_stats.presented = self.present_stats.presented.saturating_add(1)
            }
            Ok(Present::Retry) => {
                self.present_stats.retries = self.present_stats.retries.saturating_add(1)
            }
            Ok(Present::Hidden) => {
                self.present_stats.hidden = self.present_stats.hidden.saturating_add(1)
            }
            Err(_) => self.present_stats.failed = self.present_stats.failed.saturating_add(1),
        }
        if let Some(mut timing) = timing {
            timing.finished_at = Instant::now();
            timing.accepted_key = self.canvases.accepted_key();
            self.present_stats.latest = Some(timing);
        }
        result
    }
    /// Metadata only; ordinary rendering performs no trace/file I/O.
    pub fn present_stats(&self) -> PresentStats {
        self.present_stats
    }
    fn present_inner(
        &mut self,
        chrome: &ResolvedScene,
        plan: Option<DisplayPlan>,
        desired: Option<(affint_document::Revision, RenderRequest, u64, u64)>,
        scale: f64,
        timing: &mut Option<PresentTiming>,
    ) -> AppResult<Present> {
        // Poll drives finite texture retirement without synchronously waiting for GPU work.
        let started = stage_start(timing);
        let polled = self.device.poll(wgpu::PollType::Poll);
        if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
            timing.poll = started.elapsed();
        }
        polled?;
        self.keep_desired(desired);
        use wgpu::CurrentSurfaceTexture as Acquired;
        let started = stage_start(timing);
        let acquired = self.surface.get_current_texture();
        if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
            timing.acquire = started.elapsed();
        }
        let surface_frame = match acquired {
            Acquired::Success(frame) | Acquired::Suboptimal(frame) => {
                self.retries = 0;
                frame
            }
            Acquired::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return self.retry();
            }
            Acquired::Timeout => return self.retry(),
            Acquired::Lost => {
                self.surface = self.instance.create_surface(
                    wgpu::SurfaceTarget::from_window_without_display(Arc::clone(&self.window)),
                )?;
                self.surface.configure(&self.device, &self.config);
                return self.retry();
            }
            Acquired::Occluded => return Ok(Present::Hidden),
            Acquired::Validation => return Err(
                "The display rejected its graphics surface. Save your document and reopen AFFINT."
                    .into(),
            ),
        };
        let target = surface_frame.texture.create_view(&Default::default());
        let device = self.device.clone();
        let checked_started = stage_start(timing);
        let submitted = upload::checked_gpu(&device, || {
            let started = stage_start(timing);
            let result = crate::jobs::guard("Native canvas presentation", || {
                self.compose(chrome, plan, scale, &target, timing)
            });
            if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
                timing.compose = started.elapsed();
            }
            result
        });
        if let (Some(timing), Some(started)) = (timing.as_mut(), checked_started) {
            timing.gpu_checks = started.elapsed().saturating_sub(timing.compose);
        }
        if let Err(error) = submitted {
            self.converted_for = None;
            self.canvases.discard_candidate();
            return Err(error);
        }
        // Successful validation, conversion and the final Hybrid submission precede
        // publication. The old pair remains charged until its last sampled use ends.
        if self.lost.load(Ordering::Acquire) {
            self.canvases.discard_candidate();
            return Err("The GPU was lost before canvas publication. The document is preserved while graphics recover.".into());
        }
        self.canvases
            .promote_after_submit(desired, plan.is_some())?;
        self.window.pre_present_notify();
        let started = stage_start(timing);
        surface_frame.present();
        if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
            timing.surface_present = started.elapsed();
        }
        self.receipt = plan;
        Ok(Present::Presented)
    }
    fn compose(
        &mut self,
        chrome: &ResolvedScene,
        plan: Option<DisplayPlan>,
        scale: f64,
        target: &wgpu::TextureView,
        timing: &mut Option<PresentTiming>,
    ) -> AppResult<()> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("AFFINT display + MUI"),
            });
        self.scene.reset();
        let mut bindings = TextureBindings::new();
        if let (Some(plan), Some(canvas)) = (plan, self.canvases.sample()) {
            if canvas.key != plan.evaluation.key {
                return Err(
                    "The planned canvas changed before submission; its chrome was not presented."
                        .into(),
                );
            }
            if let Some(timing) = timing.as_mut() {
                timing.sampled_key = Some(canvas.key);
                timing.plan = Some(plan);
            }
            let stamp = DisplayStamp::from(plan);
            if self.converted_for != Some(stamp) {
                let started = stage_start(timing);
                let bind = self.converter.bind(&self.device, &canvas.source_view);
                self.converter.encode_region(
                    &self.queue,
                    &mut encoder,
                    &bind,
                    &canvas.display_view,
                    Conversion {
                        page: plan.artboard,
                        source: SourceMode::LinearPremultiplied,
                        scale: plan.scale as f32,
                        offset: plan.source_offset,
                    },
                    Some(plan.paint_size),
                );
                self.present_stats.conversions_encoded =
                    self.present_stats.conversions_encoded.saturating_add(1);
                if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
                    timing.conversion_encode = started.elapsed();
                    timing.converted = true;
                }
            }
            let started = stage_start(timing);
            self.scene.set_transform(Affine::IDENTITY);
            self.scene
                .set_paint(vello_common::peniko::Color::from_rgb8(29, 29, 29));
            self.scene.fill_rect(&plan.viewport.rect());
            append_canvas_region(
                &mut self.scene,
                &mut bindings,
                &canvas.display_view,
                plan.paint_size,
                plan.viewport,
            )?;
            if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
                timing.canvas_encode = started.elapsed();
            }
        }
        let started = stage_start(timing);
        {
            let mut canvas = mui::vello::Gpu {
                scene: &mut self.scene,
                resources: &mut self.resources,
                cache: &mut self.cache,
                atlas: Some(mui::vello::Atlas {
                    renderer: &mut self.hybrid,
                    device: &self.device,
                    queue: &self.queue,
                }),
            };
            mui::vello::paint(&mut canvas, chrome, Affine::scale(scale))
                .map_err(|error| format!("MUI could not paint this frame: {error}"))?;
        }
        if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
            timing.chrome_paint = started.elapsed();
        }
        let started = stage_start(timing);
        self.hybrid.render(
            &self.scene,
            &mut self.resources,
            &self.device,
            &self.queue,
            &mut encoder,
            &RenderSize {
                width: self.config.width,
                height: self.config.height,
            },
            target,
            &bindings,
        )?;
        if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
            timing.hybrid_encode = started.elapsed();
        }
        let started = stage_start(timing);
        self.queue.submit([encoder.finish()]);
        if let Some(plan) = plan {
            self.converted_for = Some(DisplayStamp::from(plan));
        }
        if let (Some(timing), Some(started)) = (timing.as_mut(), started) {
            timing.submit = started.elapsed();
        }
        Ok(())
    }
    fn retry(&mut self) -> AppResult<Present> {
        self.retries = self.retries.saturating_add(1);
        if self.retries > 3 {
            return Err("The display could not present after three recovery attempts. Your artwork is preserved; try resizing the window or save and reopen AFFINT.".into());
        }
        Ok(Present::Retry)
    }
}

pub fn append_canvas(
    scene: &mut Scene,
    bindings: &mut TextureBindings,
    view: &wgpu::TextureView,
    source: [u32; 2],
    viewport: Viewport,
) -> AppResult<()> {
    let width = u16::try_from(source[0])?;
    let height = u16::try_from(source[1])?;
    if width == 0 || height == 0 {
        return Err("The canvas texture is empty.".into());
    }
    scene.set_transform(Affine::IDENTITY);
    scene.push_clip_path(&viewport.rect().to_path(0.05));
    scene.draw_texture_rects(
        CANVAS_ID,
        ImageQuality::Low,
        [SampleRect {
            source_region: RectU16::new(0, 0, width, height),
            transform: Affine::new([
                f64::from(viewport.width) / f64::from(width),
                0.,
                0.,
                f64::from(viewport.height) / f64::from(height),
                f64::from(viewport.x),
                f64::from(viewport.y),
            ]),
        }],
    );
    scene.pop_clip_path();
    bindings.insert(CANVAS_ID, view.clone());
    Ok(())
}
fn append_canvas_region(
    scene: &mut Scene,
    bindings: &mut TextureBindings,
    view: &wgpu::TextureView,
    visible: [u32; 2],
    viewport: Viewport,
) -> AppResult<()> {
    // Source crop and destination extent are identical physical pixels. The
    // retained display is never fitted or magnified into a different viewport.
    append_canvas(
        scene,
        bindings,
        view,
        visible,
        Viewport {
            width: visible[0],
            height: visible[1],
            ..viewport
        },
    )
}
fn validate_size(width: u32, height: u32, device: &wgpu::Device) -> AppResult<()> {
    let limit = device
        .limits()
        .max_texture_dimension_2d
        .min(u32::from(u16::MAX));
    if width == 0 || height == 0 || width > limit || height > limit {
        return Err(format!("The canvas dimensions exceed this graphics adapter's {limit}px limit. Reduce the window size; no artwork was resized.").into());
    }
    Ok(())
}
pub fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
pub fn texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("AFFINT owned canvas texture"),
        size: extent(width, height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
pub fn artboard_in_request(request: RenderRequest, document_page: Rect) -> Rect {
    match request.view() {
        View::Affine(transform) => transform.affine().transform_rect_bbox(document_page),
        View::Region(region) => Affine::new([
            f64::from(request.width()) / region.width(),
            0.,
            0.,
            f64::from(request.height()) / region.height(),
            -region.x0 * f64::from(request.width()) / region.width(),
            -region.y0 * f64::from(request.height()) / region.height(),
        ])
        .transform_rect_bbox(document_page),
    }
}

/// Explicit diagnostic readback, never called by ordinary presentation.
pub fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> AppResult<Vec<u8>> {
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("AFFINT explicit GPU proof readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        extent(width, height),
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(30)),
    })?;
    receiver.recv_timeout(Duration::from_secs(30))??;
    let mapped = buffer.slice(..).get_mapped_range();
    let mut output = Vec::with_capacity((width * height * 4) as usize);
    for row in mapped.chunks_exact(stride as usize) {
        output.extend_from_slice(&row[..(width * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(output)
}

/// The proof fails explicitly if no adapter exists; it is not a skipped passing test.
pub fn proof(path: &std::path::Path) -> AppResult<()> {
    use mui::prelude::*;
    std::fs::create_dir_all(path)?;
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let info = adapter.get_info();
    std::fs::write(
        path.join("adapter.txt"),
        format!("{info:?}\nExplicit diagnostic readback; no document acceleration claim.\n"),
    )?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let converter = DisplayConverter::new(&device);
    // Half-float constants: highlights above 1, translucent coverage, zero alpha.
    let fixture: [[u16; 4]; 8] = [
        [0x4000, 0x3800, 0x3400, 0x3c00],
        [0x3800, 0x3400, 0x3000, 0x3800],
        [0x3400, 0x3000, 0x2c00, 0x3400],
        [0, 0, 0, 0],
        [0x3c00, 0x3800, 0x3400, 0x3c00],
        [0x3a00, 0x3400, 0, 0x3a00],
        [0, 0x3400, 0x3000, 0x3800],
        [0x3000, 0, 0x2c00, 0x3000],
    ];
    let linear: [[f64; 4]; 8] = [
        [2., 0.5, 0.25, 1.],
        [0.5, 0.25, 0.125, 0.5],
        [0.25, 0.125, 0.0625, 0.25],
        [0., 0., 0., 0.],
        [1., 0.5, 0.25, 1.],
        [0.75, 0.25, 0., 0.75],
        [0., 0.25, 0.125, 0.5],
        [0.125, 0., 0.0625, 0.125],
    ];
    let mut bytes = Vec::with_capacity(64);
    for pixel in fixture {
        for channel in pixel {
            bytes.extend_from_slice(&channel.to_le_bytes());
        }
    }
    let source = texture(
        &device,
        4,
        2,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &source,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(32),
            rows_per_image: Some(2),
        },
        extent(4, 2),
    );
    let display = texture(
        &device,
        4,
        2,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
    );
    let display_view = display.create_view(&Default::default());
    let bind = converter.bind(&device, &source.create_view(&Default::default()));
    let mut encoder = device.create_command_encoder(&Default::default());
    converter.encode(
        &queue,
        &mut encoder,
        &bind,
        &display_view,
        Conversion {
            page: Rect::new(0., 0., 4., 2.),
            source: SourceMode::LinearPremultiplied,
            scale: 1.,
            offset: [0, 0],
        },
    );
    queue.submit([encoder.finish()]);
    let result = readback(&device, &queue, &display, 4, 2)?;
    crate::report_durability(affint_storage::save_png(
        path.join("float-display.png"),
        affint_document::PixelSize::new(4, 2)?,
        &result,
    )?);
    let background = crate::editor::decode_srgb(0.78);
    let mut maximum = 0u8;
    let mut opaque = true;
    let (pixels, remainder) = result.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err("GPU proof returned incomplete pixels.".into());
    }
    for (pixel, expected) in pixels.iter().zip(linear) {
        for channel in 0..3 {
            let value = (expected[channel] + background * (1. - expected[3])).clamp(0., 1.);
            let encoded = if value <= 0.0031308 {
                value * 12.92
            } else {
                1.055 * value.powf(1. / 2.4) - 0.055
            };
            maximum = maximum.max(pixel[channel].abs_diff((encoded * 255.).round() as u8));
        }
        opaque &= pixel[3] == 255;
    }
    let target = texture(
        &device,
        64,
        32,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let (mut hybrid, mut resources) = Renderer::new(
        &device,
        &RenderTargetConfig {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: 64,
            height: 32,
        },
    );
    let mut scene = Scene::new(64, 32);
    let mut bindings = TextureBindings::new();
    append_canvas(
        &mut scene,
        &mut bindings,
        &display_view,
        [4, 2],
        Viewport {
            x: 8,
            y: 8,
            width: 32,
            height: 16,
        },
    )?;
    let font = Font::new(epaint_default_fonts::UBUNTU_LIGHT)?;
    let mut ui = mui::Ui::new(Theme {
        text: 12.,
        ..Theme::DEFAULT
    })
    .font(font);
    let root = overlay([leaf(10., 32.)
        .fill(mui::scene::Color::srgb(0.1, 0.2, 0.8))
        .radius(0.)
        .anchor(Align::Start, Align::Start)
        .id("proof-chrome")
        .offset(28., 0.)])
    .size(64., 32.)
    .no_fill();
    let frame = ui.frame(
        root,
        Some(Size::new(64., 32.)),
        mui::prelude::Input::default(),
        0.,
    )?;
    let chrome = frame
        .scene
        .surface("proof-chrome")
        .ok_or("GPU proof chrome did not resolve.")?
        .frame;
    let chrome_rect = Rect::new(
        chrome.x,
        chrome.y,
        chrome.x + chrome.size.width,
        chrome.y + chrome.size.height,
    );
    let overlap = chrome_rect.intersect(Rect::new(8., 8., 40., 24.));
    let sample = [
        ((overlap.x0 + overlap.x1) / 2.).floor() as usize,
        ((overlap.y0 + overlap.y1) / 2.).floor() as usize,
    ];
    let paint = frame
        .scene
        .paint
        .iter()
        .find(|paint| {
            paint.key.as_ref() == "proof-chrome" && paint.layer == mui::scene::Layer::Fill
        })
        .ok_or("GPU proof chrome has no fill.")?;
    let mui::scene::Paint::Solid(solid) = &paint.paint else {
        return Err("GPU proof chrome has no solid fill.".into());
    };
    let expected_chrome = solid
        .to_srgb()
        .components
        .map(|channel| (channel.clamp(0., 1.) * 255.).round() as u8);
    let mut cache = mui::vello::Cache::default();
    let mut canvas = mui::vello::Gpu {
        scene: &mut scene,
        resources: &mut resources,
        cache: &mut cache,
        atlas: Some(mui::vello::Atlas {
            renderer: &mut hybrid,
            device: &device,
            queue: &queue,
        }),
    };
    mui::vello::paint(&mut canvas, frame.scene, Affine::IDENTITY)?;
    let mut encoder = device.create_command_encoder(&Default::default());
    hybrid.render(
        &scene,
        &mut resources,
        &device,
        &queue,
        &mut encoder,
        &RenderSize {
            width: 64,
            height: 32,
        },
        &target.create_view(&Default::default()),
        &bindings,
    )?;
    queue.submit([encoder.finish()]);
    let composed = readback(&device, &queue, &target, 64, 32)?;
    crate::report_durability(affint_storage::save_png(
        path.join("mui-seam.png"),
        affint_document::PixelSize::new(64, 32)?,
        &composed,
    )?);
    let at = |x: usize, y: usize| &composed[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
    std::fs::write(
        path.join("observations.txt"),
        format!(
            "Adapter: {info:?}\nDisplay maximum channel error: {maximum}/255\nResolved chrome: {chrome:?}\nCanvas/chrome overlap: {overlap:?}\nOverlap sample: {sample:?}\nExpected chrome: {expected_chrome:?}\nActual original hardcoded sample (30,12): {:?}\nActual derived sample: {:?}\nCanvas source sample: {:?}\nCanvas composed sample (12,12): {:?}\nOutside sample (2,2): {:?}\n",
            at(30, 12),
            at(sample[0].min(63), sample[1].min(31)),
            &result[0..4],
            at(12, 12),
            at(2, 2)
        ),
    )?;
    if !opaque {
        return Err(
            "GPU display proof produced non-opaque output; inspect float-display.png.".into(),
        );
    }
    if maximum > 1 {
        return Err(format!("GPU display proof differs from linear-light reference by {maximum}/255; allowed 1/255. Inspect observations.txt and float-display.png.").into());
    }
    if overlap.width() < 2.
        || overlap.height() < 2.
        || sample[0] >= 64
        || sample[1] >= 32
        || !chrome_rect.contains(kurbo::Point::new(
            sample[0] as f64 + 0.5,
            sample[1] as f64 + 0.5,
        ))
    {
        return Err("GPU proof chrome has no legitimate interior overlap with the canvas; inspect observations.txt.".into());
    }
    if at(2, 2) != [0, 0, 0, 0] {
        return Err("External canvas leaked outside its physical viewport clip.".into());
    }
    if at(12, 12)
        .iter()
        .zip(&result[0..4])
        .any(|(a, b)| a.abs_diff(*b) > 1)
    {
        return Err("Hybrid did not preserve the bound document sample.".into());
    }
    if expected_chrome[3] != 255
        || at(sample[0], sample[1])
            .iter()
            .zip(expected_chrome)
            .any(|(actual, expected)| actual.abs_diff(expected) > 1)
    {
        return Err("MUI chrome was not above the external document texture; inspect observations.txt and mui-seam.png.".into());
    }
    document_proof::run(path, &adapter, &device, &queue)?;
    if let Some(error) = pollster::block_on(validation.pop()) {
        return Err(format!("GPU proof validation: {error}").into());
    }
    std::fs::write(
        path.join("proof.txt"),
        format!(
            "Adapter: {info:?}\nRGBA16Float linear-premultiplied diagnostic source with >1 highlight and translucent coverage.\nDisplay maximum absolute channel error: {maximum}/255, tolerance 1/255.\nExternal TextureBindings + clipped SampleRect preserved pixels; MUI chrome above canvas; no clip leak.\nReal-document F32 uploader/converter/Hybrid proof also passed; inspect working-proof.txt and working-observations.txt.\nReadback: explicit diagnostic only. CPU reference remains ordinary document evaluator.\n"
        ),
    )?;
    println!(
        "GPU display and MUI texture seam proof passed: {}",
        path.display()
    );
    Ok(())
}
