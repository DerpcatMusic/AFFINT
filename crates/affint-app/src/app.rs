//! Native editor input, document commands and host ownership.
mod chrome;
mod profile;
use std::sync::{
    Arc,
    atomic::Ordering,
    mpsc::{self, Receiver},
};
use std::time::{Duration, Instant};

use affint_document::{Change, Content, History, LiveOperation, NodeId, Revision, Transform};
use affint_render::{Frame as DocumentFrame, OutputFormat, RenderRequest, Renderer, View};
use affint_storage::{Durability, FileLimits, SourceColor, Store};
use kurbo::{Affine, Point as DocumentPoint, Rect};
use mui::prelude::*;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime as NativeIme, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key as NativeKey, NamedKey},
    window::{CursorIcon, Window, WindowId},
};

use crate::{
    Wake,
    editor::{AppResult, Editor, Tool, document_limits, layer_spec, selection_bounds},
    gpu::{
        CameraIntent, DisplayPlan, Evaluation, Host, OVERSCAN, Present, Viewport,
        artboard_in_request, padded_request, prefetch_fits,
    },
    jobs::{
        ClipboardJob, ClipboardResult, ClipboardWorker, FileJob, FileResult, FileWorker,
        RenderWorker,
    },
    upload::{Admission, Key as UploadKey, Worker as UploadWorker},
};

const INK: [f32; 3] = [242. / 255.; 3];
const MUTED: [f32; 3] = [173. / 255.; 3];
const CHROME: [f32; 3] = [61. / 255.; 3];
const STRIP: [f32; 3] = [31. / 255.; 3];
const ACCENT: [f32; 3] = [0., 122. / 255., 1.];
const SWATCHES: [[f64; 4]; 5] = [
    [0.96, 0.36, 0.27, 1.],
    [0.13, 0.28, 0.93, 1.],
    [0.96, 0.93, 0.82, 1.],
    [0.20, 0.67, 0.51, 1.],
    [0.20, 0.22, 0.28, 1.],
];

fn color(rgb: [f32; 3]) -> mui::scene::Color {
    mui::scene::Color::srgb(rgb[0], rgb[1], rgb[2])
}
fn label_text(value: impl Into<String>) -> El {
    text(value).text_size(10.).fill(color(INK))
}
fn muted(value: impl Into<String>) -> El {
    text(value).text_size(9.).fill(color(MUTED))
}
fn panel(children: impl IntoIterator<Item = El>) -> El {
    column(children).fill(color(CHROME)).gap(8.).pad(12.)
}

#[derive(Clone, Copy)]
enum Command {
    New,
    Open,
    Import,
    Save,
    SaveAs,
    Export,
    Quit,
    Undo,
    Redo,
    Delete,
    Raise,
    Lower,
    Fit,
    Actual,
    ZoomIn,
    ZoomOut,
    PreviousPage,
    NextPage,
    AddPage,
}
#[derive(Clone, Copy)]
enum AfterSave {
    Close,
    New,
    Open,
}
#[derive(Debug, PartialEq, Eq)]
enum SaveDisposition {
    ContinueEditing,
    Proceed,
    NeedsAttention(String),
    NewerEdits,
}
struct UiEffects {
    animating: bool,
    repaint_after: Option<Duration>,
    clipboard: Option<String>,
    ime: Option<(Point, Size)>,
    cursor: Cursor,
}
impl UiEffects {
    fn from_frame(frame: mui::Frame<'_>) -> Self {
        Self {
            animating: frame.animating,
            repaint_after: frame.repaint_after,
            clipboard: frame.clipboard,
            ime: frame.ime,
            cursor: frame.cursor,
        }
    }
    fn then(self, later: Self) -> Self {
        let repaint_after = match (self.repaint_after, later.repaint_after) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        Self {
            animating: self.animating || later.animating,
            repaint_after,
            clipboard: later.clipboard.or(self.clipboard),
            ime: later.ime,
            cursor: later.cursor,
        }
    }
}

struct ViewState {
    zoom: f64,
    center: DocumentPoint,
    fitted: bool,
}
#[derive(Default)]
struct Navigation {
    quiet_at: Option<Instant>,
    retry_at: Option<Instant>,
    zooming: bool,
    submitted_at: Option<Instant>,
}
impl Navigation {
    fn changed(&mut self, zoom: bool) {
        self.changed_at(Instant::now(), zoom);
    }
    fn changed_at(&mut self, now: Instant, zoom: bool) {
        self.quiet_at = Some(now + Duration::from_millis(100));
        self.retry_at = None;
        self.zooming |= zoom;
    }
    fn deadline(&self) -> Option<Instant> {
        self.retry_at.or(self.quiet_at)
    }
    fn ready(&mut self, now: Instant, busy: bool, margin: Option<f64>) -> bool {
        if self.zooming && self.quiet_at.is_some_and(|deadline| now < deadline) {
            return false;
        }
        if busy {
            self.retry_at = Some(now + Duration::from_millis(16));
            return false;
        }
        if self.quiet_at.is_none_or(|deadline| now >= deadline)
            || margin.is_none_or(|margin| margin <= 32.)
            || self
                .submitted_at
                .is_none_or(|last| now.duration_since(last) >= Duration::from_millis(100))
        {
            return true;
        }
        self.retry_at = None;
        false
    }
    fn submitted(&mut self, now: Instant) {
        self.submitted_at = Some(now);
        self.retry_at = None;
        if self.quiet_at.is_some_and(|deadline| now >= deadline) {
            self.quiet_at = None;
            self.zooming = false;
        }
    }
    fn settled(&mut self, now: Instant) {
        self.retry_at = None;
        if self.quiet_at.is_some_and(|deadline| now >= deadline) {
            self.quiet_at = None;
            self.retry_at = None;
            self.zooming = false;
        }
    }
}
impl ViewState {
    fn new() -> Self {
        Self {
            zoom: 1.,
            center: DocumentPoint::new(480., 360.),
            fitted: false,
        }
    }
    fn affine(&self, size: Size, dpi: f64) -> Affine {
        let scale = self.zoom * dpi;
        Affine::new([
            scale,
            0.,
            0.,
            scale,
            size.width * dpi / 2. - self.center.x * scale,
            size.height * dpi / 2. - self.center.y * scale,
        ])
    }
    fn document(&self, local: Point, size: Size) -> DocumentPoint {
        DocumentPoint::new(
            self.center.x + (local.x - size.width / 2.) / self.zoom,
            self.center.y + (local.y - size.height / 2.) / self.zoom,
        )
    }
    fn zoom_about(&mut self, factor: f64, fixed: DocumentPoint, local: Point, size: Size) {
        self.zoom = (self.zoom * factor).clamp(0.02, 32.);
        self.center = DocumentPoint::new(
            fixed.x - (local.x - size.width / 2.) / self.zoom,
            fixed.y - (local.y - size.height / 2.) / self.zoom,
        );
        self.fitted = true;
    }
}

pub struct App {
    editor: Editor,
    ui: Ui,
    window: Option<Arc<Window>>,
    host: Option<Host>,
    startup: Option<Receiver<AppResult<Host>>>,
    startup_attempts: u8,
    proxy: EventLoopProxy<Wake>,
    renderer: Renderer,
    render: Option<RenderWorker>,
    upload: UploadWorker,
    files: Option<FileWorker>,
    clipboard: Option<ClipboardWorker>,
    file_busy: bool,
    dialog_open: bool,
    after_save: Option<AfterSave>,
    uncertain_save: Option<String>,
    close_ready: bool,
    menu: Option<&'static str>,
    commands: Vec<Command>,
    error: Option<String>,
    status: String,
    pointer: PointerInput,
    motion: Option<PointerInput>,
    viewport: Option<mui::layout::Frame>,
    physical_viewport: Option<Viewport>,
    view: ViewState,
    page_layout_pending: bool,
    pan_last: Option<Point>,
    canvas_last: Option<DocumentPoint>,
    canvas_dragged: bool,
    desired: Option<(Revision, RenderRequest, u64, u64)>,
    generation: u64,
    repaint_at: Option<Instant>,
    last_tick: Instant,
    visible: bool,
    name: String,
    name_node: Option<NodeId>,
    name_edited: bool,
    profile: Option<profile::Recorder>,
    chrome: chrome::ChromeState,
    evaluation: Option<Evaluation>,
    intent: Option<CameraIntent>,
    chrome_plan: Option<DisplayPlan>,
    chrome_input: bool,
    navigation: Navigation,
    canvas_session: Option<Revision>,
    overscan_refused: Option<(Revision, [u32; 2])>,
}
impl App {
    pub fn new(proxy: EventLoopProxy<Wake>) -> AppResult<Self> {
        let font = Font::new(&include_bytes!("../assets/fonts/AdwaitaSans-Regular.ttf")[..])?;
        let theme = Theme {
            palette: Palette {
                mode: Mode::Dark,
                ..Palette::NEUTRAL
            },
            corners: Corners {
                box_: 3.,
                field: 3.,
                selector: 3.,
                ..Corners::DEFAULT
            },
            text: 10.,
            control: 22.,
            ..Theme::DEFAULT
        };
        let renderer = Renderer::new_retained(
            affint_render::Limits {
                max_bytes: 256 * 1024 * 1024,
                max_pixels: 24_000_000,
                ..Default::default()
            },
            128 * 1024 * 1024,
        )?;
        let upload = UploadWorker::new(proxy.clone())?;
        Ok(Self {
            editor: Editor::demo()?,
            ui: Ui::new(theme).font(font),
            window: None,
            host: None,
            startup: None,
            startup_attempts: 0,
            proxy,
            renderer,
            render: None,
            upload,
            files: None,
            clipboard: None,
            file_busy: false,
            dialog_open: false,
            after_save: None,
            uncertain_save: None,
            close_ready: false,
            menu: None,
            commands: Vec::new(),
            error: None,
            status: "Original synthetic study · editable vector + raster".into(),
            pointer: PointerInput::default(),
            motion: None,
            viewport: None,
            physical_viewport: None,
            view: ViewState::new(),
            page_layout_pending: true,
            pan_last: None,
            canvas_last: None,
            canvas_dragged: false,
            desired: None,
            generation: 1,
            repaint_at: None,
            last_tick: Instant::now(),
            visible: true,
            name: String::new(),
            name_node: None,
            name_edited: false,
            profile: crate::jobs::diagnostics_enabled().then(profile::Recorder::new),
            chrome: chrome::ChromeState::default(),
            evaluation: None,
            intent: None,
            chrome_plan: None,
            chrome_input: true,
            navigation: Navigation::default(),
            canvas_session: None,
            overscan_refused: None,
        })
    }
    fn fail(&mut self, error: impl std::fmt::Display) {
        let message = error.to_string();
        if self.error.as_deref() != Some(message.as_str()) {
            self.error = Some(message);
            self.redraw();
        }
    }
    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn blocked(&self) -> bool {
        self.after_save.is_some() || self.dialog_open
    }
    fn tick(&mut self, input: Input) -> AppResult<()> {
        self.refresh(input, true)
    }
    fn paint(&mut self) -> AppResult<()> {
        self.refresh(Input::from(self.pointer), false)
    }
    fn refresh(&mut self, input: Input, consume_input: bool) -> AppResult<()> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let physical = window.inner_size();
        if physical.width == 0 || physical.height == 0 {
            return Ok(());
        }
        let dpi = window.scale_factor();
        let size = Size::new(
            f64::from(physical.width) / dpi,
            f64::from(physical.height) / dpi,
        );
        self.ui.scale = Some(dpi);
        self.chrome_plan = self.host.as_ref().and_then(Host::receipt);
        let measurement = self.profile.as_ref().map(|_| Instant::now());
        let mut tree_time = Duration::ZERO;
        let mut frame_time = Duration::ZERO;
        let mut calls = 0;
        if consume_input && let Some(profile) = &mut self.profile {
            profile.input();
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        let stage = measurement.map(|_| Instant::now());
        let root = self.tree(size)?;
        if let Some(stage) = stage {
            tree_time += stage.elapsed();
        }
        let stage = measurement.map(|_| Instant::now());
        let mut effects = UiEffects::from_frame(self.ui.frame(root, Some(size), input, dt)?);
        if let Some(stage) = stage {
            frame_time += stage.elapsed();
        }
        calls += 1;
        if consume_input {
            self.canvas_input()?;
            // Only consumed input needs the widget response replay. A graphics
            // wake or animation repaint resolves one tree and consumes no canvas edit.
            let stage = measurement.map(|_| Instant::now());
            let root = self.tree(size)?;
            if let Some(stage) = stage {
                tree_time += stage.elapsed();
            }
            let stage = measurement.map(|_| Instant::now());
            effects = effects.then(UiEffects::from_frame(self.ui.frame(
                root,
                Some(size),
                Input::from(self.pointer),
                0.,
            )?));
            if let Some(stage) = stage {
                frame_time += stage.elapsed();
            }
            calls += 1;
        }
        let frame = self
            .ui
            .scene()
            .and_then(|scene| scene.surface("viewport"))
            .map(|surface| surface.frame)
            .ok_or("MUI could not resolve the artwork viewport.")?;
        self.viewport = Some(frame);
        self.physical_viewport = snapped_viewport(frame, dpi, physical.width, physical.height);
        if !self.view.fitted {
            self.fit()?;
        }
        let commands = std::mem::take(&mut self.commands);
        for command in commands {
            self.command(command)?;
        }
        if self.page_layout_pending {
            // One bounded layout refresh removes old-page layers/selection in this
            // presentation; no input edge is replayed or document command repeated.
            self.fit()?;
            let stage = measurement.map(|_| Instant::now());
            let root = self.tree(size)?;
            if let Some(stage) = stage {
                tree_time += stage.elapsed();
            }
            let stage = measurement.map(|_| Instant::now());
            effects = effects.then(UiEffects::from_frame(self.ui.frame(
                root,
                Some(size),
                Input::from(self.pointer),
                0.,
            )?));
            if let Some(stage) = stage {
                frame_time += stage.elapsed();
            }
            calls += 1;
            self.page_layout_pending = false;
        }
        if let (Some(text), Some(clipboard)) = (effects.clipboard, &self.clipboard) {
            clipboard.submit(ClipboardJob::Write(text))?;
        }
        self.repaint_at = effects
            .repaint_after
            .and_then(|delay| now.checked_add(delay));
        if effects.animating {
            self.repaint_at = Some(now + Duration::from_millis(16));
        }
        if let Some(window) = &self.window {
            window.set_ime_allowed(effects.ime.is_some());
            if let Some((point, size)) = effects.ime {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(point.x * dpi, point.y * dpi),
                    PhysicalSize::new(size.width * dpi, size.height * dpi),
                );
            }
            window.set_cursor(match effects.cursor {
                Cursor::Crosshair => CursorIcon::Crosshair,
                Cursor::Grab => CursorIcon::Grab,
                Cursor::Grabbing => CursorIcon::Grabbing,
                Cursor::Text => CursorIcon::Text,
                Cursor::Hand => CursorIcon::Pointer,
                _ => CursorIcon::Default,
            });
            window.set_title(&format!(
                "{}{} — AFFINT",
                self.editor.title,
                if self.editor.dirty() || self.name_edited {
                    " •"
                } else {
                    ""
                }
            ));
        }
        self.schedule_render()?;
        if let Some(profile) = &mut self.profile {
            profile.metrics(
                profile::Metrics {
                    physical: [physical.width, physical.height],
                    logical: [size.width, size.height],
                    scale_factor: dpi,
                    viewport: self.physical_viewport,
                    logical_viewport: self
                        .viewport
                        .map(|frame| [frame.x, frame.y, frame.size.width, frame.size.height]),
                },
                self.renderer.retained_stats(),
            );
        }
        if let (Some(profile), Some(started)) = (&mut self.profile, measurement) {
            profile.ui(
                consume_input,
                tree_time,
                frame_time,
                started.elapsed(),
                calls,
                upload_key(self.desired),
            );
        }
        Ok(())
    }
    fn canvas_input(&mut self) -> AppResult<()> {
        if self.blocked() || self.menu.is_some() {
            return Ok(());
        }
        let response = self.ui.get("viewport");
        let Some(_frame) = self.viewport else {
            return Ok(());
        };
        let Some(pointer) = self.pointer.pos else {
            return Ok(());
        };
        if response.pressed {
            self.canvas_dragged = false;
            self.canvas_last = None;
            self.canvas_session = None;
            if self.editor.tool == Tool::Pan || response.button == Some(Button::Middle) {
                self.pan_last = Some(pointer);
            } else if response.button == Some(Button::Primary) {
                self.commit_name()?;
                let plan = self.host.as_ref().and_then(Host::receipt);
                let Some(point) =
                    canvas_entry_point(plan, self.editor.session.snapshot().revision(), pointer)
                else {
                    self.status = "Updating artwork before the next canvas edit…".into();
                    return Ok(());
                };
                self.editor.press(point)?;
                self.canvas_session = Some(self.editor.session.snapshot().revision());
                self.canvas_last = Some(point);
            }
        }
        if response.held {
            self.canvas_dragged |= response.dragged;
            if let Some(last) = self.pan_last.filter(|last| *last != pointer) {
                let displayed_zoom = self
                    .host
                    .as_ref()
                    .and_then(Host::receipt)
                    .map_or(self.view.zoom, DisplayPlan::zoom);
                self.view.center.x -= (pointer.x - last.x) / displayed_zoom;
                self.view.center.y -= (pointer.y - last.y) / displayed_zoom;
                self.pan_last = Some(pointer);
                self.view.fitted = true;
                self.navigation.changed(false);
            } else if self.pan_last.is_none() && self.canvas_session.is_some() && response.dragged {
                let Some(point) =
                    self.host.as_ref().and_then(Host::receipt).and_then(|plan| {
                        plan.document_point(DocumentPoint::new(pointer.x, pointer.y))
                    })
                else {
                    return Ok(());
                };
                if changed_canvas_point(&mut self.canvas_last, point) {
                    self.editor.drag(point)?;
                }
            }
        }
        if response.released && (self.pan_last.is_some() || self.canvas_session.is_some()) {
            self.pan_last = None;
            self.canvas_last = None;
            self.canvas_session = None;
            // A click-sized shape/crop is canceled; move with no updates commits no edit.
            if !self.canvas_dragged
                && matches!(
                    self.editor.tool,
                    Tool::Rectangle | Tool::Ellipse | Tool::Crop
                )
            {
                self.editor.cancel()?;
            } else {
                self.editor.commit()?;
            }
        }
        Ok(())
    }
    fn fit(&mut self) -> AppResult<()> {
        let Some(frame) = self.viewport else {
            return Ok(());
        };
        let snapshot = self.editor.snapshot();
        let size = snapshot
            .page(self.editor.page()?)
            .ok_or("The page is missing.")?
            .size();
        self.view.zoom = ((frame.size.width - 64.).max(1.) / size.width())
            .min((frame.size.height - 64.).max(1.) / size.height())
            .clamp(0.02, 32.);
        self.view.center = DocumentPoint::new(size.width() / 2., size.height() / 2.);
        self.view.fitted = true;
        Ok(())
    }
    fn schedule_render(&mut self) -> AppResult<()> {
        let (Some(viewport), Some(logical), Some(render)) =
            (self.physical_viewport, self.viewport, &self.render)
        else {
            return Ok(());
        };
        let dpi = self
            .window
            .as_ref()
            .map_or(1., |window| window.scale_factor());
        let snapshot = self.editor.snapshot();
        let origin = Affine::translate((
            logical.x * dpi - f64::from(viewport.x),
            logical.y * dpi - f64::from(viewport.y),
        ));
        let camera_request = RenderRequest::new(
            self.editor.page()?,
            viewport.width,
            viewport.height,
            View::Affine(Transform::new(
                (origin * self.view.affine(logical.size, dpi)).as_coeffs(),
            )?),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let intent = CameraIntent::new(snapshot.revision(), camera_request, viewport, dpi)?;
        let revision = intent.revision();
        self.intent = Some(intent);
        let padded = padded_request(intent.request(), OVERSCAN)?;
        let limits = self.renderer.limits();
        let gpu_dimension = self.host.as_ref().map_or(limits.max_dimension, |host| {
            host.device.limits().max_texture_dimension_2d
        });
        let padded_allowed = self.overscan_refused
            != Some((revision, [viewport.width, viewport.height]))
            && prefetch_fits(padded, limits, gpu_dimension)
            && self.upload.admission(padded)? != Admission::Refused;
        let (request, margin) = if padded_allowed {
            (padded, OVERSCAN)
        } else {
            (camera_request, 0)
        };
        let now = Instant::now();
        if self
            .desired
            .is_some_and(|(old_revision, old_request, generation, _)| {
                old_revision == revision && old_request == request && generation == self.generation
            })
        {
            self.navigation.settled(now);
            return Ok(());
        }
        let document_changed =
            self.desired
                .is_none_or(|(old_revision, old_request, generation, _)| {
                    old_revision != revision
                        || old_request.page() != request.page()
                        || generation != self.generation
                });
        if !document_changed {
            let stats = render.stats()?;
            let upload_pending = if let Some((_, old, _, _)) = self.desired {
                self.upload.admission(old)? == Admission::Pending
            } else {
                false
            };
            let busy = stats.active_serial.is_some()
                || stats.pending_serial.is_some()
                || stats.result_pending
                || self.host.as_ref().is_some_and(Host::has_candidate)
                || upload_pending;
            let remaining = if let Some(host) = &self.host {
                host.plan(Some(intent))?.map(DisplayPlan::coverage_margin)
            } else {
                None
            };
            if !self.navigation.ready(now, busy, remaining) {
                return Ok(());
            }
        }
        let serial = render.submit(snapshot, request, self.generation)?;
        self.desired = Some((revision, request, self.generation, serial));
        let evaluation = Evaluation::new(
            UploadKey {
                revision,
                request,
                generation: self.generation,
                serial,
            },
            intent,
            margin,
        )?;
        self.evaluation = Some(evaluation);
        self.navigation.submitted(now);
        self.upload.desire(upload_key(self.desired))?;
        if let Some(host) = &mut self.host {
            host.register_evaluation(evaluation);
            host.keep_desired(self.desired);
        }
        Ok(())
    }
    fn command(&mut self, command: Command) -> AppResult<()> {
        if self.dialog_open {
            return Ok(());
        }
        if self.after_save.is_some() && !matches!(command, Command::Save | Command::SaveAs) {
            return Ok(());
        }
        if self.file_busy && matches!(command, Command::Quit) {
            return Ok(());
        }
        self.menu = None;
        if self.editor.gesture_active() {
            self.editor.cancel()?;
            self.canvas_session = None;
            self.canvas_last = None;
        }
        self.commit_name()?;
        match command {
            Command::Quit => self.request_replace(AfterSave::Close)?,
            Command::New => self.request_replace(AfterSave::New)?,
            Command::Open => self.request_replace(AfterSave::Open)?,
            Command::Import => self.file(
                FileJob::Import {
                    page: self.editor.page()?,
                },
                true,
            )?,
            Command::Save | Command::SaveAs => {
                let path = if matches!(command, Command::SaveAs) {
                    None
                } else {
                    self.editor.path.clone()
                };
                let dialog = path.is_none();
                self.file(
                    FileJob::Save {
                        snapshot: self.editor.session.snapshot(),
                        path,
                    },
                    dialog,
                )?;
            }
            Command::Export => {
                let (snapshot, request) = self.editor.export_request()?;
                self.file(FileJob::Export { snapshot, request }, true)?;
            }
            Command::Undo => {
                if self.editor.session.history_status().undo > 0 {
                    let page = self.editor.page()?;
                    self.editor.history(History::Undo)?;
                    if self.editor.page()? != page {
                        self.reset_page_view()?;
                    }
                }
            }
            Command::Redo => {
                if self.editor.session.history_status().redo > 0 {
                    let page = self.editor.page()?;
                    self.editor.history(History::Redo)?;
                    if self.editor.page()? != page {
                        self.reset_page_view()?;
                    }
                }
            }
            Command::Delete => self.editor.remove()?,
            Command::Raise => self.editor.reorder(true)?,
            Command::Lower => self.editor.reorder(false)?,
            Command::AddPage => {
                self.editor.add_page()?;
                self.reset_page_view()?;
                self.status = "Blank page added · draw a shape or import an original PNG".into();
            }
            Command::PreviousPage | Command::NextPage => {
                if let Some(page) = self
                    .editor
                    .adjacent_page(matches!(command, Command::NextPage))?
                    && self.editor.switch_page(page)?
                {
                    self.reset_page_view()?;
                    self.status = "Active page changed · editable originals preserved".into();
                }
            }
            Command::Fit => {
                self.fit()?;
                self.navigation.changed(true);
            }
            Command::Actual => {
                self.view.zoom = 1.;
                self.view.fitted = true;
                self.navigation.changed(true);
            }
            Command::ZoomIn | Command::ZoomOut => {
                if let Some(frame) = self.viewport {
                    let local = Point::new(frame.size.width / 2., frame.size.height / 2.);
                    let fixed = self
                        .host
                        .as_ref()
                        .and_then(Host::receipt)
                        .and_then(|plan| {
                            plan.document_point(DocumentPoint::new(
                                frame.x + local.x,
                                frame.y + local.y,
                            ))
                        })
                        .unwrap_or_else(|| self.view.document(local, frame.size));
                    self.view.zoom_about(
                        if matches!(command, Command::ZoomIn) {
                            1.2
                        } else {
                            1. / 1.2
                        },
                        fixed,
                        local,
                        frame.size,
                    );
                    self.navigation.changed(true);
                }
            }
        }
        self.redraw();
        Ok(())
    }
    fn reset_page_view(&mut self) -> AppResult<()> {
        self.motion = None;
        self.pointer.buttons = Buttons::default();
        self.pan_last = None;
        self.canvas_last = None;
        self.chrome.wheel_drag = None;
        self.chrome.opacity_dispatch.reset();
        self.canvas_session = None;
        self.intent = None;
        self.evaluation = None;
        self.chrome_plan = None;
        self.navigation = Navigation::default();
        self.overscan_refused = None;
        self.canvas_dragged = false;
        self.ui.cancel();
        self.view = ViewState::new();
        self.page_layout_pending = true;
        self.name_node = None;
        self.name.clear();
        self.name_edited = false;
        self.desired = None;
        self.upload.desire(None)?;
        if let Some(host) = &mut self.host {
            host.clear_canvas();
        }
        Ok(())
    }
    fn file(&mut self, job: FileJob, dialog: bool) -> AppResult<()> {
        if self.file_busy {
            return Err("A file operation is already running. Wait for it to finish.".into());
        }
        self.files
            .as_ref()
            .ok_or("The file worker is not ready.")?
            .submit(job)?;
        self.file_busy = true;
        self.dialog_open = dialog;
        if dialog {
            self.ui.cancel();
        }
        Ok(())
    }
    fn request_replace(&mut self, next: AfterSave) -> AppResult<()> {
        if self.uncertain_save.is_some() {
            return Ok(());
        }
        self.editor.cancel()?;
        self.commit_name()?;
        if self.editor.dirty() {
            self.after_save = Some(next);
            self.ui.cancel();
            self.ui.focus("modal-cancel");
            self.redraw();
        } else {
            self.after_save = Some(next);
            self.finish_after_save();
        }
        Ok(())
    }
    fn commit_name(&mut self) -> AppResult<()> {
        if !self.name_edited {
            return Ok(());
        }
        let Some(node) = self.name_node else {
            return Ok(());
        };
        if self.editor.selected_node()? != Some(node) {
            return Err("The renamed layer is no longer selected on this page.".into());
        }
        let snapshot = self.editor.session.snapshot();
        let mut spec = layer_spec(
            snapshot
                .node(node)
                .ok_or("The renamed layer no longer exists.")?,
        );
        spec.name = self.name.clone();
        self.editor
            .transact("Rename layer", [Change::SetLayer { node, layer: spec }])?;
        self.name_edited = false;
        Ok(())
    }
    fn finish_after_save(&mut self) {
        self.uncertain_save = None;
        let Some(next) = self.after_save.take() else {
            return;
        };
        let result = match next {
            AfterSave::Close => {
                self.close_ready = true;
                Ok(())
            }
            AfterSave::New => Editor::demo().and_then(|editor| {
                self.editor = editor;
                self.reset_page_view()
            }),
            AfterSave::Open => self.file(FileJob::Open, true),
        };
        if let Err(error) = result {
            self.fail(error);
        }
        self.redraw();
    }
    fn cancel(&mut self) -> AppResult<()> {
        self.motion = None;
        self.pointer.buttons = Buttons::default();
        self.pan_last = None;
        self.canvas_last = None;
        self.chrome.wheel_drag = None;
        self.chrome.opacity_dispatch.reset();
        self.canvas_session = None;
        self.ui.cancel();
        self.editor.cancel()?;
        Ok(())
    }
    fn process_motion(&mut self) -> AppResult<bool> {
        if let Some(pointer) = self.motion.take() {
            self.pointer = pointer;
            self.tick(Input::from(pointer))?;
            return Ok(true);
        }
        Ok(false)
    }
    fn receive(&mut self) -> AppResult<()> {
        if let Some(startup) = &self.startup {
            match startup.try_recv() {
                Ok(Ok(host)) => {
                    self.host = Some(host);
                    if let (Some(host), Some(evaluation)) = (&mut self.host, self.evaluation) {
                        host.register_evaluation(evaluation);
                    }
                    self.startup = None;
                    if let Some(window) = &self.window {
                        window.set_visible(true);
                    }
                    self.paint()?;
                }
                Ok(Err(error)) => {
                    self.startup = None;
                    self.fail(error);
                    if let Some(window) = &self.window {
                        window.set_visible(true);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.startup = None;
                    self.fail("The graphics startup worker stopped unexpectedly.");
                }
            }
        }
        self.receive_canvas()?;
        if let Some(files) = &self.files
            && let Some(result) = files.take()?
        {
            self.file_busy = false;
            self.dialog_open = false;
            match result {
                Err(error) => self.fail(format!(
                    "File operation failed: {error}. Your current artwork is preserved."
                )),
                Ok(FileResult::Canceled) => {
                    self.status = "File dialog canceled".into();
                }
                Ok(FileResult::Open { session, path }) => {
                    self.editor = Editor::from_loaded(*session, path);
                    self.reset_page_view()?;
                    self.status = format!(
                        "Editable document opened · {} pages",
                        self.editor.snapshot().page_order().len()
                    );
                }
                Ok(FileResult::Imported { page, raster, path }) => {
                    let interpretation = match raster.color {
                        SourceColor::DeclaredSrgb => "declared sRGB",
                        SourceColor::AssumedSrgb => "untagged · assumed sRGB",
                    };
                    let name = path.file_name().map_or_else(
                        || "Imported PNG".into(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    self.editor.import(page, Arc::new(raster.asset), name)?;
                    self.status = format!("Imported original PNG · {interpretation}");
                }
                Ok(FileResult::Saved { receipt, path }) => {
                    let current = self.editor.session.snapshot().revision() == receipt.revision;
                    self.editor.saved(receipt.revision, path);
                    self.status =
                        durability_message("Editable document saved", &receipt.durability);
                    match save_disposition(self.after_save.is_some(), current, &receipt.durability) {
                            SaveDisposition::Proceed => self.finish_after_save(),
                            SaveDisposition::NeedsAttention(message) => {
                                self.uncertain_save = Some(message);
                                self.ui.cancel();
                                self.ui.focus("durability-keep");
                            },
                            SaveDisposition::NewerEdits => self.error = Some("The saved revision predates your latest changes. Save again or cancel closing to keep editing.".into()),
                            SaveDisposition::ContinueEditing => {},
                        }
                    crate::report_durability(receipt.durability);
                }
                Ok(FileResult::Exported { durability, path }) => {
                    self.status = durability_message(
                        &format!("Exact PNG exported · {}", path.display()),
                        &durability,
                    );
                }
            }
            self.paint()?;
        }
        if let Some(clipboard) = &self.clipboard
            && let Some(result) = clipboard.take()?
        {
            match result {
                Ok(ClipboardResult::Read {
                    focus,
                    revision,
                    node,
                    text,
                }) if self.ui.focus_key() == Some(focus.as_str())
                    && self.editor.selected == Some(node)
                    && self.editor.session.snapshot().revision() == revision
                    && !self.blocked() =>
                {
                    let mut input = Input::from(self.pointer);
                    input.clipboard = Some(text);
                    input.keys.push(KeyPress {
                        key: Key::Char('v'),
                        mods: Mods {
                            ctrl: true,
                            ..Default::default()
                        },
                    });
                    self.tick(input)?;
                }
                Ok(_) => {}
                Err(error) => self.fail(error),
            }
        }
        if let Some(error) = self.host.as_ref().and_then(Host::take_error) {
            self.fail(error);
        }
        self.redraw();
        Ok(())
    }
    fn receive_canvas(&mut self) -> AppResult<()> {
        if let Some(error) = self.upload.take_error()? {
            self.fail(error);
        }
        if let Some(completed) = self.upload.take()?
            && completed.key.matches(self.desired)
        {
            match completed.result {
                Ok(canvas) => {
                    if let Some(host) = &mut self.host {
                        host.offer(canvas, self.desired)?;
                    }
                }
                Err(error) => self.fail(format!("The canvas could not upload: {error}")),
            }
        }
        let (Some(host), Some(render), Some(key)) =
            (&self.host, &self.render, upload_key(self.desired))
        else {
            return Ok(());
        };
        // Leave the only completed CPU owner in the render mailbox while the
        // uploader or candidate occupies its slot. Never add a UI pixel owner.
        if host.has_candidate() {
            return Ok(());
        }
        let admission = self.upload.admission(key.request)?;
        if admission == Admission::Pending {
            return Ok(());
        }
        let context = host.context();
        let Some(completed) = render.take()? else {
            return Ok(());
        };
        if let Some(profile) = &mut self.profile {
            profile.completed(completed.serial, completed.timing);
        }
        let metadata_matches =
            completed.generation == key.generation && completed.serial == key.serial;
        match completed.result {
            Ok(frame)
                if render_matches(self.desired, completed.generation, completed.serial, &frame) =>
            {
                if admission == Admission::Refused {
                    self.fail("The exact viewport cannot fit beside the retained canvas in the 320 MiB graphics limit. Reduce the window size; the previous valid image and source resolution are preserved.");
                    return Ok(());
                }
                let page = self
                    .editor
                    .snapshot()
                    .page(key.request.page())
                    .ok_or("The rendered page is missing.")?
                    .size();
                let artboard = artboard_in_request(
                    key.request,
                    Rect::new(0., 0., page.width(), page.height()),
                );
                self.upload.submit(frame, key, artboard, context)?;
            }
            Err(error) if metadata_matches => {
                if self
                    .evaluation
                    .is_some_and(|evaluation| evaluation.margin > 0)
                    && matches!(
                        error.downcast_ref::<affint_render::Error>(),
                        Some(affint_render::Error::BudgetExceeded { .. })
                    )
                {
                    let visible = self
                        .evaluation
                        .ok_or("The refused coverage metadata is missing.")?
                        .camera
                        .viewport();
                    self.overscan_refused = Some((key.revision, [visible.width, visible.height]));
                    self.desired = None;
                    self.evaluation = None;
                    self.upload.desire(None)?;
                    if let Some(host) = &mut self.host {
                        host.keep_desired(None);
                    }
                    self.schedule_render()?;
                } else {
                    self.fail(format!("The viewport could not render: {error}. The previous valid image is retained; reduce the window size or undo the last edit."));
                }
            }
            Ok(_) | Err(_) => {}
        }
        Ok(())
    }
    fn start_gpu(&mut self, event_loop: &ActiveEventLoop) -> AppResult<()> {
        if self.startup.is_some() {
            return Ok(());
        }
        if self.startup_attempts >= 3 {
            return Err("Graphics recovery failed three times. Save your document using Ctrl+S, then reopen AFFINT.".into());
        }
        self.startup_attempts += 1;
        let window = Arc::clone(
            self.window
                .as_ref()
                .ok_or("The native window is unavailable.")?,
        );
        let display = event_loop.owned_display_handle();
        let proxy = self.proxy.clone();
        let generation = self.generation;
        let retirement = self.upload.retirement();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("affint-gpu-startup".into())
            .spawn(move || {
                let result = crate::jobs::guard("Native graphics startup", || {
                    Host::open(window, display, proxy.clone(), generation, retirement)
                });
                if sender.send(result).is_ok() {
                    let _ = proxy.send_event(Wake);
                }
            })?;
        self.startup = Some(receiver);
        Ok(())
    }
    fn draw(&mut self, event_loop: &ActiveEventLoop) -> AppResult<()> {
        if !self.process_motion()? {
            self.paint()?;
        }
        if self
            .host
            .as_ref()
            .is_some_and(|host| host.lost.load(Ordering::Acquire))
        {
            self.host = None;
            self.generation = self
                .generation
                .checked_add(1)
                .ok_or("Graphics generation exhausted.")?;
            self.desired = None;
            self.evaluation = None;
            self.chrome_plan = None;
            self.upload.desire(None)?;
            self.start_gpu(event_loop)?;
            return Ok(());
        }
        let dpi = self
            .window
            .as_ref()
            .map_or(1., |window| window.scale_factor());
        self.receive_canvas()?;
        let plan = if let Some(host) = &self.host {
            host.plan(self.intent)?
        } else {
            None
        };
        if self.chrome_plan != plan {
            self.paint_presentation(plan)?;
        }
        if let (Some(host), Some(scene)) = (&mut self.host, self.ui.scene()) {
            let result = host.present(scene, plan, self.desired, dpi);
            self.chrome_plan = host.receipt();
            match result? {
                Present::Presented => {}
                Present::Retry => {
                    self.repaint_at = Some(Instant::now() + Duration::from_millis(50))
                }
                Present::Hidden => {}
            }
            if let (Some(profile), Some(render)) = (&mut self.profile, &self.render) {
                profile.present(
                    render.stats()?,
                    host.present_stats(),
                    upload_key(self.desired),
                );
            }
        }
        self.receive_canvas()?;
        Ok(())
    }
    fn paint_presentation(&mut self, plan: Option<DisplayPlan>) -> AppResult<()> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let physical = window.inner_size();
        let dpi = window.scale_factor();
        let size = Size::new(
            f64::from(physical.width) / dpi,
            f64::from(physical.height) / dpi,
        );
        let measured = self.profile.as_ref().map(|_| Instant::now());
        self.chrome_plan = plan;
        self.chrome_input = false;
        let tree_started = measured.map(|_| Instant::now());
        let result = (|| -> AppResult<()> {
            let root = self.tree(size)?;
            let tree = tree_started.map_or(Duration::ZERO, |started| started.elapsed());
            let frame_started = measured.map(|_| Instant::now());
            self.ui
                .frame(root, Some(size), Input::from(self.pointer), 0.)?;
            if let (Some(profile), Some(started)) = (&mut self.profile, measured) {
                profile.ui(
                    false,
                    tree,
                    frame_started.map_or(Duration::ZERO, |started| started.elapsed()),
                    started.elapsed(),
                    1,
                    upload_key(self.desired),
                );
            }
            Ok(())
        })();
        self.chrome_input = true;
        result
    }
    fn keyboard(&mut self, event: &winit::event::KeyEvent) -> AppResult<()> {
        if event.state != ElementState::Pressed {
            return Ok(());
        }
        self.process_motion()?;
        let mods = self.pointer.mods;
        if matches!(event.logical_key, NativeKey::Named(NamedKey::Escape)) {
            self.cancel()?;
            self.menu = None;
            if !self.dialog_open {
                self.after_save = None;
                self.uncertain_save = None;
            }
            return self.tick(Input::from(self.pointer));
        }
        if self.blocked() {
            let mut input = Input::from(self.pointer);
            if let NativeKey::Named(named) = event.logical_key
                && let Some(key) = named_key(named)
            {
                input.keys.push(KeyPress { key, mods });
            }
            return self.tick(input);
        }
        let editing = self.ui.focus_key() == Some("layer-name");
        if !editing && (mods.ctrl || mods.cmd) {
            let command = match event.logical_key {
                NativeKey::Named(NamedKey::PageUp) => Some(Command::PreviousPage),
                NativeKey::Named(NamedKey::PageDown) => Some(Command::NextPage),
                _ => None,
            };
            if let Some(command) = command {
                self.command(command)?;
                return self.tick(Input::from(self.pointer));
            }
        }
        if let NativeKey::Character(value) = &event.logical_key {
            let key = value.chars().next().map(|value| value.to_ascii_lowercase());
            if (mods.ctrl || mods.cmd) && key == Some('v') && editing {
                let node = self
                    .editor
                    .selected
                    .ok_or("Select a layer before pasting its name.")?;
                self.clipboard
                    .as_ref()
                    .ok_or("The system clipboard is unavailable.")?
                    .submit(ClipboardJob::Read {
                        focus: "layer-name".into(),
                        revision: self.editor.session.snapshot().revision(),
                        node,
                    })?;
                return Ok(());
            }
            let command = if mods.ctrl || mods.cmd {
                match key {
                    Some('n') => Some(Command::New),
                    Some('o') => Some(Command::Open),
                    Some('q') => Some(Command::Quit),
                    Some('s') => Some(if mods.shift {
                        Command::SaveAs
                    } else {
                        Command::Save
                    }),
                    Some('z') if !editing => Some(if mods.shift {
                        Command::Redo
                    } else {
                        Command::Undo
                    }),
                    Some('y') if !editing => Some(Command::Redo),
                    Some('0') => Some(Command::Fit),
                    Some('1') => Some(Command::Actual),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(command) = command {
                self.command(command)?;
                return self.tick(Input::from(self.pointer));
            }
            if !editing && !mods.ctrl && !mods.cmd && !mods.alt {
                let tool = match key {
                    Some('v') => Some(Tool::Move),
                    Some('r') => Some(Tool::Rectangle),
                    Some('e') => Some(Tool::Ellipse),
                    Some('c') => Some(Tool::Crop),
                    Some('h') => Some(Tool::Pan),
                    _ => None,
                };
                if let Some(tool) = tool {
                    self.editor.cancel()?;
                    self.editor.tool = tool;
                }
            }
        }
        if !editing && matches!(event.logical_key, NativeKey::Named(NamedKey::Delete)) {
            self.command(Command::Delete)?;
            return self.tick(Input::from(self.pointer));
        }
        let mut input = Input::from(self.pointer);
        match &event.logical_key {
            NativeKey::Named(named) => {
                if let Some(key) = named_key(*named) {
                    input.keys.push(KeyPress { key, mods });
                }
            }
            NativeKey::Character(value) if mods.ctrl || mods.cmd => {
                input.keys.extend(value.chars().map(|key| KeyPress {
                    key: Key::Char(key),
                    mods,
                }))
            }
            _ => {}
        }
        if !mods.ctrl
            && !mods.cmd
            && let Some(text) = &event.text
        {
            input
                .text
                .extend(text.chars().filter(|ch| !ch.is_control()));
        }
        self.tick(input)
    }
}

fn short_label(value: &str, maximum: usize) -> String {
    let mut characters = value.chars();
    let mut label: String = characters.by_ref().take(maximum).collect();
    if characters.next().is_some() {
        label.push('…');
    }
    label
}

// Compare in document space: a changed viewport or zoom can move the document
// point even when the native pointer has not moved. No epsilon hides real input.
fn changed_canvas_point(last: &mut Option<DocumentPoint>, point: DocumentPoint) -> bool {
    if *last == Some(point) {
        return false;
    }
    *last = Some(point);
    true
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("AFFINT")
            .with_inner_size(LogicalSize::new(1440., 940.))
            .with_min_inner_size(LogicalSize::new(500., 640.))
            .with_visible(false);
        let result = (|| -> AppResult<()> {
            let window = Arc::new(event_loop.create_window(attributes)?);
            self.files = Some(FileWorker::new(
                Store::new(FileLimits::default(), document_limits())?,
                self.renderer.clone(),
                Arc::clone(&window),
                self.proxy.clone(),
            )?);
            self.clipboard = Some(ClipboardWorker::new(self.proxy.clone())?);
            self.render = Some(RenderWorker::new(
                self.renderer.clone(),
                self.proxy.clone(),
            )?);
            self.window = Some(window);
            self.paint()?;
            self.start_gpu(event_loop)
        })();
        if let Err(error) = result {
            eprintln!("AFFINT startup failed: {error}");
            self.fail(error);
            event_loop.exit();
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: Wake) {
        if let Err(error) = self.receive() {
            self.fail(error);
        }
        if self.close_ready {
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let result = (|| -> AppResult<()> {
            match event {
                WindowEvent::CloseRequested => self.request_replace(AfterSave::Close)?,
                WindowEvent::Resized(size) => {
                    self.visible = size.width > 0 && size.height > 0;
                    if let Some(host) = &mut self.host {
                        host.resize(size.width, size.height)?;
                    }
                    self.paint()?;
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    self.motion = None;
                    if let Some(window) = &self.window {
                        let size = window.inner_size();
                        if let Some(host) = &mut self.host {
                            host.resize(size.width, size.height)?;
                        }
                    }
                    self.paint()?;
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let dpi = self
                        .window
                        .as_ref()
                        .map_or(1., |window| window.scale_factor());
                    let mut pointer = self.pointer;
                    pointer.pos = Some(Point::new(position.x / dpi, position.y / dpi));
                    self.motion = Some(pointer);
                }
                WindowEvent::CursorLeft { .. } => {
                    self.process_motion()?;
                    if self.pointer.buttons.is_empty() {
                        self.pointer.pos = None;
                        self.tick(Input::from(self.pointer))?;
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    self.process_motion()?;
                    let button = match button {
                        MouseButton::Left => Button::Primary,
                        MouseButton::Right => Button::Secondary,
                        MouseButton::Middle => Button::Middle,
                        _ => return Ok(()),
                    };
                    self.pointer.buttons = self
                        .pointer
                        .buttons
                        .set(button, state == ElementState::Pressed);
                    self.tick(Input::from(self.pointer))?;
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    self.process_motion()?;
                    let dpi = self
                        .window
                        .as_ref()
                        .map_or(1., |window| window.scale_factor());
                    let wheel = logical_wheel(delta, dpi);
                    let in_canvas =
                        self.viewport
                            .zip(self.pointer.pos)
                            .is_some_and(|(frame, point)| {
                                point.x >= frame.x
                                    && point.y >= frame.y
                                    && point.x < frame.x + frame.size.width
                                    && point.y < frame.y + frame.size.height
                            });
                    if in_canvas && !self.blocked() && self.menu.is_none() {
                        if let (Some(frame), Some(point)) = (self.viewport, self.pointer.pos) {
                            if self.pointer.mods.ctrl || self.pointer.mods.cmd {
                                let local = Point::new(point.x - frame.x, point.y - frame.y);
                                let fixed = self
                                    .host
                                    .as_ref()
                                    .and_then(Host::receipt)
                                    .and_then(|plan| {
                                        plan.document_point(DocumentPoint::new(point.x, point.y))
                                    })
                                    .unwrap_or_else(|| self.view.document(local, frame.size));
                                self.view.zoom_about(
                                    wheel_zoom_factor(delta, dpi),
                                    fixed,
                                    local,
                                    frame.size,
                                );
                                self.navigation.changed(true);
                            } else {
                                self.view.center.x += wheel.x / self.view.zoom;
                                self.view.center.y += wheel.y / self.view.zoom;
                                self.view.fitted = true;
                                self.navigation.changed(false);
                            }
                        }
                        self.tick(Input::from(self.pointer))?;
                    } else {
                        let mut input = Input::from(self.pointer);
                        input.wheel = wheel;
                        self.tick(input)?;
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => self.keyboard(&event)?,
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.process_motion()?;
                    let state = modifiers.state();
                    self.pointer.mods = Mods {
                        ctrl: state.control_key(),
                        shift: state.shift_key(),
                        alt: state.alt_key(),
                        cmd: state.super_key(),
                    };
                }
                WindowEvent::Ime(ime) => {
                    self.process_motion()?;
                    let mut input = Input::from(self.pointer);
                    match &ime {
                        NativeIme::Preedit(text, _) | NativeIme::Commit(text)
                            if text.len() > 4096 =>
                        {
                            return Err(
                                "Input-method text exceeds the 4096-byte layer-name limit.".into(),
                            );
                        }
                        _ => {}
                    }
                    input.ime.push(match ime {
                        NativeIme::Enabled => Ime::Enabled,
                        NativeIme::Disabled => Ime::Disabled,
                        NativeIme::Preedit(text, cursor) => Ime::Preedit { text, cursor },
                        NativeIme::Commit(text) => Ime::Commit(text),
                    });
                    self.tick(input)?;
                }
                WindowEvent::Focused(false) => {
                    self.cancel()?;
                    self.paint()?;
                }
                WindowEvent::Occluded(hidden) => {
                    self.visible = !hidden;
                    if hidden {
                        self.cancel()?;
                        self.repaint_at = None;
                    }
                }
                WindowEvent::RedrawRequested => {
                    if self.visible {
                        self.draw(event_loop)?;
                    }
                    return Ok(());
                }
                _ => {}
            }
            self.redraw();
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(error);
        }
        if self.close_ready {
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.close_ready {
            event_loop.exit();
            return;
        }
        if self.visible
            && let Some(deadline) = match (self.repaint_at, self.navigation.deadline()) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        {
            if deadline <= Instant::now() {
                self.repaint_at = None;
                self.redraw();
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
                return;
            }
        }
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}

pub fn run() -> AppResult<()> {
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut app = App::new(event_loop.create_proxy())?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
pub fn run_profile(directory: &std::path::Path) -> AppResult<()> {
    if !crate::jobs::diagnostics_enabled() {
        return Err("Launch --interaction-profile with AFFINT_DIAGNOSTICS=1 to enable native timing observations.".into());
    }
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut app = App::new(event_loop.create_proxy())?;
    let result = event_loop.run_app(&mut app);
    if let Some(profile) = &app.profile {
        profile.write(directory)?;
    }
    result?;
    Ok(())
}
fn activated(ui: &Ui, id: &str) -> bool {
    ui.get(id).clicked_with(Button::Primary)
        || ui
            .keys(id)
            .iter()
            .any(|press| matches!(press.key, Key::Enter | Key::Space))
}
fn save_disposition(pending: bool, current: bool, durability: &Durability) -> SaveDisposition {
    if !pending {
        return SaveDisposition::ContinueEditing;
    }
    if !current {
        return SaveDisposition::NewerEdits;
    }
    if let Durability::PublishedUnconfirmed { directory, source } = durability {
        return SaveDisposition::NeedsAttention(format!(
            "The document was saved, but its directory could not be synchronized ({}): {source}.",
            directory.display()
        ));
    }
    SaveDisposition::Proceed
}
fn render_matches(
    desired: Option<(Revision, RenderRequest, u64, u64)>,
    generation: u64,
    serial: u64,
    frame: &DocumentFrame,
) -> bool {
    desired.is_some_and(|(revision, request, current_generation, current_serial)| {
        revision == frame.revision()
            && request == frame.request()
            && current_generation == generation
            && current_serial == serial
    })
}
fn upload_key(desired: Option<(Revision, RenderRequest, u64, u64)>) -> Option<UploadKey> {
    desired.map(|(revision, request, generation, serial)| UploadKey {
        revision,
        request,
        generation,
        serial,
    })
}
fn durability_message(prefix: &str, durability: &Durability) -> String {
    match durability {
        Durability::Synced => prefix.into(),
        Durability::PublishedUnconfirmed { source, .. } => {
            format!("{prefix}; directory durability unconfirmed: {source}")
        }
        Durability::PlatformUnverified => {
            format!("{prefix}; directory durability is not verified on this platform")
        }
    }
}
pub fn logical_wheel(delta: MouseScrollDelta, dpi: f64) -> Point {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => Point::new(-f64::from(x) * 24., -f64::from(y) * 24.),
        MouseScrollDelta::PixelDelta(point) => Point::new(-point.x / dpi, -point.y / dpi),
    }
}
fn wheel_zoom_factor(delta: MouseScrollDelta, dpi: f64) -> f64 {
    // AFFINT precise-scroll calibration:24 logical pixels equal one notch.
    // The signed cap/gains and f32 arithmetic follow the independently read spec.
    let notches = match delta {
        MouseScrollDelta::LineDelta(_, y) => f64::from(y),
        MouseScrollDelta::PixelDelta(point) => point.y / dpi / 24.,
    }
    .clamp(-3., 3.) as f32;
    let gain = if notches > 0. { 0.25 } else { 0.2 };
    f64::from(1_f32 + (f64::from(notches) * gain) as f32)
}
fn canvas_entry_point(
    plan: Option<DisplayPlan>,
    revision: Revision,
    pointer: Point,
) -> Option<DocumentPoint> {
    plan.filter(|plan| plan.evaluation.key.revision == revision)?
        .document_point(DocumentPoint::new(pointer.x, pointer.y))
}
fn snapped_viewport(
    frame: mui::layout::Frame,
    dpi: f64,
    width: u32,
    height: u32,
) -> Option<Viewport> {
    let x = (frame.x * dpi).round().max(0.).min(f64::from(width)) as u32;
    let y = (frame.y * dpi).round().max(0.).min(f64::from(height)) as u32;
    let x1 = ((frame.x + frame.size.width) * dpi)
        .round()
        .max(0.)
        .min(f64::from(width)) as u32;
    let y1 = ((frame.y + frame.size.height) * dpi)
        .round()
        .max(0.)
        .min(f64::from(height)) as u32;
    (x1 > x && y1 > y).then_some(Viewport {
        x,
        y,
        width: x1.saturating_sub(x),
        height: y1.saturating_sub(y),
    })
}
fn named_key(key: NamedKey) -> Option<Key> {
    Some(match key {
        NamedKey::Enter => Key::Enter,
        NamedKey::Escape => Key::Escape,
        NamedKey::Tab => Key::Tab,
        NamedKey::Space => Key::Space,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stationary_held_canvas_response_does_not_issue_another_edit() -> AppResult<()> {
        let mut ui = Ui::new(Theme::DEFAULT);
        let root = || leaf(960., 720.).id("viewport");
        ui.frame(root(), None, Input::default(), 0.)?;
        let mut pointer = PointerInput {
            pos: Some(Point::new(660., 260.)),
            buttons: Buttons::default().set(Button::Primary, true),
            ..Default::default()
        };
        ui.frame(root(), None, Input::from(pointer), 0.)?;
        let mut editor = Editor::demo()?;
        editor.press(DocumentPoint::new(660., 260.))?;
        let mut last = Some(DocumentPoint::new(660., 260.));
        pointer.pos = Some(Point::new(680., 270.));
        ui.frame(root(), None, Input::from(pointer), 0.)?;
        assert!(ui.get("viewport").dragged);
        let moved = DocumentPoint::new(680., 270.);
        assert!(changed_canvas_point(&mut last, moved));
        editor.drag(moved)?;
        let revision = editor.snapshot().revision();
        // MUI intentionally retains drag capture across stationary paint frames.
        ui.frame(root(), None, Input::from(pointer), 0.)?;
        assert!(ui.get("viewport").dragged);
        assert_eq!(ui.get("viewport").drag_delta, Point::ZERO);
        assert!(!changed_canvas_point(&mut last, moved));
        assert_eq!(editor.snapshot().revision(), revision);
        let moved_again = DocumentPoint::new(700., 290.);
        assert!(changed_canvas_point(&mut last, moved_again));
        editor.drag(moved_again)?;
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, 1);
        Ok(())
    }
    fn name_input_cycle(ui: &mut Ui, value: &mut String, input: Input) -> AppResult<UiEffects> {
        let root = text_input(ui, "name", value).0;
        let first =
            UiEffects::from_frame(ui.frame(root, Some(Size::new(280., 40.)), input, 0.1)?);
        let root = text_input(ui, "name", value).0;
        let replay = UiEffects::from_frame(ui.frame(
            root,
            Some(Size::new(280., 40.)),
            Input::default(),
            0.,
        )?);
        Ok(first.then(replay))
    }
    #[test]
    fn pixel_wheel_divides_dpi_but_line_wheel_is_already_logical() {
        assert_eq!(
            logical_wheel(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(20., 40.)),
                2.
            ),
            Point::new(-10., -20.)
        );
        assert_eq!(
            logical_wheel(MouseScrollDelta::LineDelta(1., 1.), 2.),
            Point::new(-24., -24.)
        );
    }
    #[test]
    fn viewport_snaps_edges_once_at_fractional_dpi() {
        let frame = mui::layout::Frame {
            x: 46.,
            y: 130.,
            size: Size::new(700., 500.),
        };
        assert_eq!(
            snapped_viewport(frame, 1.5, 1600, 1200),
            Some(Viewport {
                x: 69,
                y: 195,
                width: 1050,
                height: 750
            })
        );
    }
    #[test]
    fn zoom_keeps_pointer_document_position_fixed() {
        let mut view = ViewState::new();
        let size = Size::new(900., 600.);
        let local = Point::new(370., 210.);
        let before = view.document(local, size);
        view.zoom_about(1.7, before, local, size);
        assert!((view.document(local, size) - before).hypot() < 1e-9);
    }
    #[test]
    fn wheel_zoom_uses_signed_linear_event_gains_cap_and_physical_pixel_adapter() {
        for (notches, expected) in [
            (1., 1.25),
            (2., 1.5),
            (3., 1.75),
            (20., 1.75),
            (-1., f64::from(0.8f32)),
            (-2., f64::from(0.6f32)),
            (-3., f64::from(f32::from_bits(0x3ecc_cccc))),
            (-20., f64::from(f32::from_bits(0x3ecc_cccc))),
        ] {
            assert_eq!(
                wheel_zoom_factor(MouseScrollDelta::LineDelta(0., notches), 1.5),
                expected
            );
            assert_eq!(
                wheel_zoom_factor(
                    MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                        0.,
                        f64::from(notches) * 24. * 1.5
                    )),
                    1.5
                ),
                expected
            );
        }
        assert_eq!(
            wheel_zoom_factor(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0., 18.)),
                1.5
            ),
            1.125
        );
    }
    #[test]
    fn navigation_defers_zoom_and_busy_work_but_final_intent_progresses_without_input() {
        let origin = Instant::now();
        let mut navigation = Navigation::default();
        navigation.submitted(origin);
        navigation.changed_at(origin + Duration::from_millis(1), true);
        assert!(!navigation.ready(origin + Duration::from_millis(100), false, Some(0.)));
        assert_eq!(
            navigation.deadline(),
            Some(origin + Duration::from_millis(101))
        );
        assert!(!navigation.ready(origin + Duration::from_millis(101), true, Some(0.)));
        assert_eq!(
            navigation.deadline(),
            Some(origin + Duration::from_millis(117))
        );
        assert!(navigation.ready(origin + Duration::from_millis(117), false, Some(64.)));
        navigation.submitted(origin + Duration::from_millis(117));
        assert!(navigation.deadline().is_none());
        assert!(!navigation.zooming);
        navigation.changed_at(origin + Duration::from_millis(120), false);
        assert!(!navigation.ready(origin + Duration::from_millis(121), false, Some(63.)));
        assert!(navigation.ready(origin + Duration::from_millis(122), false, Some(32.)));
        assert!(!navigation.ready(origin + Duration::from_millis(122), true, Some(0.)));
        assert!(navigation.ready(origin + Duration::from_millis(220), false, Some(64.)));
        let mut reverted = Navigation::default();
        assert!(!reverted.ready(origin, true, None));
        assert!(reverted.deadline().is_some());
        reverted.settled(origin + Duration::from_millis(1));
        assert!(
            reverted.deadline().is_none(),
            "Returning to the submitted camera must not retain an expired redraw retry loop."
        );
    }
    #[test]
    fn actual_ui_pick_and_gesture_follow_displayed_camera_while_zoom_is_pending() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let viewport = Viewport {
            x: 0,
            y: 0,
            width: 1440,
            height: 1080,
        };
        let initial_request = RenderRequest::new(
            editor.page()?,
            1440,
            1080,
            View::Affine(Transform::new([1.5, 0., 0., 1.5, 0., 0.])?),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let initial =
            CameraIntent::new(editor.snapshot().revision(), initial_request, viewport, 1.5)?;
        let evaluation = Evaluation::new(
            UploadKey {
                revision: initial.revision(),
                request: padded_request(initial_request, OVERSCAN)?,
                generation: 1,
                serial: 5,
            },
            initial,
            OVERSCAN,
        )?;
        let wanted = CameraIntent::new(
            initial.revision(),
            RenderRequest::new(
                editor.page()?,
                1440,
                1080,
                View::Affine(Transform::new([3., 0., 0., 3., -720., -540.])?),
            )?
            .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?,
            viewport,
            1.5,
        )?;
        let plan = DisplayPlan::choose(evaluation, Rect::new(64., 64., 1504., 1144.), wanted)?;
        assert!(plan.pending);
        let mut ui = Ui::new(Theme::DEFAULT);
        let root = || leaf(960., 720.).id("viewport");
        ui.frame(root(), None, Input::default(), 0.)?;
        let mut pointer = PointerInput {
            pos: Some(Point::new(660., 260.)),
            buttons: Buttons::default().set(Button::Primary, true),
            ..Default::default()
        };
        ui.frame(root(), None, Input::from(pointer), 0.)?;
        assert!(ui.get("viewport").pressed);
        let original_revision = editor.session.snapshot().revision();
        let point = canvas_entry_point(
            Some(plan),
            original_revision,
            pointer.pos.ok_or("Fixture pointer is missing.")?,
        )
        .ok_or("Displayed camera refused a current document pick.")?;
        assert_eq!(point, DocumentPoint::new(660., 260.));
        editor.press(point)?;
        let selected = editor
            .selected_node()?
            .ok_or("Displayed pick missed the original artwork.")?;
        let original = editor
            .session
            .snapshot()
            .node(selected)
            .ok_or("Fixture layer is missing.")?
            .transform();
        pointer.pos = Some(Point::new(680., 270.));
        ui.frame(root(), None, Input::from(pointer), 0.)?;
        assert!(ui.get("viewport").dragged);
        let moved = plan
            .document_point(DocumentPoint::new(680., 270.))
            .ok_or("Displayed drag is outside coverage.")?;
        editor.drag(moved)?;
        let preview_revision = editor.snapshot().revision();
        assert_ne!(preview_revision, original_revision);
        // An ongoing gesture uses the displayed camera even after its preview
        // changes revision; entry into a new gesture requires matching content.
        assert!(canvas_entry_point(Some(plan), preview_revision, Point::new(680., 270.)).is_none());
        assert_eq!(
            plan.document_point(DocumentPoint::new(700., 290.)),
            Some(DocumentPoint::new(700., 290.))
        );
        editor.drag(DocumentPoint::new(700., 290.))?;
        pointer.buttons = Buttons::default();
        ui.frame(root(), None, Input::from(pointer), 0.)?;
        assert!(ui.get("viewport").released);
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, 1);
        assert!(
            canvas_entry_point(
                Some(plan),
                editor.session.snapshot().revision(),
                Point::new(660., 260.)
            )
            .is_none()
        );
        editor.history(History::Undo)?;
        assert_eq!(
            editor
                .session
                .snapshot()
                .node(selected)
                .ok_or("Undo layer is missing.")?
                .transform(),
            original
        );
        let mut view = ViewState::new();
        view.zoom = 2.;
        let local = Point::new(660., 260.);
        let size = Size::new(960., 720.);
        let anchor = plan
            .document_point(DocumentPoint::new(local.x, local.y))
            .ok_or("Zoom anchor is outside the displayed camera.")?;
        view.zoom_about(
            wheel_zoom_factor(MouseScrollDelta::LineDelta(0., 1.), 1.5),
            anchor,
            local,
            size,
        );
        assert!((view.document(local, size) - anchor).hypot() < 1e-10);
        Ok(())
    }
    #[test]
    fn handoff_requires_revision_view_format_device_and_serial() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let snapshot = editor.session.snapshot();
        let request = RenderRequest::new(
            editor.page()?,
            8,
            8,
            View::Region(Rect::new(0., 0., 960., 720.)),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let changed_view = RenderRequest::new(
            editor.page()?,
            8,
            8,
            View::Region(Rect::new(10., 10., 970., 730.)),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let renderer = Renderer::new(Default::default())?;
        let frame = renderer.render(&snapshot, request)?;
        editor.exposure(0.25)?;
        let decisions = [
            render_matches(Some((snapshot.revision(), request, 1, 7)), 1, 7, &frame),
            render_matches(
                Some((editor.session.snapshot().revision(), request, 1, 7)),
                1,
                7,
                &frame,
            ),
            render_matches(
                Some((snapshot.revision(), changed_view, 1, 7)),
                1,
                7,
                &frame,
            ),
            render_matches(Some((snapshot.revision(), request, 2, 7)), 1, 7, &frame),
            render_matches(Some((snapshot.revision(), request, 1, 8)), 1, 7, &frame),
            render_matches(
                Some((
                    snapshot.revision(),
                    request.with_format(OutputFormat::SrgbRgba8Straight)?,
                    1,
                    7,
                )),
                1,
                7,
                &frame,
            ),
        ];
        assert_eq!(decisions, [true, false, false, false, false, false]);
        Ok(())
    }
    #[test]
    fn handoff_rejects_another_page_at_the_same_revision_and_view() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let first = editor.page()?;
        let second = editor.add_page()?;
        let snapshot = editor.session.snapshot();
        let region = View::Region(Rect::new(0., 0., 960., 720.));
        let first_request = RenderRequest::new(first, 8, 8, region)?;
        let second_request = RenderRequest::new(second, 8, 8, region)?;
        let renderer = Renderer::new(Default::default())?;
        let frame = renderer.render(&snapshot, first_request)?;
        assert!(render_matches(
            Some((snapshot.revision(), first_request, 1, 7)),
            1,
            7,
            &frame
        ));
        assert!(!render_matches(
            Some((snapshot.revision(), second_request, 1, 7)),
            1,
            7,
            &frame
        ));
        Ok(())
    }
    #[test]
    fn custom_control_accepts_enter_and_paints_its_focus_ring() -> AppResult<()> {
        let mut ui = Ui::new(Theme::DEFAULT);
        let root = || {
            leaf(30., 30.)
                .id("custom")
                .role(Kind::Button)
                .focusable()
                .on(State::Focus, |style| style.border(color(ACCENT), 1.5))
        };
        ui.frame(root(), None, Input::default(), 0.)?;
        ui.focus("custom");
        let input = Input {
            keys: vec![KeyPress {
                key: Key::Enter,
                mods: Mods::default(),
            }],
            ..Default::default()
        };
        ui.frame(root(), None, input, 1.)?;
        assert!(activated(&ui, "custom"));
        assert!(
            ui.scene().is_some_and(|scene| scene
                .paint
                .iter()
                .any(|paint| paint.key.as_ref() == "custom"
                    && paint.layer == mui::scene::Layer::Stroke))
        );
        Ok(())
    }
    #[test]
    fn save_and_close_keeps_real_sync_failure_visible() {
        let durability = Durability::PublishedUnconfirmed {
            directory: std::path::PathBuf::from("/artwork"),
            source: std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "directory sync refused",
            ),
        };
        assert!(
            matches!(save_disposition(true, true, &durability), SaveDisposition::NeedsAttention(message) if message.contains("directory sync refused"))
        );
        assert_eq!(
            save_disposition(true, true, &Durability::Synced),
            SaveDisposition::Proceed
        );
    }
    #[test]
    fn older_saved_revision_cannot_finish_pending_close() {
        assert_eq!(
            save_disposition(true, false, &Durability::Synced),
            SaveDisposition::NewerEdits
        );
    }
    #[test]
    fn native_copy_and_cut_handoff_contains_the_actual_selected_text() -> AppResult<()> {
        let mut ui = Ui::new(Theme::DEFAULT).font(Font::new(epaint_default_fonts::UBUNTU_LIGHT)?);
        let mut name = "Coral disc".to_owned();
        name_input_cycle(&mut ui, &mut name, Input::default())?;
        ui.focus("name");
        let key = |character| Input {
            keys: vec![KeyPress {
                key: Key::Char(character),
                mods: Mods {
                    ctrl: true,
                    ..Default::default()
                },
            }],
            ..Input::default()
        };
        name_input_cycle(&mut ui, &mut name, key('a'))?;
        let copied = name_input_cycle(&mut ui, &mut name, key('c'))?;
        assert_eq!(copied.clipboard.as_deref(), Some("Coral disc"));
        let cut = name_input_cycle(&mut ui, &mut name, key('x'))?;
        assert_eq!(cut.clipboard.as_deref(), Some("Coral disc"));
        assert!(name.is_empty());
        assert!(cut.ime.is_some());
        Ok(())
    }
}
