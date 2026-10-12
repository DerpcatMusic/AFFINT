//! Bounded, opt-in native interaction observations. Serialization runs after exit.
use std::{
    collections::VecDeque,
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{AppResult, UploadKey};
use crate::{
    gpu::{DisplayPlan, PresentStats, Viewport},
    jobs::{RenderStats, RenderTiming},
};

const SAMPLE_LIMIT: usize = 4096;
const TIMING_LIMIT: usize = 128;
#[derive(Clone, Copy, Default)]
pub(super) struct Metrics {
    pub physical: [u32; 2],
    pub logical: [f64; 2],
    pub scale_factor: f64,
    pub viewport: Option<Viewport>,
    pub logical_viewport: Option<[f64; 4]>,
}

#[derive(Clone, Copy)]
struct InputStamp {
    id: u64,
    at: Instant,
    submitted: Option<u64>,
}
struct Sample {
    at: Instant,
    kind: &'static str,
    input: Option<InputStamp>,
    desired: Option<UploadKey>,
    tree: Duration,
    frame: Duration,
    total: Duration,
    calls: u8,
    render: RenderStats,
    present: PresentStats,
    response_to_present: Option<Duration>,
    exact_render_age: Option<Duration>,
    metrics: Metrics,
    retained: affint_render::RetainedStats,
}
pub(super) struct Recorder {
    origin: Instant,
    origin_unix: SystemTime,
    last_input: Option<InputStamp>,
    next_input: u64,
    samples: Vec<Sample>,
    timings: VecDeque<(u64, RenderTiming)>,
    dropped_samples: u64,
    dropped_timings: u64,
    response: Option<(u64, Duration)>,
    metrics: Metrics,
    retained: affint_render::RetainedStats,
}
impl Recorder {
    pub(super) fn new() -> Self {
        let origin = Instant::now();
        let origin_unix = SystemTime::now();
        Self {
            origin,
            origin_unix,
            last_input: None,
            next_input: 0,
            samples: Vec::with_capacity(SAMPLE_LIMIT),
            timings: VecDeque::with_capacity(TIMING_LIMIT),
            dropped_samples: 0,
            dropped_timings: 0,
            response: None,
            metrics: Metrics::default(),
            retained: affint_render::RetainedStats::default(),
        }
    }
    pub(super) fn input(&mut self) {
        self.next_input = self.next_input.saturating_add(1);
        self.last_input = Some(InputStamp {
            id: self.next_input,
            at: Instant::now(),
            submitted: None,
        });
        self.response = None;
    }
    pub(super) fn metrics(&mut self, metrics: Metrics, retained: affint_render::RetainedStats) {
        self.metrics = metrics;
        self.retained = retained;
    }
    pub(super) fn completed(&mut self, serial: u64, timing: Option<RenderTiming>) {
        if let Some(timing) = timing {
            if self.timings.len() == TIMING_LIMIT {
                self.timings.pop_front();
                self.dropped_timings = self.dropped_timings.saturating_add(1);
            }
            self.timings.push_back((serial, timing));
        }
    }
    pub(super) fn ui(
        &mut self,
        input: bool,
        tree: Duration,
        frame: Duration,
        total: Duration,
        calls: u8,
        desired: Option<UploadKey>,
    ) {
        if input && let Some(stamp) = &mut self.last_input {
            stamp.submitted = desired.map(|key| key.serial);
        }
        self.push(Sample {
            at: Instant::now(),
            kind: if input { "input" } else { "paint" },
            input: self.last_input,
            desired,
            tree,
            frame,
            total,
            calls,
            render: RenderStats::default(),
            present: PresentStats::default(),
            response_to_present: None,
            exact_render_age: None,
            metrics: self.metrics,
            retained: self.retained,
        });
    }
    pub(super) fn present(
        &mut self,
        render: RenderStats,
        present: PresentStats,
        desired: Option<UploadKey>,
    ) {
        let at = Instant::now();
        let sampled = present.latest.and_then(|timing| timing.sampled_key);
        if self.response.is_none()
            && let Some(input) = self.last_input.filter(|input| {
                input.submitted.is_some() && input.submitted == sampled.map(|key| key.serial)
            })
        {
            self.response = Some((input.id, at.saturating_duration_since(input.at)));
        }
        let response_to_present = self
            .response
            .filter(|(id, _)| Some(*id) == self.last_input.map(|input| input.id))
            .map(|(_, duration)| duration);
        let exact_render_age = sampled
            .and_then(|key| {
                self.timings
                    .iter()
                    .find(|(serial, _)| *serial == key.serial)
            })
            .map(|(_, timing)| at.saturating_duration_since(timing.queued_at));
        self.push(Sample {
            at,
            kind: "present",
            input: self.last_input,
            desired,
            tree: Duration::ZERO,
            frame: Duration::ZERO,
            total: Duration::ZERO,
            calls: 0,
            render,
            present,
            response_to_present,
            exact_render_age,
            metrics: self.metrics,
            retained: self.retained,
        });
    }
    fn push(&mut self, sample: Sample) {
        if self.samples.len() == SAMPLE_LIMIT {
            self.dropped_samples = self.dropped_samples.saturating_add(1);
        } else {
            self.samples.push(sample);
        }
    }
    pub(super) fn write(&self, directory: &Path) -> AppResult<()> {
        let origin_unix_ns = self.origin_unix.duration_since(UNIX_EPOCH)?.as_nanos();
        std::fs::create_dir_all(directory)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("interaction-profile.json"))?;
        let mut out = BufWriter::new(file);
        writeln!(
            out,
            "{{\"schema\":1,\"units\":\"cpu_wall_nanoseconds\",\"origin_unix_ns\":{origin_unix_ns},\"sample_limit\":{SAMPLE_LIMIT},\"sample_bytes_cap\":{},\"render_timing_limit\":{TIMING_LIMIT},\"render_timing_bytes_cap\":{},\"dropped_samples\":{},\"dropped_render_timings\":{},\"samples\":[",
            SAMPLE_LIMIT * std::mem::size_of::<Sample>(),
            TIMING_LIMIT * std::mem::size_of::<(u64, RenderTiming)>(),
            self.dropped_samples,
            self.dropped_timings
        )?;
        for (index, sample) in self.samples.iter().enumerate() {
            if index > 0 {
                writeln!(out, ",")?;
            }
            write!(
                out,
                "{{\"kind\":\"{}\",\"at_ns\":{},\"input_id\":{},\"input_at_ns\":{},\"input_submit_serial\":{},\"ui_tree_ns\":{},\"ui_frame_ns\":{},\"ui_total_ns\":{},\"ui_calls\":{},\"response_to_present_ns\":{},\"exact_render_age_ns\":{},\"desired\":",
                sample.kind,
                ns(sample.at.saturating_duration_since(self.origin)),
                number(sample.input.map(|input| input.id)),
                number(
                    sample
                        .input
                        .map(|input| ns(input.at.saturating_duration_since(self.origin)))
                ),
                number(sample.input.and_then(|input| input.submitted)),
                ns(sample.tree),
                ns(sample.frame),
                ns(sample.total),
                sample.calls,
                number(sample.response_to_present.map(ns)),
                number(sample.exact_render_age.map(ns))
            )?;
            key(&mut out, sample.desired)?;
            let m = sample.metrics;
            write!(
                out,
                ",\"window\":{{\"physical\":[{},{}],\"logical\":[{},{}],\"scale_factor\":{}",
                m.physical[0], m.physical[1], m.logical[0], m.logical[1], m.scale_factor
            )?;
            if let Some(v) = m.viewport {
                write!(
                    out,
                    ",\"viewport_physical\":[{},{},{},{}]",
                    v.x, v.y, v.width, v.height
                )?;
            }
            if let Some([x, y, w, h]) = m.logical_viewport {
                write!(out, ",\"viewport_logical\":[{x},{y},{w},{h}]")?;
            }
            let cache = sample.retained;
            write!(
                out,
                "}},\"retained\":{{\"tile_hits\":{},\"tile_misses\":{},\"evaluated_tiles\":{},\"geometry_hits\":{},\"geometry_misses\":{},\"prepared_curves\":{},\"preflight_reclaims\":{},\"evictions\":{},\"refused\":{},\"resident_bytes\":{},\"source_bytes\":{},\"retained_pixel_bytes\":{}}}",
                cache.tile_hits,
                cache.tile_misses,
                cache.evaluated_tiles,
                cache.geometry_hits,
                cache.geometry_misses,
                cache.prepared_curves,
                cache.preflight_reclaims,
                cache.evictions,
                cache.refused,
                cache.resident_bytes,
                cache.source_bytes,
                cache.retained_pixel_bytes
            )?;
            let r = sample.render;
            write!(
                out,
                ",\"render\":{{\"submitted\":{},\"pending_replaced\":{},\"pending_dropped_on_stop\":{},\"started\":{},\"finished\":{},\"produced\":{},\"canceled\":{},\"failed\":{},\"published\":{},\"completion_rejected\":{},\"taken\":{},\"result_replaced\":{},\"result_dropped_on_stop\":{},\"active_serial\":{},\"pending_serial\":{},\"result_pending\":{}",
                r.submitted,
                r.pending_replaced,
                r.pending_dropped_on_stop,
                r.started,
                r.finished,
                r.produced,
                r.canceled,
                r.failed,
                r.published,
                r.completion_rejected,
                r.taken,
                r.result_replaced,
                r.result_dropped_on_stop,
                number(r.active_serial),
                number(r.pending_serial),
                r.result_pending
            )?;
            if let Some((serial, timing)) = r.latest_timing {
                write!(
                    out,
                    ",\"latest_serial\":{serial},\"queue_ns\":{},\"evaluate_ns\":{}",
                    ns(timing
                        .started_at
                        .saturating_duration_since(timing.queued_at)),
                    ns(timing
                        .finished_at
                        .saturating_duration_since(timing.started_at))
                )?;
            }
            let p = sample.present;
            write!(
                out,
                "}},\"host\":{{\"attempts\":{},\"presented\":{},\"retries\":{},\"hidden\":{},\"failed\":{},\"conversions_encoded\":{}",
                p.attempts, p.presented, p.retries, p.hidden, p.failed, p.conversions_encoded
            )?;
            if let Some(t) = p.latest {
                write!(
                    out,
                    ",\"total_ns\":{},\"poll_ns\":{},\"acquire_ns\":{},\"compose_ns\":{},\"gpu_checks_ns\":{},\"conversion_encode_ns\":{},\"canvas_encode_ns\":{},\"chrome_paint_ns\":{},\"hybrid_encode_ns\":{},\"submit_ns\":{},\"surface_present_ns\":{},\"converted\":{},\"sampled\":",
                    ns(t.finished_at.saturating_duration_since(t.started_at)),
                    ns(t.poll),
                    ns(t.acquire),
                    ns(t.compose),
                    ns(t.gpu_checks),
                    ns(t.conversion_encode),
                    ns(t.canvas_encode),
                    ns(t.chrome_paint),
                    ns(t.hybrid_encode),
                    ns(t.submit),
                    ns(t.surface_present),
                    t.converted
                )?;
                key(&mut out, t.sampled_key)?;
                write!(out, ",\"accepted\":")?;
                key(&mut out, t.accepted_key)?;
                write!(out, ",\"display_plan\":")?;
                plan(&mut out, t.plan)?;
            }
            write!(out, "}}}}")?;
        }
        writeln!(out, "]}}")?;
        out.flush()?;
        Ok(())
    }
}
fn plan(out: &mut impl Write, value: Option<DisplayPlan>) -> std::io::Result<()> {
    let Some(plan) = value else {
        return write!(out, "null");
    };
    write!(out, "{{\"evaluation\":")?;
    key(out, Some(plan.evaluation.key))?;
    let [a, b, c, d, e, f] = plan.document_to_window().as_coeffs();
    write!(
        out,
        ",\"source_margin\":{},\"paint_size\":[{},{}],\"source_offset\":[{},{}],\"document_to_window_physical\":[{a},{b},{c},{d},{e},{f}],\"mode\":\"{:?}\",\"pending\":{},\"fresh_viewport_reference_equivalence\":\"not_asserted\"}}",
        plan.evaluation.margin,
        plan.paint_size[0],
        plan.paint_size[1],
        plan.source_offset[0],
        plan.source_offset[1],
        plan.mode,
        plan.pending
    )
}
fn ns(value: Duration) -> u64 {
    u64::try_from(value.as_nanos()).unwrap_or(u64::MAX)
}
fn number(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}
fn key(out: &mut impl Write, value: Option<UploadKey>) -> std::io::Result<()> {
    if let Some(value) = value {
        write!(
            out,
            "{{\"serial\":{},\"generation\":{},\"revision\":\"{:?}\",\"page\":{},\"width\":{},\"height\":{},\"format\":\"{:?}\",\"view\":",
            value.serial,
            value.generation,
            value.revision,
            value.request.page().get(),
            value.request.width(),
            value.request.height(),
            value.request.format()
        )?;
        match value.request.view() {
            affint_render::View::Affine(transform) => {
                let [a, b, c, d, e, f] = transform.affine().as_coeffs();
                write!(out, "{{\"affine\":[{a},{b},{c},{d},{e},{f}]}}")?;
            }
            affint_render::View::Region(rect) => write!(
                out,
                "{{\"region\":[{},{},{},{}]}}",
                rect.x0, rect.y0, rect.x1, rect.y1
            )?,
        }
        write!(out, "}}")
    } else {
        write!(out, "null")
    }
}
