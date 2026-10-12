# Native interaction latency: source findings and coordinator probes

2026-10-12. Baseline `a0ad784`. Source review only: no worker builds, profiles,
tests, native input or runtime execution. User reports a very laggy canvas; the
findings below explain concrete work/publication paths, not measured attribution.
GPU document-backend planning is stopped for this increment.

## Highest-confidence defect and current correction

Baseline `App::draw` calls `process_motion`, which calls `tick`, then calls `tick`
again with the same pointer. Each tick authors/resolves two MUI trees. After its
first MUI frame, `canvas_input` calls `Editor::drag` whenever `response.dragged` is
true. The pinned MUI input implementation sets `dragged = held && dragging`; it
does not require a nonzero delta. Once the threshold is crossed, stationary paint,
animation and replay frames therefore still qualify as drags.

Baseline `Editor::drag` always stages an update. Core `Session::update_gesture`
validates the staged graph and mints a fresh preview revision, even if the desired
transform/path/crop is unchanged. `schedule_render` sees that revision, submits a
new complete exact viewport, clears the completed mailbox and changes upload
desire. The worker's cancellation closure observes the new serial. This can prevent
a render from publishing while the pointer is held still; real motion need not be
continuing. It is a source-established starvation route, not a reproduced runtime
result from this worker.

The renderer worker's editor fix now compares the proposed transform, exact path
elements/fill, ordered crop operations or clamped opacity to the current preview
before staging. No-op calls retain revision/history. Three new regressions cover
static versus changed motion, one undo/redo, shape/crop cancellation, original
source pointers, and repeated opacity. The UI owner separately gates actual motion
and separates consumed input from paint. Both layers matter: editor correctness
must survive any future caller, while paint should not repeatedly invoke commands.

Baseline evidence: `app.rs::draw/process_motion/tick/canvas_input/schedule_render`;
`editor.rs::drag/opacity`; `session.rs::update_gesture/stage`; pinned MUI
`mui-input/src/lib.rs::Interaction::update_with/get` at c33c4451.

## Other concrete costs and limits

- **Every changed camera requests a full viewport.** Wheel pan/zoom updates ViewState
  and runs tick immediately; hand-pan changes the center during held input. Request
  identity includes that affine, so each changed view replaces evaluation. Cursor
  motion is already coalesced into one latest pointer, but redraw still adds work;
  wheel events are not similarly coalesced. A single pending job bounds memory,
  not progress under frequent replacement. Tiny-skia fill is checked before/after,
  not interruptible inside its bounded 8× coverage call.
- **All pixels are evaluated/uploaded again.** `render_impl` allocates the dense
  output, preflights dependencies, evaluates every color tile and writes every
  pixel. There is no retained output-tile or coverage cache. Every accepted F32
  frame uploads 16 bytes per viewport pixel in padded bands. Uploader alternates
  two buffers but waits for each band's GPU submission before proceeding; the
  second buffer currently does not overlap two submitted copies. These are worker
  costs, not UI-thread decoding. Quantify them before relaxing staging ownership.
- **Repeated UI work is real.** A motion-bearing draw can resolve four MUI frames
  through two ticks. Each tree also rebuilds a complete layer-order vector and
  traverses the page; only 14 visible layer rows are shaped, which is already a
  useful bound. Every presentation resets the Hybrid scene and paints chrome.
  Cache stable layer order/chrome inputs separately from transform previews; measure
  layout and paint before attributing latency to the toolkit.
- **The accepted image uses the old mapping.** CanvasSet correctly retains the
  old valid pair while a candidate is pending, but `append_canvas` fits those old
  pixels into the current viewport, without old-request→new-request compensation.
  Artwork can remain visually fixed while selection/rulers follow the new camera.
  Track desired versus presented camera and associate hit testing/selection with
  what is actually displayed. Avoid relabeling an old frame as the new exact result.
- **Conversion is already mostly lazy.** `Host::compose` converts only a private
  candidate; ordinary chrome redraws reuse the current RGBA8 display texture.
  Count conversions to verify this at runtime. `present` also polls callbacks,
  acquires a surface image, encodes/validates/submits Hybrid work and presents on
  the UI thread. Surface acquisition, callback destruction, encode or driver
  submission may have material wall cost; none is measured here.
- **Do not invent a scope-pop GPU fence.** Native wgpu 29's `pop_error_scope`
  returns `ready(scope.error)` after its error-sink lock. The current three
  `pollster::block_on` calls warrant wall measurement/lock analysis, but source
  does not establish a GPU-completion wait there.

## Full-resolution improvement order

1. Land no-op protection and input/paint separation, then measure static held input
   and continuous motion before further renderer changes.
2. Coalesce navigation intent per presentation deadline; keep one active exact
   render and one latest pending intent rather than canceling blindly on every
   event. Preserve stale-result checks and metadata. A cadence/settle deadline must
   ensure progress and immediate final scheduling; a debounce alone cannot provide
   continuous artwork feedback.
3. Add retained full-resolution view coverage/tiles and dirty-region evaluation only
   after identifying the dominant work. Semantic cache identity must include source,
   operations, mask dependencies, isolation/order, request mapping/precision and
   coverage policy. Keep unchanged tiles shared and budgeted; update/copy only dirty
   tiles, and publish a revision atomically. No hidden preview decimation.
4. For camera-only translation, a possible immediate display mapping is
   `desired_affine * accepted_affine.inverse()` on the old available region. Raw
   F32 must be recomposed against the new checker/artboard; transforming baked
   opaque RGBA8 also transforms its checker. Revealed strips require genuine
   coverage availability and newly evaluated full-resolution pixels. Shifted
   coverage needs cached/uncached oracles before being called exact.

Magnifying the old viewport for zoom is a display sample of old evaluation, not an
exact new source evaluation, and can pixelate. That conflicts with this user's
explicit preference. Use pixels evaluated at the required density with bounded
overscan/tiles, or retain a coherent old presented camera until exact replacement.
Do not advertise smoothing of magnified pixels as faithful zoom. A preview→commit
metadata reuse could avoid an identical final re-render, but requires a proved
content-identity transition; skipping solely because the gesture ended is unsafe.

## Bounded instrumentation authored for root integration

With `AFFINT_DIAGNOSTICS=1`, RenderResult contains queued/start/finish Instants;
RenderWorker exposes scalar counts for submitted, pending replacements, started,
finished, produced/canceled/failed, published/rejected completions, taken/replaced
results, stop drops and current mailbox state. Counts distinguish canceled work
from a successfully produced frame rejected as stale. Before saturation:

`submitted = pending_replaced + pending_stop_drops + started + pending_count`

`started = finished + active_count`

`finished = produced + canceled + failed = published + completion_rejected`

`published = taken + result_replaced + result_stop_drops + result_pending`

Host exposes latest CPU wall timings for polling, acquisition, compose, GPU-check
overhead, conversion encoding, canvas/chrome/Hybrid encoding, submission and
surface present, plus sampled and accepted keys. Times overlap by nesting:
compose includes its encode stages; do not sum both. Normal operation records no
Instants or trace/file I/O. Fixed counters and the latest timing do not retain
pixel owners or an unbounded render history.

The UI owner pairs consumed-input {id, Instant} with serial and accepted key, records
tree/frame count/wall time, and keeps bounded metadata for render→upload→publication
age. Event-delivery-to-present-return and exact-canvas age are separate metrics.
Neither timestamp proves hardware-to-photon latency or compositor scanout.

## Exact proposed coordinator probes

After the coherent app batch is frozen, root formats under its lane and runs
`scripts/verify --label native-interaction-01 --build-app --jobs 4`. All existing
renderer goldens must remain unchanged; three editor regressions are additional.

Launch the owned native window with `AFFINT_DIAGNOSTICS=1 target/debug/affint-app`
after the root-owned diagnostic output path is integrated. Record actual logical
viewport, DPI, physical render dimensions, adapter/backend/driver and build hash.
Root's deterministic input/compositor injector should run:

1. **Static-held starvation oracle:** baseline idle, press a real editable layer,
   send a 500ms movement sequence, hold the final coordinates another 500ms, release.
   During the hold, revision/render-submit counts must stop increasing, the current
   exact job must be able to publish, and release commits one undoable edit. Undo,
   redo and cancellation preserve originals. Repeat for shape/crop and opacity.
2. **Navigation/chrome separation:** pan and zoom bursts, then settle; separately
   hover toolbar/menu/selection without document/view change. Record cancellation,
   pending replacement and completion ratios, exact result age, tree/frame counts,
   paint/acquire/submit costs and conversion count. Chrome-only work must not submit
   a document render or repeatedly convert the accepted canvas.
3. **Size/workload scaling:** repeat the same interactions at 1024×720 and 1440×940
   logical window sizes at the actual DPI, then on one original 24MP raster and on
   an editable-vector/mask fixture. Correlate renderer durations, upload bands/bytes
   and present stages. Keep source/output resolution and format explicit. Capture
   p50/p95/max of the observed bounded samples; do not compare GPU encode-only
   time to CPU end-to-end time or invent a profile when instrumentation is absent.

Existing release workload artifacts describe different headless jobs. They help
select representative probes but do not establish this native interaction latency.

Primary code references: [pinned MUI interaction flags](https://github.com/Matari-Audio/MUI/blob/c33c4451e8814562997171a1e00e9fb41fa955e6/crates/mui-input/src/lib.rs),
[pinned native wgpu scope handling](https://github.com/gfx-rs/wgpu/blob/e99f5305ded96ff7006f0714d043a7f735bd45c2/wgpu/src/backend/wgpu_core.rs#L1847).
