//! Bounded native canvas upload, with one source owner and explicit GPU retirement.
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use affint_document::Revision;
use affint_render::{Frame, FramePixels, OutputFormat, RenderRequest};
use kurbo::Rect;
use winit::event_loop::EventLoopProxy;

use crate::{Wake, editor::AppResult};

const CANVAS_BYTES: usize = 320 * 1024 * 1024;
const BAND_BYTES: usize = 8 * 1024 * 1024;
const STAGING_BYTES: usize = 2 * BAND_BYTES;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub revision: Revision,
    pub request: RenderRequest,
    pub generation: u64,
    pub serial: u64,
}
impl Key {
    pub fn matches(self, desired: Option<(Revision, RenderRequest, u64, u64)>) -> bool {
        desired == Some((self.revision, self.request, self.generation, self.serial))
    }
    pub fn accepts(self, frame: &Frame, generation: u64) -> bool {
        self.generation == generation
            && frame.revision() == self.revision
            && frame.request() == self.request
            && self.request.format() == OutputFormat::LinearSrgbRgbaF32Premultiplied
    }
}
#[derive(Clone)]
pub struct Context {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub generation: u64,
}
impl Context {
    pub fn new(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        generation: u64,
    ) -> AppResult<Self> {
        if !cfg!(target_endian = "little") {
            return Err(
                "Native F32 canvas requires a little-endian host; precision is not downgraded."
                    .into(),
            );
        }
        for (format, usage) in [
            (
                wgpu::TextureFormat::Rgba32Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            (
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC,
            ),
        ] {
            if !adapter
                .get_texture_format_features(format)
                .allowed_usages
                .contains(usage)
            {
                return Err(format!("This GPU cannot provide the exact native {format:?} canvas usages. The source is preserved; no precision fallback is used.").into());
            }
        }
        Ok(Self {
            device,
            queue,
            generation,
        })
    }
}
#[derive(Debug, Default)]
struct Counts {
    textures: usize,
    pairs: usize,
    staging: usize,
}
#[derive(Debug, Default)]
struct Budget {
    counts: Mutex<Counts>,
}
impl Budget {
    fn admission(&self, bytes: usize) -> AppResult<Admission> {
        let counts = self
            .counts
            .lock()
            .map_err(|_| "Canvas accounting stopped unexpectedly.")?;
        if bytes > CANVAS_BYTES
            || (counts.pairs <= 1
                && counts
                    .textures
                    .checked_add(bytes)
                    .is_none_or(|n| n > CANVAS_BYTES))
        {
            return Ok(Admission::Refused);
        }
        if counts.pairs >= 2
            || counts
                .textures
                .checked_add(bytes)
                .is_none_or(|n| n > CANVAS_BYTES)
        {
            return Ok(Admission::Pending);
        }
        Ok(Admission::Ready)
    }
    fn pair(self: &Arc<Self>, bytes: usize) -> AppResult<Lease> {
        let mut counts = self
            .counts
            .lock()
            .map_err(|_| "Canvas accounting stopped unexpectedly.")?;
        let total = counts
            .textures
            .checked_add(bytes)
            .ok_or("Canvas texture arithmetic overflow.")?;
        if counts.pairs >= 2 || total > CANVAS_BYTES {
            return Err("Canvas graphics are still retiring or exceed the 320 MiB texture limit. Keep the previous image, wait, or reduce the window size.".into());
        }
        counts.textures = total;
        counts.pairs += 1;
        Ok(Lease {
            budget: Arc::clone(self),
            bytes,
            staging: false,
        })
    }
    fn staging(self: &Arc<Self>) -> AppResult<Lease> {
        let mut counts = self
            .counts
            .lock()
            .map_err(|_| "Canvas accounting stopped unexpectedly.")?;
        if counts.staging != 0 {
            return Err("Previous canvas upload buffers have not retired yet.".into());
        }
        counts.staging = STAGING_BYTES;
        Ok(Lease {
            budget: Arc::clone(self),
            bytes: STAGING_BYTES,
            staging: true,
        })
    }
}
struct Lease {
    budget: Arc<Budget>,
    bytes: usize,
    staging: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut counts) = self.budget.counts.lock() {
            if self.staging {
                counts.staging -= self.bytes;
            } else {
                counts.textures -= self.bytes;
                counts.pairs -= 1;
            }
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    Ready,
    Pending,
    Refused,
}

#[derive(Debug, Default)]
pub struct Stats {
    pub bands: usize,
    pub copied_bytes: usize,
    /// Sum of stride × rows for buffer row spans, not measured bus traffic.
    pub padded_bytes: usize,
    pub row_stride: usize,
}

pub struct Canvas {
    pub key: Key,
    pub artboard: Rect,
    pub source: wgpu::Texture,
    pub source_view: wgpu::TextureView,
    pub display: wgpu::Texture,
    pub display_view: wgpu::TextureView,
    pub stats: Stats,
    _lease: Lease,
}
impl Canvas {
    fn new(context: &Context, key: Key, artboard: Rect, lease: Lease) -> Self {
        let descriptor = |label, format, usage| wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: key.request.width(),
                height: key.request.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        };
        let source = context.device.create_texture(&descriptor(
            "AFFINT F32 working canvas",
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        ));
        let display = context.device.create_texture(&descriptor(
            "AFFINT opaque canvas",
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        ));
        let source_view = source.create_view(&Default::default());
        let display_view = display.create_view(&Default::default());
        Self {
            key,
            artboard,
            source,
            source_view,
            display,
            display_view,
            stats: Stats::default(),
            _lease: lease,
        }
    }
}
struct Pool {
    buffers: [wgpu::Buffer; 2],
    mapped: [bool; 2],
    generation: u64,
    _lease: Lease,
}
impl Pool {
    fn new(context: &Context, budget: &Arc<Budget>) -> AppResult<Self> {
        let lease = budget.staging()?;
        let descriptor = wgpu::BufferDescriptor {
            label: Some("AFFINT bounded upload band"),
            size: BAND_BYTES as u64,
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        };
        Ok(Self {
            buffers: [
                context.device.create_buffer(&descriptor),
                context.device.create_buffer(&descriptor),
            ],
            mapped: [true; 2],
            generation: context.generation,
            _lease: lease,
        })
    }
}
struct Job {
    frame: Frame,
    key: Key,
    artboard: Rect,
    context: Context,
    lease: Lease,
}
pub struct Completed {
    pub key: Key,
    pub result: AppResult<Canvas>,
}
/// GPU fence completion may precede callback execution on another polling thread.
/// This token acknowledges the callback's owner drop, rather than just the fence.
#[derive(Default)]
struct Completion {
    done: Mutex<bool>,
    wake: Condvar,
}
impl Completion {
    fn finished(&self) -> bool {
        self.done.lock().is_ok_and(|done| *done)
    }
    fn finish<T>(&self, owner: T) -> AppResult<()> {
        drop(owner);
        let mut done = self
            .done
            .lock()
            .map_err(|_| "GPU retirement acknowledgement stopped.")?;
        *done = true;
        self.wake.notify_all();
        Ok(())
    }
    fn wait(&self, deadline: Instant) -> AppResult<()> {
        let mut done = self
            .done
            .lock()
            .map_err(|_| "GPU retirement acknowledgement stopped.")?;
        while !*done {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("GPU retirement callback did not finish within two seconds; its owners have not been acknowledged or forcibly released.".into());
            }
            let (next, _) = self
                .wake
                .wait_timeout(done, remaining)
                .map_err(|_| "GPU retirement acknowledgement stopped.")?;
            done = next;
        }
        Ok(())
    }
}
struct Poll {
    device: wgpu::Device,
    done: Arc<Completion>,
}
struct State {
    job: Option<Job>,
    result: Option<Completed>,
    desired: Option<Key>,
    busy: bool,
    stop: bool,
    polls: Vec<Poll>,
    pool_available: bool,
    error: Option<String>,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    budget: Arc<Budget>,
    proxy: Option<EventLoopProxy<Wake>>,
}
impl Shared {
    fn notify(&self) {
        if let Some(proxy) = &self.proxy {
            let _ = proxy.send_event(Wake);
        }
    }
    fn current(&self, key: Key) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| !state.stop && state.desired == Some(key))
    }
    fn retire<T: Send + 'static>(self: &Arc<Self>, context: &Context, owner: T) {
        let done = Arc::new(Completion::default());
        let signal = Arc::clone(&done);
        let shared = Arc::clone(self);
        context.queue.on_submitted_work_done(move || {
            if let Err(error) = signal.finish(owner)
                && let Ok(mut state) = shared.state.lock()
            {
                state.error = Some(error.to_string());
            }
            shared.wake.notify_one();
            shared.notify();
        });
        if let Ok(mut state) = self.state.lock() {
            state.polls.push(Poll {
                device: context.device.clone(),
                done,
            });
        }
        self.wake.notify_one();
    }
}
/// Keeps charged GPU owners alive and drives completion even while the window sleeps.
#[derive(Clone)]
pub struct Retirement {
    shared: Arc<Shared>,
}
impl Retirement {
    pub fn retire<T: Send + 'static>(&self, context: &Context, owner: T) {
        self.shared.retire(context, owner);
    }
}
/// A fixed worker; a busy uploader leaves the sole completion in the render mailbox.
pub struct Worker {
    shared: Arc<Shared>,
}
impl Worker {
    pub fn new(proxy: EventLoopProxy<Wake>) -> AppResult<Self> {
        Self::with_proxy(Some(proxy))
    }
    pub fn diagnostic() -> AppResult<Self> {
        Self::with_proxy(None)
    }
    fn with_proxy(proxy: Option<EventLoopProxy<Wake>>) -> AppResult<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                job: None,
                result: None,
                desired: None,
                busy: false,
                stop: false,
                polls: Vec::with_capacity(3),
                pool_available: false,
                error: None,
            }),
            wake: Condvar::new(),
            budget: Arc::new(Budget::default()),
            proxy,
        });
        let worker = Arc::clone(&shared);
        thread::Builder::new().name("affint-upload".into()).spawn(move || {
            let mut pool = None;
            loop {
                let job = {
                    let Ok(mut state) = worker.state.lock() else { return; };
                    state.polls.retain(|poll| !poll.done.finished());
                    let devices: Vec<_> = state.polls.iter().map(|poll| poll.device.clone()).collect();
                    if !devices.is_empty() {
                        drop(state);
                        let mut failure = None;
                        for device in devices {
                            if let Err(error) = device.poll(wgpu::PollType::Poll) { failure = Some(format!("Canvas GPU retirement polling failed: {error}. Graphics owners remain charged.")); }
                        }
                        let Ok(next) = worker.state.lock() else { return; };
                        state = next;
                        if let Some(failure) = failure { state.error = Some(failure); worker.notify(); }
                        state.polls.retain(|poll| !poll.done.finished());
                    }
                    if state.stop && state.polls.is_empty() { return; }
                    if let Some(job) = state.job.take() { Some(job) }
                    else {
                        if state.polls.is_empty() {
                            let Ok(next) = worker.wake.wait(state) else { return; };
                            drop(next);
                        } else {
                            let Ok((next, _)) = worker.wake.wait_timeout(state, Duration::from_millis(20)) else { return; };
                            drop(next);
                        }
                        None
                    }
                };
                let Some(job) = job else { continue; };
                let key = job.key;
                let result = crate::jobs::guard("Canvas upload", || run(job, &worker, &mut pool));
                if let Ok(mut state) = worker.state.lock() {
                    state.pool_available = pool.is_some();
                    if !state.stop && state.desired == Some(key) { state.result = Some(Completed { key, result }); }
                    else { drop(result); }
                    state.busy = false;
                }
                worker.wake.notify_all();
                worker.notify();
            }
        })?;
        Ok(Self { shared })
    }
    pub fn retirement(&self) -> Retirement {
        Retirement {
            shared: Arc::clone(&self.shared),
        }
    }
    pub fn accounting(&self) -> AppResult<(usize, usize, usize)> {
        let counts = self
            .shared
            .budget
            .counts
            .lock()
            .map_err(|_| "Canvas accounting stopped.")?;
        Ok((counts.textures, counts.pairs, counts.staging))
    }
    pub fn take_error(&self) -> AppResult<Option<String>> {
        Ok(self
            .shared
            .state
            .lock()
            .map_err(|_| "Canvas upload worker stopped.")?
            .error
            .take())
    }
    pub fn desire(&self, key: Option<Key>) -> AppResult<()> {
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| "Canvas upload worker stopped.")?;
        state.desired = key;
        if state
            .result
            .as_ref()
            .is_some_and(|result| Some(result.key) != key)
        {
            state.result = None;
        }
        self.shared.wake.notify_one();
        Ok(())
    }
    pub fn admission(&self, request: RenderRequest) -> AppResult<Admission> {
        let state = self
            .shared
            .state
            .lock()
            .map_err(|_| "Canvas upload worker stopped.")?;
        let bytes = pair_bytes(request)?;
        let admission = self.shared.budget.admission(bytes)?;
        if admission != Admission::Ready {
            return Ok(admission);
        }
        let counts = self
            .shared
            .budget
            .counts
            .lock()
            .map_err(|_| "Canvas accounting stopped.")?;
        if state.busy || state.result.is_some() || (counts.staging != 0 && !state.pool_available) {
            return Ok(Admission::Pending);
        }
        Ok(Admission::Ready)
    }
    pub fn submit(
        &self,
        frame: Frame,
        key: Key,
        artboard: Rect,
        context: Context,
    ) -> AppResult<()> {
        if !key.accepts(&frame, context.generation) {
            return Err("Canvas upload identity/format changed before handoff.".into());
        }
        let limit = context
            .device
            .limits()
            .max_texture_dimension_2d
            .min(u32::from(u16::MAX));
        if key.request.width() > limit || key.request.height() > limit {
            return Err(
                "The exact viewport exceeds this GPU's texture dimensions. Reduce the window size."
                    .into(),
            );
        }
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| "Canvas upload worker stopped.")?;
        if state.busy || state.result.is_some() || state.desired != Some(key) {
            return Err("Canvas upload handoff is obsolete or busy.".into());
        }
        let lease = self.shared.budget.pair(pair_bytes(key.request)?)?;
        state.busy = true;
        state.job = Some(Job {
            frame,
            key,
            artboard,
            context,
            lease,
        });
        self.shared.wake.notify_one();
        Ok(())
    }
    pub fn take(&self) -> AppResult<Option<Completed>> {
        Ok(self
            .shared
            .state
            .lock()
            .map_err(|_| "Canvas upload worker stopped.")?
            .result
            .take())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.stop = true;
            state.desired = None;
            state.job = None;
            state.result = None;
        }
        self.shared.wake.notify_one();
    }
}
fn pair_bytes(request: RenderRequest) -> AppResult<usize> {
    let bytes = u64::from(request.width())
        .checked_mul(u64::from(request.height()))
        .and_then(|n| n.checked_mul(20))
        .ok_or("Canvas texture size overflow.")?;
    usize::try_from(bytes).map_err(Into::into)
}
fn byte_view(frame: &Frame) -> AppResult<&[u8]> {
    if !cfg!(target_endian = "little") {
        return Err("Native F32 canvas upload currently requires a little-endian host; precision is not downgraded.".into());
    }
    match frame.pixels() {
        FramePixels::LinearSrgbRgbaF32Premultiplied(pixels) => {
            bytemuck::try_cast_slice::<[f32; 4], u8>(pixels).map_err(|error| {
                format!("Cannot borrow working pixels for upload: {error:?}").into()
            })
        }
        FramePixels::SrgbRgba8Straight(_) => {
            Err("Native canvas requires linear-premultiplied F32 output.".into())
        }
    }
}
struct Work {
    canvas: Option<Canvas>,
    pool: Option<Pool>,
    context: Context,
    shared: Arc<Shared>,
    outstanding: bool,
}
impl Drop for Work {
    fn drop(&mut self) {
        if self.outstanding {
            self.shared
                .retire(&self.context, (self.canvas.take(), self.pool.take()));
        }
    }
}
fn wait(context: &Context, index: wgpu::SubmissionIndex) -> AppResult<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match context.device.poll(wgpu::PollType::Wait { submission_index: Some(index.clone()), timeout: Some(Duration::from_millis(50)) }) {
            Ok(status) if status.wait_finished() => return Ok(()),
            Ok(_) | Err(wgpu::PollError::Timeout) if Instant::now() < deadline => {},
            Ok(_) | Err(wgpu::PollError::Timeout) => return Err("GPU upload did not finish within two seconds. The previous canvas is retained; save and reopen if graphics do not recover.".into()),
            Err(error) => return Err(error.into()),
        }
    }
}
fn run(job: Job, shared: &Arc<Shared>, saved_pool: &mut Option<Pool>) -> AppResult<Canvas> {
    let device = job.context.device.clone();
    checked_gpu(&device, || {
        crate::jobs::guard("Canvas upload", || upload(job, shared, saved_pool))
    })
}
/// Native wgpu scopes are thread-local; inspect all failures at the workload seam.
pub fn checked_gpu<T>(
    device: &wgpu::Device,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = operation();
    let errors = [
        pollster::block_on(validation.pop()),
        pollster::block_on(internal.pop()),
        pollster::block_on(memory.pop()),
    ];
    if let Some(error) = errors.into_iter().flatten().next() {
        return Err(format!(
            "Canvas GPU operation rejected: {error}. The previous valid image is retained."
        )
        .into());
    }
    result
}
fn upload(job: Job, shared: &Arc<Shared>, saved_pool: &mut Option<Pool>) -> AppResult<Canvas> {
    if !shared.current(job.key) {
        return Err("Obsolete canvas upload canceled.".into());
    }
    if saved_pool
        .as_ref()
        .is_some_and(|pool| pool.generation != job.key.generation)
    {
        *saved_pool = None;
    }
    let pool = match saved_pool.take() {
        Some(pool) => pool,
        None => Pool::new(&job.context, &shared.budget)?,
    };
    let canvas = Canvas::new(&job.context, job.key, job.artboard, job.lease);
    let mut work = Work {
        canvas: Some(canvas),
        pool: Some(pool),
        context: job.context,
        shared: Arc::clone(shared),
        outstanding: false,
    };
    let bytes = byte_view(&job.frame)?;
    let expected_bytes = usize::try_from(
        u64::from(job.key.request.width())
            .checked_mul(u64::from(job.key.request.height()))
            .and_then(|pixels| pixels.checked_mul(16))
            .ok_or("Working frame size overflow.")?,
    )?;
    if bytes.len() != expected_bytes {
        return Err("Working frame byte length does not match its exact dimensions.".into());
    }
    let row = usize::try_from(job.key.request.width())?
        .checked_mul(16)
        .ok_or("Upload row size overflow.")?;
    let stride = row
        .checked_add(255)
        .map(|n| n & !255)
        .ok_or("Upload alignment overflow.")?;
    let rows_per_band = BAND_BYTES
        .checked_div(stride)
        .filter(|rows| *rows > 0)
        .ok_or("One working row exceeds the upload band limit.")?;
    let height = usize::try_from(job.key.request.height())?;
    let mut stats = Stats {
        row_stride: stride,
        ..Default::default()
    };
    let mut y = 0;
    let mut band = 0;
    while y < height {
        if !shared.current(job.key) {
            return Err("Obsolete canvas upload canceled.".into());
        }
        let rows = rows_per_band.min(height - y);
        let index = band % 2;
        let pool = work
            .pool
            .as_mut()
            .ok_or("Canvas upload buffers are missing.")?;
        if !pool.mapped[index] {
            let (sender, receiver) = mpsc::sync_channel(1);
            pool.buffers[index]
                .slice(..)
                .map_async(wgpu::MapMode::Write, move |result| {
                    let _ = sender.send(result);
                });
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if !shared.current(job.key) {
                    work.outstanding = true;
                    return Err("Obsolete canvas upload canceled during buffer mapping.".into());
                }
                work.context.device.poll(wgpu::PollType::Poll)?;
                match receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(result) => {
                        result?;
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                    Err(error) => {
                        work.outstanding = true;
                        return Err(format!("GPU upload buffer mapping did not finish: {error}. Previous artwork is retained.").into());
                    }
                }
            }
            pool.mapped[index] = true;
        }
        {
            let mut mapped = pool.buffers[index].slice(..).get_mapped_range_mut();
            for local in 0..rows {
                let start = (y + local)
                    .checked_mul(row)
                    .ok_or("Upload source offset overflow.")?;
                let end = start
                    .checked_add(row)
                    .ok_or("Upload source row overflow.")?;
                let target_start = local
                    .checked_mul(stride)
                    .ok_or("Upload staging offset overflow.")?;
                let target_end = target_start
                    .checked_add(row)
                    .ok_or("Upload staging row overflow.")?;
                if target_end > mapped.len() {
                    return Err("Upload row exceeds the fixed staging buffer.".into());
                }
                // wgpu 29 exposes write-combining memory through WriteOnly;
                // BufferViewMut deliberately provides no mutable slice deref.
                mapped.slice(target_start..target_end).copy_from_slice(
                    bytes
                        .get(start..end)
                        .ok_or("Upload source row is missing.")?,
                );
            }
        }
        pool.buffers[index].unmap();
        pool.mapped[index] = false;
        let source = &work
            .canvas
            .as_ref()
            .ok_or("Canvas upload target is missing.")?
            .source;
        let mut encoder =
            work.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("AFFINT upload band"),
                });
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &pool.buffers[index],
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(u32::try_from(stride)?),
                    rows_per_image: Some(u32::try_from(rows)?),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: source,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: u32::try_from(y)?,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: job.key.request.width(),
                height: u32::try_from(rows)?,
                depth_or_array_layers: 1,
            },
        );
        work.outstanding = true;
        let submitted = work.context.queue.submit([encoder.finish()]);
        y += rows;
        band += 1;
        wait(&work.context, submitted)?;
        work.outstanding = false;
        stats.bands += 1;
        stats.copied_bytes = stats
            .copied_bytes
            .checked_add(rows * row)
            .ok_or("Uploaded byte count overflow.")?;
        stats.padded_bytes = stats
            .padded_bytes
            .checked_add(rows * stride)
            .ok_or("Padded upload byte count overflow.")?;
    }
    // All bytes were borrowed into bounded staging; no CPU pixel owner is retained.
    drop(job.frame);
    if !shared.current(job.key) {
        return Err("Obsolete canvas upload canceled.".into());
    }
    *saved_pool = work.pool.take();
    let mut canvas = work
        .canvas
        .take()
        .ok_or("Canvas upload completed without a target.")?;
    canvas.stats = stats;
    Ok(canvas)
}

/// The explicit proof borrows the same frame and runs the ordinary band uploader.
pub fn diagnostic_upload(
    worker: &Worker,
    frame: Frame,
    key: Key,
    artboard: Rect,
    context: Context,
) -> AppResult<Canvas> {
    if !key.accepts(&frame, context.generation) {
        return Err("Diagnostic working-frame identity mismatch.".into());
    }
    worker.desire(Some(key))?;
    worker.submit(frame, key, artboard, context)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut state = worker
        .shared
        .state
        .lock()
        .map_err(|_| "Diagnostic upload worker stopped.")?;
    loop {
        if let Some(completed) = state.result.take() {
            return completed.result;
        }
        if Instant::now() >= deadline {
            return Err("Diagnostic upload did not complete within 30 seconds.".into());
        }
        let (next, _) = worker
            .shared
            .wake
            .wait_timeout(state, Duration::from_millis(20))
            .map_err(|_| "Diagnostic upload worker stopped.")?;
        state = next;
    }
}
/// Cancels through the real mailbox seam. Timing may stop a queued job, a partial
/// upload, or a completed private result; none may remain published afterward.
pub fn diagnostic_cancel(
    worker: &Worker,
    frame: Frame,
    key: Key,
    artboard: Rect,
    context: Context,
) -> AppResult<()> {
    worker.desire(Some(key))?;
    worker.submit(frame, key, artboard, context)?;
    worker.desire(None)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut state = worker
        .shared
        .state
        .lock()
        .map_err(|_| "Diagnostic upload worker stopped.")?;
    loop {
        if !state.busy && state.job.is_none() {
            if state.result.is_some() {
                return Err("Obsolete upload remained published in the mailbox.".into());
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("Canceled diagnostic upload did not retire within 30 seconds.".into());
        }
        let (next, _) = worker
            .shared
            .wake
            .wait_timeout(state, Duration::from_millis(20))
            .map_err(|_| "Diagnostic upload worker stopped.")?;
        state = next;
    }
}

/// Explicit diagnostics wait for currently registered callback owner-drop
/// acknowledgements. Ordinary presentation keeps retirement asynchronous and its
/// finite worker poll active; queue completion alone is not callback completion.
pub fn diagnostic_retirement(worker: &Worker) -> AppResult<()> {
    let completions: Vec<_> = worker
        .shared
        .state
        .lock()
        .map_err(|_| "Diagnostic upload worker stopped.")?
        .polls
        .iter()
        .map(|poll| Arc::clone(&poll.done))
        .collect();
    let deadline = Instant::now() + Duration::from_secs(2);
    for completion in completions {
        completion.wait(deadline)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retirement_acknowledgement_waits_until_callback_owner_drop_releases_admission()
    -> AppResult<()> {
        struct HeldOwner {
            entered: mpsc::SyncSender<()>,
            release: mpsc::Receiver<()>,
            _lease: Lease,
        }
        impl Drop for HeldOwner {
            fn drop(&mut self) {
                let _ = self.entered.send(());
                // Channel closure also releases this test owner during cleanup.
                let _ = self.release.recv();
            }
        }
        let budget = Arc::new(Budget::default());
        let current = budget.pair(129_000_000)?;
        let retired = budget.pair(129_000_000)?;
        let completion = Arc::new(Completion::default());
        let callback_completion = Arc::clone(&completion);
        let (entered_sender, entered_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let owner = HeldOwner {
            entered: entered_sender,
            release: release_receiver,
            _lease: retired,
        };
        let callback = thread::spawn(move || callback_completion.finish(owner));
        entered_receiver.recv_timeout(Duration::from_secs(2))?;
        // A polling thread has taken the callback, but its owner drop is still
        // executing. A completed GPU fence must not stand in for this token.
        assert!(!completion.finished());
        assert!(completion.wait(Instant::now()).is_err());
        assert_eq!(budget.admission(129_000_000)?, Admission::Pending);
        release_sender.send(())?;
        completion.wait(Instant::now() + Duration::from_secs(2))?;
        callback.join().map_err(|_| "Test callback panicked.")??;
        assert!(completion.finished());
        assert_eq!(budget.admission(129_000_000)?, Admission::Ready);
        assert_eq!(
            budget
                .counts
                .lock()
                .map_err(|_| "Test accounting stopped.")?
                .textures,
            129_000_000
        );
        drop(current);
        assert_eq!(
            budget
                .counts
                .lock()
                .map_err(|_| "Test accounting stopped.")?
                .textures,
            0
        );
        Ok(())
    }
    #[test]
    fn pair_admission_counts_retired_owners_and_failure_preserves_current() -> AppResult<()> {
        let budget = Arc::new(Budget::default());
        let current = budget.pair(129_000_000)?;
        let candidate = budget.pair(129_000_000)?;
        assert_eq!(budget.admission(129_000_000)?, Admission::Pending);
        assert!(budget.pair(1).is_err());
        drop(candidate);
        assert_eq!(budget.admission(129_000_000)?, Admission::Ready);
        assert_eq!(budget.admission(CANVAS_BYTES)?, Admission::Refused);
        drop(current);
        assert_eq!(
            budget
                .counts
                .lock()
                .map_err(|_| "Poisoned accounting.")?
                .textures,
            0
        );
        Ok(())
    }
    #[test]
    fn staging_pool_cannot_duplicate_during_device_retirement() -> AppResult<()> {
        let budget = Arc::new(Budget::default());
        let retained = budget.staging()?;
        assert!(budget.staging().is_err());
        assert_eq!(
            budget
                .counts
                .lock()
                .map_err(|_| "Poisoned accounting.")?
                .staging,
            16 * 1024 * 1024
        );
        drop(retained);
        drop(budget.staging()?);
        assert_eq!(
            budget
                .counts
                .lock()
                .map_err(|_| "Poisoned accounting.")?
                .staging,
            0
        );
        Ok(())
    }
    #[test]
    fn native_handoff_rejects_export_format_stale_view_revision_and_generation() -> AppResult<()> {
        let mut session = crate::editor::sample_document()?;
        let snapshot = session.snapshot();
        let request = RenderRequest::new(
            snapshot.page_order()[0],
            8,
            8,
            affint_render::View::Region(Rect::new(0., 0., 960., 720.)),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let renderer = affint_render::Renderer::new(Default::default())?;
        let frame = renderer.render(&snapshot, request)?;
        let key = Key {
            revision: snapshot.revision(),
            request,
            generation: 7,
            serial: 19,
        };
        assert!(key.accepts(&frame, 7));
        assert!(!key.accepts(&frame, 8));
        assert!(
            !Key {
                request: request.with_format(OutputFormat::SrgbRgba8Straight)?,
                ..key
            }
            .accepts(&frame, 7)
        );
        let different_view = RenderRequest::new(
            request.page(),
            8,
            8,
            affint_render::View::Region(Rect::new(1., 0., 961., 720.)),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        assert!(
            !Key {
                request: different_view,
                ..key
            }
            .accepts(&frame, 7)
        );
        let node = *snapshot
            .page(request.page())
            .ok_or("Fixture page missing.")?
            .roots()
            .first()
            .ok_or("Fixture layer missing.")?;
        let mut layer =
            crate::editor::layer_spec(snapshot.node(node).ok_or("Fixture layer missing.")?);
        layer.opacity = 0.5;
        session.transact(affint_document::Edit::new(
            "New revision",
            [affint_document::Change::SetLayer { node, layer }],
        ))?;
        assert!(
            !Key {
                revision: session.snapshot().revision(),
                ..key
            }
            .accepts(&frame, 7)
        );
        assert!(!key.matches(Some((
            key.revision,
            key.request,
            key.generation,
            key.serial + 1
        ))));
        let exported = renderer.render(
            &snapshot,
            request.with_format(OutputFormat::SrgbRgba8Straight)?,
        )?;
        assert!(byte_view(&exported).is_err());
        assert_eq!(byte_view(&frame)?.len(), 8 * 8 * 16);
        drop(frame);
        drop(exported);
        assert_eq!(renderer.allocated_bytes(), 0);
        Ok(())
    }
    #[test]
    fn replacement_and_cancellation_invalidate_every_publication_identity() -> AppResult<()> {
        let worker = Worker::diagnostic()?;
        let snapshot = crate::editor::sample_document()?.snapshot();
        let request = RenderRequest::new(
            snapshot.page_order()[0],
            8,
            8,
            affint_render::View::Region(Rect::new(0., 0., 960., 720.)),
        )?
        .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
        let key = Key {
            revision: snapshot.revision(),
            request,
            generation: 1,
            serial: 1,
        };
        worker.desire(Some(key))?;
        assert!(worker.shared.current(key));
        worker.desire(Some(Key { serial: 2, ..key }))?;
        assert!(!worker.shared.current(key));
        worker.desire(Some(Key {
            generation: 2,
            ..key
        }))?;
        assert!(!worker.shared.current(key));
        worker.desire(None)?;
        assert!(!worker.shared.current(Key {
            generation: 2,
            ..key
        }));
        assert_eq!(worker.accounting()?, (0, 0, 0));
        Ok(())
    }
}
