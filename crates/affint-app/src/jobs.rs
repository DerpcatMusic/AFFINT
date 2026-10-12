//! Fixed workers with one pending request and one completed result per workload.
use std::path::PathBuf;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};
use std::thread;
use std::time::Instant;

use affint_document::{NodeId, PageId, Snapshot};
use affint_render::{Frame, OutputFormat, RenderRequest, Renderer};
use affint_storage::{Durability, ImportedRaster, PngLimits, SaveReceipt, Store};
use winit::{event_loop::EventLoopProxy, window::Window};

use crate::{Wake, editor::AppResult};

struct RenderJob {
    serial: u64,
    snapshot: Snapshot,
    request: RenderRequest,
    generation: u64,
    queued_at: Option<Instant>,
}
struct RenderState {
    pending: Option<RenderJob>,
    result: Option<RenderResult>,
    stats: RenderStats,
}
impl RenderState {
    fn take_job(&mut self) -> Option<RenderJob> {
        let job = self.pending.take()?;
        self.stats.started = self.stats.started.saturating_add(1);
        self.stats.active_serial = Some(job.serial);
        Some(job)
    }
}
/// Monotonic worker timestamps, present only with AFFINT_DIAGNOSTICS=1.
#[derive(Debug, Clone, Copy)]
pub struct RenderTiming {
    pub queued_at: Instant,
    pub started_at: Instant,
    pub finished_at: Instant,
}
/// Bounded cumulative counts. Counters saturate rather than wrapping.
/// A canceled evaluation and a rejected obsolete completion are distinct events.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderStats {
    pub submitted: u64,
    pub pending_replaced: u64,
    pub pending_dropped_on_stop: u64,
    pub started: u64,
    pub finished: u64,
    pub produced: u64,
    pub canceled: u64,
    pub failed: u64,
    pub published: u64,
    pub completion_rejected: u64,
    pub taken: u64,
    pub result_replaced: u64,
    pub result_dropped_on_stop: u64,
    pub active_serial: Option<u64>,
    pub pending_serial: Option<u64>,
    pub result_pending: bool,
    pub latest_timing: Option<(u64, RenderTiming)>,
}
pub struct RenderResult {
    pub serial: u64,
    pub generation: u64,
    pub timing: Option<RenderTiming>,
    pub result: AppResult<Frame>,
}
struct Shared {
    state: Mutex<RenderState>,
    signal: Condvar,
    serial: AtomicU64,
    stop: AtomicBool,
    timing_enabled: bool,
}

impl Shared {
    fn new(timing_enabled: bool) -> Self {
        Self {
            state: Mutex::new(RenderState {
                pending: None,
                result: None,
                stats: RenderStats::default(),
            }),
            signal: Condvar::new(),
            serial: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            timing_enabled,
        }
    }
    fn publish(&self, completed: RenderResult) -> AppResult<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "The render result channel stopped unexpectedly.")?;
        state.stats.finished = state.stats.finished.saturating_add(1);
        state.stats.active_serial = None;
        match &completed.result {
            Ok(_) => state.stats.produced = state.stats.produced.saturating_add(1),
            Err(error)
                if error.downcast_ref::<affint_render::Error>()
                    == Some(&affint_render::Error::Canceled) =>
            {
                state.stats.canceled = state.stats.canceled.saturating_add(1);
            }
            Err(_) => state.stats.failed = state.stats.failed.saturating_add(1),
        }
        if let Some(timing) = completed.timing {
            state.stats.latest_timing = Some((completed.serial, timing));
        }
        if self.serial.load(Ordering::Acquire) != completed.serial
            || self.stop.load(Ordering::Acquire)
        {
            state.stats.completion_rejected = state.stats.completion_rejected.saturating_add(1);
            return Ok(false);
        }
        if state.result.is_some() {
            state.stats.result_replaced = state.stats.result_replaced.saturating_add(1);
        }
        state.stats.published = state.stats.published.saturating_add(1);
        state.result = Some(completed);
        Ok(true)
    }
}

/// Replacement cancels obsolete evaluation at the renderer's tile checkpoints.
pub struct RenderWorker {
    shared: Arc<Shared>,
}
impl RenderWorker {
    pub fn new(renderer: Renderer, proxy: EventLoopProxy<Wake>) -> AppResult<Self> {
        let shared = Arc::new(Shared::new(diagnostics_enabled()));
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("affint-render".into())
            .spawn(move || {
                loop {
                    let job = {
                        let Ok(mut state) = worker.state.lock() else {
                            let _ = proxy.send_event(Wake);
                            return;
                        };
                        while state.pending.is_none() && !worker.stop.load(Ordering::Acquire) {
                            let Ok(next) = worker.signal.wait(state) else {
                                let _ = proxy.send_event(Wake);
                                return;
                            };
                            state = next;
                        }
                        if worker.stop.load(Ordering::Acquire) {
                            return;
                        }
                        state.take_job()
                    };
                    let Some(RenderJob {
                        serial,
                        snapshot,
                        request,
                        generation,
                        queued_at,
                    }) = job
                    else {
                        continue;
                    };
                    let started_at = worker.timing_enabled.then(Instant::now);
                    let result = guard("The render worker", || {
                        renderer
                            .render_with_cancel(&snapshot, request, || {
                                worker.stop.load(Ordering::Acquire)
                                    || worker.serial.load(Ordering::Acquire) != serial
                            })
                            .map_err(Into::into)
                    });
                    let timing =
                        queued_at
                            .zip(started_at)
                            .map(|(queued_at, started_at)| RenderTiming {
                                queued_at,
                                started_at,
                                finished_at: Instant::now(),
                            });
                    // The lock and final serial check close the handoff race with submit.
                    match worker.publish(RenderResult {
                        serial,
                        generation,
                        timing,
                        result,
                    }) {
                        Ok(true) => {
                            if proxy.send_event(Wake).is_err() {
                                return;
                            }
                        }
                        Ok(false) => {}
                        Err(_) => {
                            let _ = proxy.send_event(Wake);
                            return;
                        }
                    }
                }
            })?;
        Ok(Self { shared })
    }
    pub fn submit(
        &self,
        snapshot: Snapshot,
        request: RenderRequest,
        generation: u64,
    ) -> AppResult<u64> {
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| "The render worker stopped unexpectedly.")?;
        let previous = self
            .shared
            .serial
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |serial| {
                serial.checked_add(1)
            })
            .map_err(|_| "Render request counter exhausted.")?;
        let serial = previous + 1;
        state.stats.submitted = state.stats.submitted.saturating_add(1);
        if state.result.is_some() {
            state.stats.result_replaced = state.stats.result_replaced.saturating_add(1);
        }
        state.result = None;
        if state.pending.is_some() {
            state.stats.pending_replaced = state.stats.pending_replaced.saturating_add(1);
        }
        state.pending = Some(RenderJob {
            serial,
            snapshot,
            request,
            generation,
            queued_at: self.shared.timing_enabled.then(Instant::now),
        });
        self.shared.signal.notify_one();
        Ok(serial)
    }
    pub fn take(&self) -> AppResult<Option<RenderResult>> {
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| "The render worker stopped unexpectedly.")?;
        let result = state.result.take();
        if result.is_some() {
            state.stats.taken = state.stats.taken.saturating_add(1);
        }
        Ok(result)
    }
    /// Metadata only; takes the existing mailbox lock and never clones pixels.
    pub fn stats(&self) -> AppResult<RenderStats> {
        let state = self
            .shared
            .state
            .lock()
            .map_err(|_| "The render worker stopped unexpectedly.")?;
        let mut stats = state.stats;
        stats.pending_serial = state.pending.as_ref().map(|job| job.serial);
        stats.result_pending = state.result.is_some();
        Ok(stats)
    }
}
impl Drop for RenderWorker {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            self.shared.stop.store(true, Ordering::Release);
            if state.pending.is_some() {
                state.stats.pending_dropped_on_stop =
                    state.stats.pending_dropped_on_stop.saturating_add(1);
            }
            if state.result.is_some() {
                state.stats.result_dropped_on_stop =
                    state.stats.result_dropped_on_stop.saturating_add(1);
            }
            state.pending = None;
            state.result = None;
        } else {
            self.shared.stop.store(true, Ordering::Release);
        }
        self.shared.signal.notify_one();
    }
}

/// Read once when workers/graphics are created; normal rendering records no Instants.
pub fn diagnostics_enabled() -> bool {
    matches!(std::env::var("AFFINT_DIAGNOSTICS").as_deref(), Ok("1"))
}

pub enum FileJob {
    Open,
    Import {
        page: PageId,
    },
    Save {
        snapshot: Snapshot,
        path: Option<PathBuf>,
    },
    Export {
        snapshot: Snapshot,
        request: RenderRequest,
    },
}
pub enum FileResult {
    Canceled,
    Open {
        session: Box<affint_document::Session>,
        path: PathBuf,
    },
    Imported {
        page: PageId,
        raster: ImportedRaster,
        path: PathBuf,
    },
    Saved {
        receipt: SaveReceipt,
        path: PathBuf,
    },
    Exported {
        durability: Durability,
        path: PathBuf,
    },
}

/// One job at a time; I/O commands are refused while occupied, never silently dropped.
pub struct FileWorker {
    sender: SyncSender<FileJob>,
    receiver: Receiver<AppResult<FileResult>>,
}
impl FileWorker {
    pub fn new(
        store: Store,
        renderer: Renderer,
        window: Arc<Window>,
        proxy: EventLoopProxy<Wake>,
    ) -> AppResult<Self> {
        let (sender, jobs) = mpsc::sync_channel(1);
        let (results, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("affint-files".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let result = guard("The file worker", || {
                        run_file(job, &store, &renderer, &window)
                    });
                    if results.send(result).is_err() || proxy.send_event(Wake).is_err() {
                        return;
                    }
                }
            })?;
        Ok(Self { sender, receiver })
    }
    pub fn submit(&self, job: FileJob) -> AppResult<()> {
        self.sender.try_send(job).map_err(|error| {
            match error {
                TrySendError::Full(_) => "A file operation is already queued.",
                TrySendError::Disconnected(_) => {
                    "The file worker stopped. Restart AFFINT after saving recovery data."
                }
            }
            .into()
        })
    }
    pub fn take(&self) -> AppResult<Option<AppResult<FileResult>>> {
        match self.receiver.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The file worker stopped unexpectedly.".into())
            }
        }
    }
}

/// An upstream decoder/driver panic must wake the UI with an error, not strand it.
pub fn guard<T>(name: &str, operation: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(payload) => {
            let details = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("an upstream dependency stopped unexpectedly");
            Err(format!("{name} failed: {details}. The current document is preserved.").into())
        }
    }
}
fn dialog(window: &Window, title: &str, extension: &str) -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_parent(window)
        .set_title(title)
        .add_filter(extension, &[extension])
}
fn run_file(
    job: FileJob,
    store: &Store,
    renderer: &Renderer,
    window: &Window,
) -> AppResult<FileResult> {
    match job {
        FileJob::Open => {
            let Some(path) = dialog(window, "Open editable AFFINT document", "affint").pick_file()
            else {
                return Ok(FileResult::Canceled);
            };
            Ok(FileResult::Open {
                session: Box::new(store.load(&path)?),
                path,
            })
        }
        FileJob::Import { page } => {
            let Some(path) = dialog(window, "Import original PNG", "png").pick_file() else {
                return Ok(FileResult::Canceled);
            };
            Ok(FileResult::Imported {
                page,
                raster: affint_storage::load_png(
                    &path,
                    PngLimits {
                        max_encoded_bytes: 128 * 1024 * 1024,
                        max_pixels: 24_000_000,
                        max_decoder_bytes: 64 * 1024 * 1024,
                    },
                )?,
                path,
            })
        }
        FileJob::Save { snapshot, path } => {
            let path = match path {
                Some(path) => path,
                None => {
                    let Some(mut path) = dialog(window, "Save editable AFFINT document", "affint")
                        .set_file_name("Untitled.affint")
                        .save_file()
                    else {
                        return Ok(FileResult::Canceled);
                    };
                    if path.extension().is_none() {
                        path.set_extension("affint");
                    }
                    path
                }
            };
            Ok(FileResult::Saved {
                receipt: store.save(&snapshot, &path)?,
                path,
            })
        }
        FileJob::Export { snapshot, request } => {
            let Some(mut path) = dialog(window, "Export exact page PNG", "png")
                .set_file_name("Artwork.png")
                .save_file()
            else {
                return Ok(FileResult::Canceled);
            };
            if path.extension().is_none() {
                path.set_extension("png");
            }
            let frame = renderer.render(
                &snapshot,
                request.with_format(OutputFormat::SrgbRgba8Straight)?,
            )?;
            let durability = affint_storage::save_png(
                &path,
                affint_document::PixelSize::new(frame.width(), frame.height())?,
                frame
                    .rgba8()
                    .ok_or("PNG export requires straight sRGB RGBA8 output.")?,
            )?;
            Ok(FileResult::Exported { durability, path })
        }
    }
}

pub enum ClipboardJob {
    Read {
        focus: String,
        revision: affint_document::Revision,
        node: NodeId,
    },
    Write(String),
}
pub enum ClipboardResult {
    Read {
        focus: String,
        revision: affint_document::Revision,
        node: NodeId,
        text: String,
    },
    Written,
}
pub struct ClipboardWorker {
    sender: SyncSender<ClipboardJob>,
    receiver: Receiver<AppResult<ClipboardResult>>,
}
impl ClipboardWorker {
    pub fn new(proxy: EventLoopProxy<Wake>) -> AppResult<Self> {
        let (sender, jobs) = mpsc::sync_channel(1);
        let (results, receiver) = mpsc::sync_channel(1);
        thread::Builder::new().name("affint-clipboard".into()).spawn(move || {
            // Keep the native clipboard owner alive: X11 selections are process-owned.
            let mut clipboard = arboard::Clipboard::new();
            while let Ok(job) = jobs.recv() {
                let result = match &mut clipboard {
                    Ok(clipboard) => match job { ClipboardJob::Read { focus, revision, node } => clipboard.get_text().map_err(Into::into).and_then(|text| { if text.len() > 4096 { return Err("Clipboard text exceeds the 4096-byte layer-name limit. Copy a shorter name and paste again.".into()); } Ok(ClipboardResult::Read { focus, revision, node, text }) }), ClipboardJob::Write(text) => clipboard.set_text(text).map(|()| ClipboardResult::Written).map_err(Into::into) },
                    Err(error) => Err(format!("The system clipboard is unavailable: {error}").into()),
                };
                if results.send(result).is_err() || proxy.send_event(Wake).is_err() { return; }
            }
        })?;
        Ok(Self { sender, receiver })
    }
    pub fn submit(&self, job: ClipboardJob) -> AppResult<()> {
        self.sender
            .try_send(job)
            .map_err(|_| "The clipboard is busy or unavailable. Try the command again.".into())
    }
    pub fn take(&self) -> AppResult<Option<AppResult<ClipboardResult>>> {
        match self.receiver.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The clipboard worker stopped unexpectedly.".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_has_exactly_one_pending_snapshot() -> AppResult<()> {
        let shared = Shared::new(false);
        let worker = RenderWorker {
            shared: Arc::new(shared),
        };
        let snapshot = crate::editor::sample_document()?.snapshot();
        let request = RenderRequest::new(
            snapshot.page_order()[0],
            8,
            8,
            affint_render::View::Region(kurbo::Rect::new(0., 0., 960., 720.)),
        )?;
        for _ in 0..100 {
            worker.submit(snapshot.clone(), request, 1)?;
        }
        let state = worker
            .shared
            .state
            .lock()
            .map_err(|_| "Worker mutex poisoned.")?;
        assert_eq!(state.pending.as_ref().map(|entry| entry.serial), Some(100));
        assert!(state.result.is_none());
        drop(state);
        let stats = worker.stats()?;
        assert_eq!(stats.submitted, 100);
        assert_eq!(stats.pending_replaced, 99);
        assert_eq!(stats.started, 0);
        assert_eq!(stats.pending_serial, Some(100));
        assert_eq!(
            stats.submitted,
            stats.pending_replaced + stats.started + u64::from(stats.pending_serial.is_some())
        );
        Ok(())
    }
    #[test]
    fn stale_handoff_releases_the_completed_frame_budget() -> AppResult<()> {
        let shared = Shared::new(false);
        let worker = RenderWorker {
            shared: Arc::new(shared),
        };
        let snapshot = crate::editor::sample_document()?.snapshot();
        let request = RenderRequest::new(
            snapshot.page_order()[0],
            8,
            8,
            affint_render::View::Region(kurbo::Rect::new(0., 0., 960., 720.)),
        )?;
        let old = worker.submit(snapshot.clone(), request, 1)?;
        let started = worker
            .shared
            .state
            .lock()
            .map_err(|_| "Worker mutex poisoned.")?
            .take_job()
            .ok_or("No queued render.")?;
        assert_eq!(started.serial, old);
        let renderer = Renderer::new(Default::default())?;
        let frame = renderer.render(&snapshot, request)?;
        assert!(renderer.allocated_bytes() > 0);
        worker.submit(snapshot, request, 1)?;
        assert!(!worker.shared.publish(RenderResult {
            serial: old,
            generation: 1,
            timing: None,
            result: Ok(frame)
        })?);
        assert_eq!(renderer.allocated_bytes(), 0);
        let stats = worker.stats()?;
        assert_eq!(stats.submitted, 2);
        assert_eq!(stats.started, 1);
        assert_eq!(stats.finished, 1);
        assert_eq!(stats.produced, 1);
        assert_eq!(stats.canceled, 0);
        assert_eq!(stats.completion_rejected, 1);
        assert_eq!(stats.published, 0);
        assert_eq!(
            stats.started,
            stats.finished + u64::from(stats.active_serial.is_some())
        );
        assert_eq!(
            stats.finished,
            stats.produced + stats.canceled + stats.failed
        );
        Ok(())
    }
}
