# Work log

## 2026-10-12 — project created

Owner confirmed a unified vector/photo/layout editor, starting with a shared
document and canvas, in this new AFFINT repository. Established the product record,
domain language, clean-room/reuse rules, ownership rules, and one build coordinator.

Earlier source review reproduced PhotoCraft stale effect-cache rendering and
omitted selection-history memory accounting; FilmCraft deferred decode errors and
stale project-ID overwrite; and DeckCraft dirty-document close without prompting.
These inform the explicit correctness rules. Earlier scratch evidence disappeared
from /tmp during the session; new research and verification artifacts will be kept
persistently in this repository. The prior source pins and reviewed findings are
being reconstructed in docs/research before reuse decisions.

No AFFINT code has been compiled or benchmarked yet. Next: current UI/canvas/crate
research, behavioral Affinity specification, independent document-module design,
then a coordinated first implementation and one verification cycle.

Owner updated all worker models to GPT-6.1 Sol xhigh and explicitly authorized
unattended continued work until told to stop. Every major implementation is followed
by a bounded evidence-based improvement pass; SIMD/GPU optimization requires
correctness comparisons and representative measurements.

Current public Affinity non-destructive documentation was fetched. Black-box launch
showed Affinity windows, but the direct SVG-open attempt left no visible Affinity
window; no reference screenshot or manual workflow claim is established yet.
Public behavior research continues while the Wine launch path is investigated.

UI-stack research completed with verified source pins/checksums. Selected initial
eframe/egui-wgpu 0.36.2 with wgpu 30 for the exposed shared-device texture/callback
seam. GPUI's examined Linux public interface would require an unproven integration
or maintained modification; no toolkit speed claim justified selection.

Owner explicitly authorized deep decompilation. Started a separate Pi xhigh REA
worker with installed Ghidra/ILSpy, raw outputs isolated from implementors. This
updates the earlier behavioral-only research restriction; proprietary implementation
will not be copied into AFFINT.

The owner then requested evaluation of their own Matari UI before selecting the
shell. Found the clean local MUI checkout at c33c4451e8814562997171a1e00e9fb41fa955e6:
intrinsic layout, standalone winit preview, retained Vello Hybrid rendering,
canvas, focus/shortcuts and IME. Started a Pi xhigh source audit of the actual
integration seam. Its first turn failed before provider startup; a fresh delegated
round retries the same bounded audit. Egui selection is now provisional fallback.

The document worker delivered the first real module: immutable shared sources,
validated atomic transactions, fresh revision tokens, undo/redo, isolated gestures,
strict archives, and resource admission. The first verification found missing test
crate documentation; the next passed all 24 tests and identified a workspace lint
priority issue. Fixed those integration defects; verification continues under the
exclusive coordinator lock. A separate Codex xhigh implementor is writing the
toolkit-independent, tiled, linear-light reference compositor.

Document verification document-core-03 passed: formatting, all 24 tests (including
two properties), and all-target Clippy with warnings denied. No further routine
document test runs are needed until a new change or concern. Native persistence,
rendering, real canvas workflows, and accelerated performance remain work in progress.

Native storage increment native-storage-01 passed formatting, ten tests, and
all-target Clippy with warnings denied. It preserves supported editable content
and exact original bytes, deduplicates shared source blobs, refuses corruption and
unknown schemas/fields, caps serialized inputs before source allocation, and uses
same-directory staging with sync/replacement. Failed writes/renames preserve the
prior destination. Save receipts name the saved revision and distinguish publication
from unconfirmed directory durability. Session history is not saved yet. See ADR0004.

MUI review completed and selected the owner's framework for the first shell:
public same-device external texture insertion exists in Hybrid, and MUI supplies
intrinsic layout, controls, input/IME handoff and native host reference code. No
framework fork or routine GPU readback is required. Use pinned MUI 0.3.2, Hybrid
0.2.0, wgpu 29.0.4 and registry winit 0.30.13 consistently. Egui remains fallback
if the actual integration proof fails. The source audit found image-fill stand-ins,
gallery clipboard limitations, and fractional-DPI wheel-unit inconsistency; those
must not enter exact artwork or AFFINT host behavior. Native app implementation
is delegated to Codex xhigh with original compact Affinity-style chrome.

First renderer verification compiled and passed 19 of 20 tests; the tile seam
fixture caught a real coverage discrepancy (alpha delta42), not merely invisible
RGB at zero alpha. The renderer worker is investigating rasterization consistency
before canvas acceptance. The output comparison has not been weakened. Cancellation,
linear compositing, source sampling, masks, crops, HDR exposure intermediates and
resource lifetime tests passed individually in that failing cycle; the renderer
increment as a whole is not verified yet.

PNG increment png-io-first passed all fourteen storage/PNG tests, formatting, and
all-target Clippy with warnings denied. Imported original pixels have bounded
allocation and explicit declared/assumed sRGB. Unsupported precision/profiles and
orientation/animation/HDR are refused, and deferred IEND checksum failure is caught
before success. Exact RGBA8 export checks finalization before atomic publication.
Independent synthetic PNG fixtures were encoded with Pillow 12.3.0, with sample
bytes and provenance recorded. This is a deliberately honest initial subset.

Coordinator advisory audit `cargo deny check advisories` reports `advisories ok`
for the current locked document/render/storage dependency graph. The first attempt
exposed a coordination flaw: a workspace glob included the app directory before
its manifest existed. Changed membership to explicit stable crates; root will add
the app when its initial target/manifest is ready. No concurrent builds or framework
edits occurred. App dependencies will need a separate audit once locked.

Renderer improvement completed and passed renderer-png-fidelity-04: 21 integration
tests, a doctest, formatting, and all-target Clippy with warnings denied, alongside
all fourteen storage tests. The tiny-skia curve clipping and coarse antialiasing
defects were addressed with bounded render-only Kurbo geometry, explicit 8× coverage,
and stable coverage blocks. Tests compare against finer full-span coverage and an
independent analytic quadratic-edge area; fully interior holes remain exactly zero.
Original editable curves and raster pixels stay intact. No speed claim is made.

The native MUI worker delivered a substantial editor slice with real commands,
bounded/cancelable rendering, native file and clipboard workers, dirty-close policy,
intrinsic chrome, and shared-device external canvas presentation. Root resolved the
pinned MUI graph (405 packages), kept wgpu29/Hybrid0.2 together, and began the first
coordinated app compilation. Development dependencies use optimizer level3 and
application code level1 to avoid known unoptimized layout/text costs; release
comparisons remain separate. A source-only benchmark harness and improvement plan is
being authored; root will execute it after the actual app proof.

Deep binary round 02 completed: 67 distinct native functions in 143 source-bound
slices. Its independently worded report describes shared-block clone versus copy
history paths, complete-payload deduplication checks, dirty work and cancellation,
and GPU lifetime carriers. End-to-end stale callback validation and fences remain
unknown; no vendor defect or runtime guarantee is claimed. Raw evidence stays
restricted; implementors consume docs/research/affinity-raster-tile-spec.md.

Native editor verification mui-native-first-04 passed formatting, all 13 tests,
all-target Clippy with warnings denied, and the normal executable build. Earlier
compile errors were corrected against the pinned wgpu29/SHA2 APIs. The improvement
pass addressed stale-result identity, checked render serials, fractional-DPI wheel
units, protected save-close durability, keyboard focus and second-frame clipboard
effects. Real MUI copy/cut/caret workflow has a regression test.

Built-binary smoke passed real exposure, crop, mask, opacity, move, reorder,
undo/redo, native save/reopen and exact PNG export. Original source bytes and
reopened rendered pixels remained equal; output is 960×720. The first GPU proof
passed half-float-to-display color tolerance, clip bounds and bound texture sample
checks, then failed its MUI chrome overlap assertion. This remains a failed proof,
not accepted integration evidence; the app worker is checking actual resolved
geometry and preserving diagnostics on failure.

The native app launched on AMD RX6600/RADV Vulkan and was inspected in an owned
1024×720 logical window at 150% scaling. Keyboard focus reached the layer-name
field; actual system copy/cut matched the synthetic name and paste restored it.
No clipboard contents from other applications were logged. Larger-window, menu,
modal and idle checks are continuing. The release workload harness is source-stable;
root will run it after the native display proof, with separate measured RSS.

Combined mui-native-workloads-05 verification passed the app's 13 tests, renderer's
21 integration tests and doctest, workload harness compilation/all-target Clippy,
formatting and executable build. The first GPU proof failure was its unanchored
intrinsic fixture, not observed shader/display ordering failure. Explicit resolved
chrome geometry and an interior overlap sample now make the check valid; failure
artifacts are persisted before assertions. The second proof passed RX6600/RADV
Vulkan with maximum display error1/255, bound sample preserved, no clip leak, and
MUI chrome above the canvas. Ordinary document evaluation remains CPU reference.

Native menu confirmation now shows compact intrinsic height below the File trigger.
The earlier oversized/misplaced menu was fixed with resolved trigger placement.
Actual keyboard close→Discard exited cleanly; first close attempt selected Save
because root's focus-count assumption was wrong and was stopped without a destination.
Focused settled idle observation recorded zero CPU ticks over five seconds; this is
a short process observation, not a responsiveness or sustained-performance result.
Native pointer input, file dialogs and IME composition remain unverified.

The expanded advisory check initially failed for unmaintained ttf-parser through
winit's optional Adwaita decorations. Removed that unused feature path, retaining
Wayland/X11/raw-handle support and the upstream Smithay fallback frame. The lock now
has398 packages and the second cargo-deny advisory check reports advisories ok.
No advisory exception was added. Platform/CSD limits are recorded in ADR0005.

Deep round03 completed41 distinct functions and91 source-equal slices in six
sequential analyses. It bounds concrete retained byte/float display buffers and
later upload, but still does not establish revision freshness, notification ordering
or application completion fences. Editable mask investigation was deferred; root
started a separately bounded Pi xhigh round04 for that specific consumer contract.
Sanitized report: docs/research/affinity-result-mask-spec.md. No raw vendor source
is consumed by implementors. Downloaded open-source archives/extracted trees stay
on disk but are excluded from the implementation Git checkpoint; pins/checksums
and review reports retain their provenance.

Release workload build has begun serially after the native app exited. Hardware,
toolchain, lock checksum, release debug/profile settings, CPU policy and background
work are recorded with the artifacts. No measured result or speed claim exists yet.

First unprofiled exact24MP CPU workload completed seven samples after a separate
admission probe and warmup: p50 3.048554139s, observed p95/max3.072478987s,
peak process RSS293440KiB. It rendered6000×4000, retained192000000 source bytes,
produced96000000 output bytes, admitted104757312 peak render bytes, and released
all render admission after each frame. Process RSS is separate from these module
counts. Small-sample p95 is not a reliable tail guarantee; Pi research/other desktop
work was uncontrolled. This baseline is not interactive or accelerated performance.
Visible/offscreen vector and transaction scaling measurements are next.
GNU /usr/bin/time was absent; that invocation started no benchmark. The coordinator
uses scripts/measure-workload with Linux child rusage and an outer timeout instead.

Local checkpoint3d810bf contains verified foundations, native editor, harness and
sanitized research. The global Git hook rejected committing on main; root created
feature/affint-native-foundation and committed there without changing the hook.
Downloaded evidence retains original whitespace/hashes; implementation-only diff
checks exclude the evidence trees. Ongoing round04 research was excluded from this
checkpoint and remains separate until stable.

31-sample reference measurements completed for vectors/offscreen and edit scaling.
Visible64curve512×512 p50 .303433183s, observed p95 .447048570s;
the same curves entirely outside the view p50 .311809807s, p95 .473859344s.
This controlled workload supports eliminating noncontributing evaluation; it does
not attribute time to particular functions. Transform/reorder p50 values for
100/1000/10000 leaf fixtures were .012810/.013061ms, .469369/.327767ms,
and5.417890/5.308648ms respectively. Sibling fanout is bounded32, so these do not
measure wide-list reorder. All statuses passed and released render leases.

Linux perf cycles:user recording was refused by installed privilege policy; root
did not modify kernel settings. No call-stack profile was recovered; Valgrind is
absent. Renderer worker is implementing a bounded conservative rejection/coverage
improvement with existing fidelity oracles unchanged. A separate source-only app
review examines linear working output versus the current RGBA8 display handoff.
The48MP baseline is now running with exact8000×6000 output and explicit pixel
admission. No source resolution is reduced.

Deep mask round04 is now stable:14 distinct functions,13 completed decompilations
and one assembly-only timeout,69 source-equal mapped slices, six sequential attempts
and unchanged original hashes. The sanitized spec bounds editable mask content,
bitmap transparency input, sampler setup/scanline consumption and ownership.
Alpha-versus-luminance conversion, complete transforms/cache identity and byte/float
equivalence remain unknown; AFFINT's existing declared contracts are unchanged.
Root acknowledged the terminal task and delegated a bounded source-only Pi audit of
small accurate GPU reuse candidates rather than expanding unspecified vendor probes.

Exact48MP baseline completed three samples after probe/warmup: p50 6.145321754s,
observed p95/max6.149978547s, process peak575160KiB,384000000 source bytes,
192000000 output bytes and200757312 peak render admission. Output is8000×6000
with explicit48MP policy; released admission returned zero. These are unprofiled
CPU observations, not interactive or GPU performance. Linux child rusage can
include fork/exec startup and a launcher floor for small workloads; it is never
presented as allocator-only or per-module memory.

App source/math followup confirms an HDR-to-checker limitation at the declared
RGBA8 frame seam: original white alpha32/255 at +2EV retains premultiplied linear
128/255 internally, but its final RGBA8 clamps straightRGB before the UI checker.
On the .78sRGB checker, that route rounds to207 while unclipped linear composition
rounds to255. This48-code example is analytical/source evidence, not a GPU runtime
measurement. It motivates a typed linear-F32 native output with retained budget
ownership; PNG's explicitRGBA8 export remains separate. Root will coordinate that
increment after the current renderer improvement, avoiding cross-worker source edits.

Reference improvement01 passed9 new optimization tests, all21 existing integration
tests, a doctest, formatting and all-target Clippy. Matched release workloads now
give p50:24MP2.693647770s,48MP5.381134423s,64 visible curves67.053664ms,
and the same offscreen curves1.490928ms. Exact source/output dimensions,8×coverage,
f32composition and error budgets are unchanged. Photo latency fell about12%, visible
vector latency4.5×, offscreen209× in these observations; no general/world speed claim.
All output leases released. Small-child RSS is affected by the changed measurement
runner/possible pre-exec floor; no allocator memory improvement is asserted.
Source/build hashes, complete distributions and caveats are in retained artifacts
and docs/performance/reference-improvement-01.md. Built release executable is retained.

The Pi core quality review found no confirmed atomicity/ID/source-sharing defect,
but recommended two independent native fidelity probes. Root registered and ran
them: BOTH failed in native-fidelity-probes-01. Unknown checksummed crop geometry
members loaded and were discarded; a normal positive page-widthf64 changed oneULP
on native text decode (input bits0x3ffd2c606cb79d18,1.8233341452376575).
These are reproduced fidelity defects, not speculative concerns or checksum errors.
Root selected the pinned codec's float_roundtrip feature; documentworker now owns
strict geometry Serde adapters preserving wire shape plus bounded staged-payload
ownership hardening. Source originals remain shared/exact; broader native parameter
fidelity cannot be claimed until the new probes and full relevant checks pass.

Coordinated core/pages/fidelity increment is verified in `core-pages-fidelity-03`:
41 document tests, 16 storage tests, 30 renderer tests, 19 app tests and one
render doctest pass; formatting, all-target Clippy -D warnings and normal app build
pass. Round01 stopped on missing test-crate documentation; round02 revealed a
malformed-start Kurbo construction panic. The correction validates serialized
starts and full typed-path geometry before constructor/canonicalization; the
original failing oracle remains unchanged. Both original native fidelity regressions
now pass, including every case of the 4,096-value bit-exact width corpus.

Built-binary `pages-runtime-03` passes real edit/source/save/reopen/export smoke
and the added distinct-page/captured-export workflow at 960×720. Fresh diagnostic
GPU proof on RX6600/RADV/Vulkan remains within1/255 with correct clipping/chrome
ordering. Owned native observations at1024×720 and1440×940 logical units,150% DPI,
confirm Add page produces an empty canvas/list, PageUp/Down restores the correct
artwork, undo/redo removes/restores creation, File/View menus fit and protected
Discard exits0. Native pointer/file-dialog/IME/accessibility/platform coverage is
not established. All tests/builds/runtime proofs were serialized under the root
coordinator lock; workers ran none. Next increment is ADR0007 typed linear-F32
CPU output and native display, with explicit candidate/retirement/staging admission.

The first root run ended at04:17UTC when the model provider reported capacity.
The09:42UTC user continuation restored xhigh Codex implementors from existing
source; verified checkpointdcc0269 and its evidence remain intact. F32 renderer
source is now frozen with39 authored tests plus a doctest; native upload/display
integration remains in progress. No new passing build is claimed. A fresh Pi
precision-design task failed before research because OpenAI OAuth refresh returned
invalid_grant. Root acknowledged that failure and moved the bounded read-only
research to a Codex xhigh worker; no login/configuration changes were attempted.

The F32 increment passes coordinator `linear-f32-native-04`: 41 document, 16 storage,
39 renderer and 23 app tests, plus one renderer doctest, formatting, all-target
Clippy with warnings denied and the normal native build. Earlier checks caught an
incorrect test history call, pinned wgpu mapped-write API misuse and four diagnostic
iterator lint errors; their corrections preserved the fidelity assertions.
The dependency change adds bytemuck as a direct app dependency without changing
any locked package version, source or checksum.

Real RX6600/Vulkan proof `linear-f32-runtime-01` passes the existing color/clip/chrome
checks and the real translucent-exposure F32 upload/display/Hybrid seam within
one display code. The legacy handoff actually produces dark-cell207 while the F32
handoff produces255, matching the independent oracle. It then fails the final
publication retirement check: charged GPU pairs have not all released when the
diagnostic checks them. This remains a failed runtime proof. Subnormal, mask and
three-band cases and the new built-binary smoke have not yet run. The native worker
is checking completion ownership/synchronization without weakening the zero-owner
assertion. Root is separately building the already verified renderer's release
workload under the exclusive lane; no new measured float-output result exists yet.

The retirement failure was a diagnostic synchronization defect: pinned wgpu-core
drains callbacks under its queue lock and fires them afterward. A concurrent
`Device::poll(Wait)` can acknowledge the GPU fence while another thread still runs
the callback that drops charged owners. A small callback token now signals only
after owner Drop; diagnostics wait for those acknowledgements, while ordinary
retirement remains asynchronous. A deterministic channel-gated regression holds
an owner inside Drop, proves admission stays Pending, then checks real release.
No sleeps, tolerance relaxation, fake zeroing or forced release were introduced.

Coordinator `linear-f32-native-05` passes120 tests (41 document,16 storage,39 renderer,
24 app) plus doctest, formatting, all-target Clippy-Dwarnings and the normal build.
Built-binary `linear-f32-runtime-02` now passes every real GPU fixture: exposure,
tiny positive alpha with normal highlights, hidden target-local alpha/luminance/
inverted masks and isolated groups, exact1281×1100 three-band upload, Hybrid seam,
private publication, two-pair backpressure, stale rejection and callback retirement.
Finite-fixture working error is at most9.16e-8; display error is at most1/255.
Source pointers/bytes remain intact and CPU frame charges release after upload.
The ordinary pipeline still evaluates documents on CPU, with no pixel readback.
Native save/reopen/edit/export and multipage captured-export smoke also passes.

Owned native captures in `linear-f32-native-window-03` show1024×720,1440×940 and
the usable full-monitor2560×1406 logical window at150% scaling. Protected close
exits0. The keyboard action sequence did not produce its expected page state, so
these captures add no new page-navigation or pointer qualification; prior native
page evidence and the separate smoke retain their established scope. A full-monitor
capture still says Rendering exact viewport, so it is geometry/display evidence,
not completed-frame timing or proof of that specific newly requested frame.

Matched CPU release F32/RGBA8 runs and one explicit refusal are retained under
`linear-working-output-01` and its per-workload directories. Exact F32 p50 is
2.544343677s at24MP (384000000 output bytes) and5.054917420s at48MP (768000000).
Every successful frame releases all renderer charges. The48MP F32 run explicitly
admits1024MiB;512MiB refuses without changing dimensions/format or emitting pixels.
Same-binary RGBA8 observations are2.938565532s/5.880998030s. Background work and
unpaired small samples prevent a causal speedup/regression claim. Source/build
hashes, distributions, source/output/scratch bytes and RSS caveats are recorded in
docs/performance/linear-working-output-01.md. The locked advisory check passes
with398 dependencies and unchanged package pins; it is not a full license/security
audit. Next work is the bounded headless GPU document sampler plan, with numerical
gates and retained original-owner accounting before any native backend switch.

The owner rejected the generic shell and reported severe native canvas lag after
trying it. Earlier native display/geometry observations are technical evidence,
not product UI or interaction acceptance. Priority now changes to faithful1:1
installed-Affinity editor reconstruction and measured input/paint/render response;
the planned headless GPU sampler is deferred. The verified core/F32 checkpoint is
`a0ad784`, without a claim that its UI meets the brief.

Codex xhigh workers now separately own reference UI/chrome reconstruction,
interaction/no-op scheduling fixes, and deep installed binary/UI/pipeline research.
Pi's earlier OAuth failure remains unresolved; no configuration/auth changes were
made. The binary researcher inspects compiled managed UI resources and native
input/tile paths, publishing independently worded specs with explicit coverage.
Root successfully launched the installed app using the existing desktop launcher
environment, confirmed its150% DPI dark/monochrome chrome and retained settled
reference captures. An initial capture caught a workspace transition and is invalid;
only explicitly validated settled files govern implementation. Full decompilation,
resource listing, actual reading and inference remain distinct coverage categories.

A concrete source bug explains canceled drag work: replay/paint frames retain MUI's
held-dragged response even without motion, and the old editor allocated a new
preview revision each time. Source fixes gate actual input and semantic changes;
bounded opt-in render/presentation timings will quantify remaining costs. These
changes and the UI rebuild are not compiled or runtime-verified yet. Additional
focused PhotoCraft/VectorCraft source reviews target genuinely reusable algorithms,
masking/vector/caching implementations and license/precision/ownership fit.

The owner explicitly prohibited opening or manipulating installed Affinity windows.
Root acknowledged that it had opened them; all workers received the prohibition,
and AGENTS.md now makes it persistent. Affinity research proceeds only from owned
binaries, extracted resources and saved preferences. No later Affinity desktop
operation was performed. Sanitized source reports now cover UI dimensions/palette,
27 native camera/tile functions and the installed module inventory; inventory is
not a claim to have read the entire proprietary codebase. Fresh PhotoCraft and
VectorCraft reuse reviews pin their source and distinguish static hazards from
runtime reproductions. Both recommend narrow audited kernels over wholesale reuse.

The rebuilt native editing chrome uses the source dimensions, neutral palette,
20 DIP rulers, compact studio layout and licensed unmodified Adwaita fallback font.
Input and repaint are separate, unchanged drags retain preview revisions, and fill
wheel previews validate encoded input, preserve originals and commit one undoable
edit. Foreground selection for raster content does not alter artwork or mark a
saved document dirty. Bounded opt-in diagnostics retain metadata rather than pixels
and serialize only after the event loop exits.

Coordinator native-interaction-02 passed126 tests plus renderer doctest, Clippy,
formatting and native build; its built-binary GPU display/retirement proof and
save/edit/export smoke passed. The independent source review found inherited
keyboard-opacity bracket loss, duplicate Delete IDs and a retained-camera/input
mismatch. The first root-owned Wayland probe additionally exposed a vertical ruler
growing horizontally and a wrapped Name label. Its window geometry drifted and
protected-close step lost focus; the parent exception ended the child before any
profile JSON was produced. Those captures are defect evidence, not a controlled
size/latency/close qualification; the close capture shows another desktop and is
invalid. Installed Affinity windows were untouched by this probe.

The corrective batch fixes opacity gesture ownership, distinct Delete identities,
ruler sizing, Name layout and minimum500×640 support, and adds protected File Quit /
Ctrl+Q. Four new regressions exercise actual MUI input/replay/paint and layout
seams. A zero-height leaf in the geometry fixture initially produced no registered
surface; replacing it with the production auto-sized canvas preserved every exact
20 DIP assertion. Coordinator native-interaction-04 now passes130 tests plus
doctest, Clippy-Dwarnings, formatting and build. The first isolated Xvfb probe
failed before opening AFFINT because the inherited Wayland environment selected
that backend; the corrected probe unsets Wayland selectors and uses a private X11
display. Native-interaction-private-02 now exits0 through actual protected Quit /
Discard and writes842 bounded samples with zero drops. During190 stationary-held
presentations, submissions stay15 and exact serial15 becomes sampled; during80
chrome-hover presentations, submissions stay16 and conversions stay3. The real
rectangle appears, Undo removes it and Redo restores it. This is a private X11
physical1024×720 observation with an actual535×521 render request, not a Wayland
latency or supported logical1024×720 UI qualification. The apparent1.5 scaling is
inferred from chrome/request mapping and was not independently recorded by this
probe. Input wall p50/p95 are1.569/2.467ms and paint0.793/1.140ms; presentation
return p50/p95 are10.695/13.381ms, with a560.297ms initialization-inclusive maximum.
These are optimized-dev CPU wall samples on a small synthetic document, not GPU
completion, optical response, release workload or comparative speedup evidence.

The owner explicitly prioritized eliminating whole-viewport/shape reevaluation
on canvas and element movement. Source diagnosis confirms repeated preflight curve
flattening and coverage rasterization. A separate xhigh renderer implementor has
designed byte-admitted retained geometry and layer tiles, complete dependency
identity and independent uncached output oracles; implementation awaits root's
next coordinated increment. Default uncached export/reference stays available.
RasterAsset now exposes read-only allocation_bytes so retained owners can charge
spare capacity without pixel copies. Desired/presented camera alignment and exact
wheel event arithmetic remain known source findings for the next app increment.
No UI parity, arbitrary camera reuse, latency fix or world-fastest claim is made.

The combined retained renderer and native camera increment is now verified in
`retained-camera-03`: 41 document, 16 storage, 51 renderer and 45 app tests (153
total), plus the renderer doctest, formatting, all-target Clippy-Dwarnings and
normal native build pass. Earlier attempts corrected test API and diagnostic
compile errors without changing numerical assertions. Twelve public retention
regressions compare fresh uncached output across semantic edits, masks, ordering,
history, camera mapping and ownership pressure. Optional cached geometry is released
before mandatory work admission; active evicted owners remain charged. Original
raster capacity is interned once, and fill edits now share immutable path storage.

The native host uses the actual presented display plan for picking, rulers and
selection. It can translate retained F32 pixels at native density and independently
recompose destination checker/page; unsupported zoom stays coherent until exact
pixels arrive. Overscan is bounded, camera requests coalesce, and revision edits
retain strict cancellation. Eleven new camera/input regressions pass. General
GPU document evaluation and arbitrary shifted-curve equivalence remain unimplemented.

The same built binary passes real RX6600/Vulkan F32 color/upload/retirement proofs
and the new retained camera fixture, with an independent bit-exact raster crop and
encoded composition tolerance of one channel value. Native edit/save/reopen/export
and multipage smoke pass. Binary SHA256 is
`fc30db5babce9023c903eacf8a86a03309cb06c41dfa17588bba6314a38ebf2d`.
Isolated native probe `retained-camera-private-02` records actual scale factor1,
physical/logical1024×720 and933 bounded samples without drops. Held-drag and chrome
hover create no extra document submissions; the real drawing/undo/redo and protected
discard exit0. Pan includes native-density reuse but also pending evaluations;
pan/zoom settle to the final desired serial. No installed Affinity window is touched.

Release workloads complete seven samples each with a fresh bit-exact uncached
oracle outside every timed call. Moving one of64 curves at512×512 F32 gives
p50/p95 12.186/15.452ms and252 reused/4 evaluated tiles. Panning over an original
24MP photo at1024×768 F32 gives7.705/9.886ms and9 reused/3 evaluated tiles. This
is viewport output, not a full24MP render. Original photo capacity96,000,000 bytes
is charged once. Resource owners release correctly. The full distributions,
memory policies, build identity and limits are in
docs/performance/retained-work-01.md; no comparative speedup is asserted.

The owner's next instruction is to finish and publish as open source. Original
code, icons, documentation and synthetic artwork receive MIT; third-party font
and dependency licenses remain intact. Publication uses a clean current source
snapshot rather than the private research-bearing Git history. Raw proprietary
research, downloaded source mirrors, scraped pages, personal environment paths
and invalid captures are excluded. The public image is AFFINT's own original
synthetic document on the isolated display. GitHub CI runs the coordinated check.
This release finishes the verified current milestone, not complete Affinity/Adobe
feature or interface parity. Remaining product work is stated in the roadmap.
