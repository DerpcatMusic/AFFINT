# Native interaction source review, round 1

Status: corrective source independently reviewed, 2026-10-12. This is an independent
read-only product-source review. Only this report was authored by the reviewer.
No build, test, formatter, benchmark, app, desktop helper or Affinity window was
run or manipulated. Installed-reference inputs were independently worded research
specifications; no restricted vendor implementation was read.

The coordinator reports that `native-interaction-02` passed 126 tests plus the
renderer doctest, Clippy and the native build, with the real GPU proof and smoke
also passing. Those results precede the corrective native-source batch and are
not checks executed by this reviewer.

## Remaining findings

### P1 — Desired-camera input and selection can disagree with retained artwork

Locations: `crates/affint-app/src/app.rs:441` (`canvas_input`) and `:1253`
(`WindowEvent::MouseWheel`); `crates/affint-app/src/app/chrome.rs:1082`
(selection mapping) and `:1117` (rulers); `crates/affint-app/src/gpu.rs:544`
(`compose`) and `:647` (`append_canvas`).

Pan/zoom immediately changes `ViewState`. Hit testing and selection use that
desired camera before the exact replacement is accepted. Presentation retains
the last valid canvas, but `append_canvas` maps its entire old request into the
current viewport without accounting for the old request affine. The visible
artwork and the point receiving a new edit can therefore differ while evaluation
or upload is pending. Resizing can stretch the retained old pixels as well.
Stale-result rejection preserves ownership/freshness; it does not make that old
display match the new interaction geometry.

The supplied `native-initial-1024.png` visibly offsets the selected Coral disc's
rectangle from the ellipse. The capture also contains the separate ruler layout
defect, so it is corroboration of disagreement, not an isolated camera benchmark.
The later `native-shape-committed.png` shows agreement after settling at its own
geometry; its name does not establish that a new rectangle was created.

This is a known unresolved acceptance risk, already analyzed in
`native-interaction-bottlenecks.md`. A correction needs a coherent presented-camera
policy or full-resolution retained coverage, with hit testing/selection tied to
the displayed mapping. Scaling a baked old preview and advertising it as exact
would not satisfy the owner's fidelity requirement.

### P2 — Wheel zoom does not follow the measured installed input contract

Locations: `crates/affint-app/src/app.rs:1444` (`logical_wheel`) and `:1272`
(wheel zoom branch). Reference:
`affinity-view-pipeline-spec.md`, “Input and camera organization.”

AFFINT multiplies a line delta by 40 and zooms by `exp(-wheel.y * 0.003)`.
A positive one-line event therefore produces approximately 1.1275 rather than
the source-established installed factor 1.25; the reverse is approximately 0.8869
rather than 0.8. A positive three-line event is approximately 1.4333, while the
installed three-notch event contract is 1.75. AFFINT also lacks that path's
three-notch event cap. Cursor anchoring is implemented, but the gain/event
arithmetic is not faithful to the specified reference.

This is a source-contract discrepancy, not a hardware-wheel runtime comparison.
The native device/DPI interpretation of scrolling remains explicitly unknown;
this finding does not assert an exact corresponding pan-distance discrepancy.

## Corrections identified in this review

### P1 — Keyboard opacity preview was canceled by delayed MUI edit brackets

Original location: `app/chrome.rs` opacity dispatch in `App::inspector`.
Pinned MUI locations: `mui/src/ui.rs::bracket_commands`, `keyed_edit`, and `commit`.

A Left key is delivered by the first input frame. The replay tree consumes the
key, starts an opacity gesture and previews 0.99. The replay frame then brackets
the consumed slider key with Begin/End for the next tree. The old inspector called
`begin_opacity` for that delayed Begin; this canceled the existing 0.99 preview.
The delayed End then committed an empty gesture. The next paint restored 1.0 and
created no undo entry. The pattern existed in the previous generic shell and
remained in this increment; it is not attributed exclusively to the new control.

The native owner's `OpacityDispatch` (`app/chrome.rs:447`) now adopts the delayed
Begin while its selected-node preview is live, applies the value, and commits once
on End. Host cancellation and page reset clear dispatch ownership. Source review
confirms the actual opacity control/Ui-frame fixture at `app/chrome.rs:2215`
exercises the failing two-frame input plus single-frame paint sequence, observes
the real delayed Begin/End pair, and checks one committed undo plus undo/redo and
Home/End. The pointer fixture at `:2242` separately checks changed motion,
stationary revision stability, one commit, and cancellation restoring revision,
history and dirty state. These tests were reviewed but not run by this reviewer.

### P1 — Vertical ruler expanded into the document region

Original location: `app/chrome.rs::rulers`, vertical-ruler sizing.

The original vertical ruler specified `width(RULER)` and `grow(1)` inside a row.
Its grow weight competed with the artwork and enlarged the ruler far beyond the
20 DIP source contract. Both supplied AFFINT captures visibly show the result.
The corrected vertical branch (`app/chrome.rs:720`) keeps a fixed 20 DIP width
without row grow. Its resolved-geometry test at `:2277` checks artwork origin and
available width/height at three document-container sizes. Source review confirms
that this exercises actual MUI layout rather than repeating a constant. It does
not qualify the entire shell at those sizes; the runtime minimum remains 1024 DIP
wide. Native confirmation of the corrected source remains necessary.

### P2 — Effects deletion controls shared an interaction identity

Original locations: `app/chrome.rs::inspector`, properties Remove layer button
and footer Delete icon.

Both live controls used `remove`; each queued Delete for one activation. Pinned
MUI's resolved surface map stores one geometry per key with last-insertion wins,
while responses and focus are also keyed by that identity. This made the two
controls share focus/interaction state and dispatch twice. The second document
deletion became a no-op after the first cleared selection; the finding does not
claim two layers were deleted. The real properties and footer controls now use
distinct constants at `app/chrome.rs:1501` and `:1558`; both cross the same command
queue seam. The Ui activation regression at `:2293` verifies separate resolved
geometry, one enabled Enter activation from either control, and no disabled
activation. Source review confirms this removes the identity collision.

The narrow Name label now reserves 35 DIP instead of wrapping into two lines.
File Quit and Ctrl/Cmd+Q (`app.rs:569`, `:1073`) route through the existing protected
replacement flow; pending native file work disables/blocks Quit. This source
review does not claim native protected-close verification.

## Ownership and timing observations

The editor's exact semantic comparisons prevent stationary move/shape/crop/opacity
updates from minting more previews. Fill validates every sRGB channel before
changing either preview or foreground, uses the document gesture seam only for
selected vectors, restores the foreground on cancellation, and leaves original
raster pixels shared. The inspected code does not introduce a full-pixel-buffer
clone for these changes.

Render scheduling retains one replaceable pending snapshot, one active job and
one result mailbox. Publication checks serial under the existing mailbox lock.
Counters distinguish canceled evaluation from successfully produced but rejected
completion. Profiling retains fixed metadata limits of 4,096 samples and 128
completed-render timestamp entries, with explicit drop counts and no pixel owners.
Serialization occurs after the event loop, using exclusive file creation.
Host timings describe CPU work and surface-present-return observations, not GPU
execution or scanout. Nested encode stages overlap compose and must not be summed
as independent intervals.

The first owned-native probe failed before `interaction-profile.json` was written.
`probe.json` records focus leaving the owned AFFINT window and a still-running
child. No latency distribution, static-held submission plateau, one-step native
undo, protected-close result or controlled-size qualification follows from that
probe. The later-size drift prevents treating the filename `native-initial-1024`
as a complete 1024-by-720 geometry qualification. The protected-close capture is
invalid unrelated-desktop evidence and was not inspected for this review.

## Remaining acceptance limits

Fresh native observation must qualify the corrected ruler, keyboard opacity,
real protected close, stationary holds and navigation. Source/build/GPU/smoke
checks do not establish comfortable native editing or 1:1 workflow acceptance.
The supplied reference itself marks Vector workspace association, tool/context
order, studio heights and active Color mode provisional. This review does not
convert those unknowns into parity claims or invent unrelated visual changes.

Real changed motion still submits a complete exact viewport and replaces the
prior serial (`app.rs:512`, `schedule_render`). The no-op fix removes stationary
churn, but does not establish continuous-motion render progress, tile reuse or a
render cadence. Their remaining cost is a measurement/architecture question, not
a new measured speedup or a justification for resolution reduction.
