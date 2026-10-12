# CPU reference and document baseline

The coordinator executed this release baseline on 2026-10-12 after the native-app
proof. Renderer/harness source corresponds to checkpoint
`3d810bf48daaf5825d56f17cb1d1b63760f0beff`. The worker authored the harness and
reviewed the retained results; only the coordinator builds, tests, or runs it.
No call-stack profile was recovered: Linux `perf cycles:u` was refused by installed
privilege policy and Valgrind is absent. No kernel setting or software installation
was changed. Timings establish complete-workload observations, not function-level
attribution or interactive/GPU performance.

The retained artifacts are `.research/benchmarks/reference-<workload>-01/` with
`workloads.csv`, `command.json`, `result.json`, and `process.log`. Hardware, toolchain,
release settings, revision, and checksums are in
`.research/benchmarks/reference-baseline/environment.json`.

| Render workload | Exact output | Warmups / samples | p50 | Observed p95 | Child peak RSS KiB | Peak render admission bytes |
| --- | --- | --- | --- | --- | --- | --- |
| `photo24` | 6000×4000 | 1 / 7 | 3.048554139 s | 3.072478987 s | 293440 | 104757312 |
| `photo48` | 8000×6000 | 1 / 3 | 6.145321754 s | 6.149978547 s | 575160 | 200757312 |
| `vectors` | 512×512, 64 curves | 2 / 31 | 0.303433183 s | 0.447048570 s | 16836 | 12107360 |
| `offscreen` | Same 64 curves outside view | 2 / 31 | 0.311809807 s | 0.473859344 s | 16576 | 12107360 |

All statuses were `ok`; every returned frame held exactly its output allocation,
and render admission returned to zero after dropping it. The two photo sources
retain 192000000 and 384000000 bytes respectively, and outputs own 96000000 and
192000000 bytes. Vector sources retain no raster bytes. Identical visible/offscreen
work establishes a concrete opportunity to reject noncontributing evaluation; the
similar timings do not identify which coverage/evaluation loop dominates.

| Requested raster leaves | Actual layers | Transform transaction p50 / p95 ms | Reorder transaction p50 / p95 ms |
| --- | --- | --- | --- |
| 100 | 105 | 0.012810 / 0.016290 | 0.013061 / 0.016431 |
| 1000 | 1033 | 0.469369 / 0.481609 | 0.327767 / 0.477929 |
| 10000 | 10323 | 5.417890 / 5.507192 | 5.308648 / 5.370249 |

These transaction rows used two warmups and 31 samples, and did not render. Reorder
fanout is at most 32, as explained below. They are outside the first renderer
optimization increment.

The machine was a Ryzen 7 7800X3D (8 cores, 16 threads), approximately 48 GB RAM,
performance governor, Linux 7.3.0-rc6 CachyOS, rustc/cargo 1.98.1 and LLVM 22.1.8.
Release uses opt-level 3, thin LTO, and one codegen unit; the environment records
the subsequent profiling build's `debug=line-tables-only` override. The native app
was stopped. Other desktop work and Pi static research were uncontrolled; no
comparative product/world-speed claim or reliable tail guarantee follows.

`crates/affint-render/examples/workloads.rs` uses existing dependencies and std,
original procedural artwork, and the real `Session`/`Renderer` interfaces. It emits
schema-versioned CSV to stdout and progress, profile identity, and failures to stderr.
Every render specifies exact source/output dimensions; no source pyramid, preview
resize, approximate mode, cache, thread pool, manual SIMD, or unsafe is introduced.

The available workloads are:

| Name | Measured operation | Fixture |
| --- | --- | --- |
| `photo24` | Exact 6000×4000 render | Two full-resolution RGBA8 sources, two isolated groups, mild affine placement, ordered exposure/crop, hidden luminance mask with its own exposure/crop/opacity, final group opacity. |
| `photo48` | Exact 8000×6000 render | Same workflow with 48MP originals/output. Opt-in; renderer `max_pixels` is explicitly 48 million and is recorded in CSV. Other precision/quality policies are identical. |
| `vectors` | Exact 512×512 render | 64 default filled cubic circles in fractional placements, adjustable from 1 through 256 layers. |
| `offscreen` | Exact 512×512 render | The same vector sources/count/placements translated entirely outside the view. Controlled comparison for conservative support rejection. |
| `edit100`, `edit1000`, `edit10000` | Separate `SetTransform` and `MoveLayer` transactions | 100/1000/10000 real raster leaves sharing one original 4×4 source, arranged in a fixed-fanout group tree. |

Fixture setup is recorded separately as `setup_ns`. Photo pixels are deterministic
integer gradients/patterns, including translucent samples; the mask has a different
procedural pattern. Circle paths come from Kurbo and remain editable document paths.
All raster insertion and edits cross the public transaction seam.

Edit fixtures have maximum sibling fanout 32 and fixed dependency depth three.
Two bounded batches establish the groups; all leaves are inserted in one batch.
The actual layer counts, including groups, are 105, 1033, and 10323; CSV also records
the requested leaf count and fanout. This avoids growing an unbounded flat sibling
list during fixture construction. One batch also avoids full document validation
per insertion. The baseline isolates whole-document transaction scaling: reorder
moves between the front/back of a list of at most 32 siblings, so it is NOT a
measurement of reordering a 10,000-item flat sibling list. That separate workload
should be added only with bounded setup or an efficient validated import fixture.

History retention is explicitly disabled (`max_history_entries=0`, metadata budget
zero), and no old snapshot is retained during edit timing. Thus increasing history
residency does not silently contaminate transaction scaling. The original asset is
still shared under all layers. This baseline does not measure undo/redo, gestures,
long retained-history/snapshot residency, concurrent renders, GPU presentation,
import/export, storage, or end-to-end input-to-display latency.

Timing and admission interpretation:

- Default is one warmup and seven measured samples per phase. `p50_ns` and `p95_ns`
  are nearest-rank order statistics of completed samples, with no interpolation.
  Seven-sample p95 is the observed maximum, not a reliable tail estimate or SLA.
  Larger studies should use repeated process runs and at least 30 samples where
  the workload/deadline budget permits.
- Render samples cover the public render call, including cooperative deadline
  polling and allocation/evaluation. Request construction, fixture creation,
  admission probing, output inspection, frame drop, and CSV I/O are outside those
  samples. `black_box` observes the returned value so output remains required.
- Transaction samples cover `Session::transact`, including staging, full validation,
  admission, publication, and history trimming. The concrete `Edit` is prepared
  before the timer. They do not time rendering or isolate validation from staging.
- A separate first render probes `Renderer::allocated_bytes()` at cancellation
  checkpoints. It is reported as `first_admission_probe_ns`, not folded into render
  quantiles. This instrumented first render also warms the scene; subsequent samples
  are repeated warm-process work. No cold-cache or effect-cache-hit claim is made.
- `render_peak_admitted_bytes` is the observed conservative global reservation, not
  allocator/RSS usage. `render_frame_admitted_bytes` is read while that sole frame
  remains alive; `render_released_admitted_bytes` must be zero after it drops.
  `frame_output_bytes` is the borrowed pixel length, with no detached pixel owner.
  Source and metadata admission are separately reported from `Session::resource_usage`;
  sources are not charged again by the renderer. These quantities must not be added
  and presented as measured RSS.
- Width/height/output/admission fields are zero for transaction-only rows because
  no render exists in those phases. For a failed initial render probe, output fields
  are zero and status is `probe_error`; any observed peak and released admission are
  still reported, and the exact refusal is on stderr. Empty timing
  fields mean no completed sample, never a fabricated zero-duration result.
- `status=ok` means the requested measured/warmup counts completed. Deadline/error
  rows retain truthful completed counts and partial statistics, and exit code two
  signals an incomplete workload. Configuration/setup/I/O failure exits one.

The deadline defaults to 120 seconds separately for the admission probe and measured
phase. Render cancellation checks `Instant` at the renderer's existing checkpoints;
this cost is part of these render timings, whereas the native worker checks request
serial atomics. That difference must be recorded when comparing them. Upstream
bounded coverage/flatten calls cannot be interrupted internally. Transactions check
the soft deadline between calls; one bounded transaction can overrun it. Fixture
construction is bounded by static dimensions, at most 256 vectors, and at most
10,000 leaves, but is not timed against that phase deadline. Use an outer process
timeout when a hard wall-time limit is required.

Sample count is limited to 64, warmups to eight, deadline to 3600 seconds, color tiles
to 1..2048, renderer bytes to at most 1024 MiB, and abstract render work to at most
one trillion units. Defaults retain renderer 512 MiB/20-billion-work admission.
Requests exceeding those finite limits fail honestly without reducing resolution.
The 48MP fixture owns two 192,000,000-byte sources before output/scratch; process
peak memory must be measured externally. Very small color tiles deliberately repeat
coverage block work and may be refused by work/deadline admission; do not choose
them as the default full-resolution workload merely to produce more samples.

Coordinator commands, from the AFFINT repository:

```sh
./scripts/verify --package affint-render --jobs 2 --label workload-harness
flock -n .affint-build.lock cargo build --locked --release -p affint-render --example workloads -j 2
./scripts/measure-workload --label reference-photo24-02 --outer-seconds 660 -- --workload photo24 --samples 7 --warmups 1 --max-seconds 300
./scripts/measure-workload --label reference-vectors-02 --outer-seconds 660 -- --workload vectors --samples 31 --warmups 2 --max-seconds 300
./scripts/measure-workload --label reference-offscreen-02 --outer-seconds 660 -- --workload offscreen --samples 31 --warmups 2 --max-seconds 300
./scripts/measure-workload --label reference-photo48-02 --outer-seconds 660 -- --workload photo48 --samples 3 --warmups 1 --max-seconds 300
```

The `-02` labels are fresh comparison artifacts; existing baseline directories must
not be overwritten. To repeat transaction rows, run each `edit100`, `edit1000`, and
`edit10000` separately with `--samples 31 --warmups 2 --max-seconds 300` and a fresh
label. The first renderer increment does not change transaction code.

Root owns formatting, all verification/builds, and every execution. The runner
acquires the same `.affint-build.lock` as `scripts/verify`, records the exact command
and release binary hash, enforces the outer process-group timeout, and creates a
new per-workload artifact directory. Do not wrap it in another lock acquisition.
Its default binary is `target/release/examples/workloads`; an alternative target
directory requires a deliberate runner adaptation by the coordinator.

GNU `/usr/bin/time` is absent and the attempted invocation started no measurement.
`scripts/measure-workload` uses Linux `resource.getrusage(RUSAGE_CHILDREN).ru_maxrss`,
with exactly one child per process. This peak covers the child's whole fork/exec
lifetime, including a possible inherited launcher floor for small scenes. It is
separate from exact pixel ownership and conservative renderer admission, and does
not attribute allocator bytes or memory to individual modules. Separate process
runs avoid allocator retention between workloads; repeated process runs are needed
to assess variation. Preserve Git/lock/source/binary hashes, toolchain and profile,
CPU policy/hardware, and concurrent workloads beside every comparison.

## Source-backed hypotheses for the first profile

These source-backed candidates remain distinct from measured attribution. The
baseline justifies eliminating demonstrably unnecessary offscreen evaluation and
investigating full coverage work. Installed profiling privileges did not permit
call-stack attribution. The first bounded implementation and its verification /
comparison status are recorded in [reference improvement 01](reference-improvement-01.md).
Other candidates require additional evidence before production changes.

| Candidate | Baseline source evidence | Evidence to seek | Correctness constraints for an eventual change |
| --- | --- | --- | --- |
| Repeated original-sample decode and mixing | `compositor::raster` always fetches four neighbours and mixes premultiplied colors, including aligned samples with zero interpolation weights. `source_pixel` reads original bytes and calls `Srgb<u8>::into_linear::<f32>()`. | Compare photo call-stack cost of sample lookup, LUT access, premultiplication/mixing, inverse mapping, and final conversion. Count actual neighbour reads, including transparent-border cases. | Exhaust all 256 encoded channel values/low alpha; identity alignment, transparent colored edges/border, subpixel affines, masks, and HDR exposure recovery. Identity/zero-weight special cases must agree exactly and never change original pixels. |
| Final transfer encoding | `write_tile` calls `Srgb::<f32>::from_linear` then integer conversion; Palette's same-type transfer uses `powf`. | Measure inclusive/exclusive output conversion cost separately from raster reads and mask coverage; characterize actual data-dependent branch mix. | Preserve declared analytic float encoding and current external/golden comparisons. Palette's faster direct f32-to-u8 path documents up to 0.6 output-code error; it is not automatically a valid reference replacement. |
| Offscreen/opaque coverage work | At the baseline checkpoint, every visible node allocates/evaluates a tile; raster sampling still visits all centers before rejecting outside samples. Vector/crop/page masks allocate and reduce explicit 8× coverage even where contribution is zero or full. | Compare `vectors`/`offscreen` with identical counts; profile allocation, zeroing, reduction, and path scanning. Compare full photo page/crop/mask costs. | Bounds must include bilinear transparent-border influence, vector AA/f32 guard, all child transforms, and target-local mask dependencies. An inverted mask never justifies skipping its source incorrectly. Fully covered page/crop fast paths require a conservative geometric proof and exact coverage, not a bbox guess for rotated polygons. |
| Repeated geometry/dependency preparation | Render preflight visits the child/mask DAG and counts flattened paths. `prepare_geometry` counts and emits global geometry per coverage evaluation; another color tile repeats it. No geometry/effect cache exists. | Attribute preflight/flatten/admission cost versus scan/reduction; count repeated node-plus-placement evaluations and derived elements. | Prepare once only for identical semantic inputs. Mask sources use target-local placement, not ordinary ancestors. Any retained derived owner must remain budgeted; frame/request revision, exact view/dimensions and reference quality policy belong in persistent cache identity. Prefer bounded per-request reuse before a persistent cache. |
| Full validation and sibling copy-on-write | `Session::stage` calls complete `validate` after staging; it walks tables/references/dependencies and checks composed transforms. Staging `link`/`unlink` clone an owner then `Arc::make_mut` its sibling vector. | Use bounded-fanout 100/1000/10000-leaf transform/reorder scaling, then distinguish staging from validation and metadata admission by a profiler. A separate wide-sibling case would isolate list costs. | Preserve atomic failures, composed-affine overflow refusal, tree-plus-mask cycle checks, stable identity across branching, opaque-source preservation, and outstanding-snapshot/history admission. Incremental validation must prove complete dependency invalidation before replacing the full oracle. |

Palette's decode path needs a specific correction to the initial hypothesis: the
existing renderer does NOT perform three float `powf` decodes per RGBA8 neighbour.
The generic RGB conversion dispatches to the maintained integer specialization,
which directly looks up `SRGB_U8_TO_F32`. Primary source:
[Palette 0.7.7 RGB conversion](https://docs.rs/crate/palette/0.7.7/source/src/rgb/rgb.rs),
[its sRGB transfer implementations](https://docs.rs/crate/palette/0.7.7/source/src/encoding/srgb.rs).
Local audited source is `.research/reference-crates/palette-0.7.7`, at
`src/rgb/rgb.rs::Rgb::into_linear` and
`src/encoding/srgb.rs::impl IntoLinear<f32, u8> for Srgb`.
Output same-type encoding is a separate implementation and still uses `powf`.

The coverage policy is deliberate fidelity work, not a speed tuning knob. Earlier
coordinator diagnostics measured clip-dependent cubic alpha changes of 42 codes;
fixed global geometry alone exposed non-convergence in tiny-skia's 4×4 sample lattice.
Explicit 8× coverage plus a 1/1024-output-pixel Kurbo geometry target agreed within
one alpha/linear-premultiplied code with independently finer full-extent geometry.
The regression and exact-byte color-tile consistency remain required. See
[ADR 0003](../adr/0003-reference-compositor.md),
[the reference fixtures](../../crates/affint-render/tests/reference.rs), and
[tiny-skia 0.12.0 supersampling source](https://docs.rs/crate/tiny-skia/0.12.0/source/src/scan/path_aa.rs).
An analytical/higher-quality maintained coverage backend may be evaluated later
against these originals and error budgets; reducing coverage/source/output quality
to improve a timing figure is not an optimization of this exact reference.

Every optimization must rerun meaningful fidelity/ownership regressions and the
same recorded workloads, then report latency distribution, process peak RSS,
admitted bytes, and any uploads/readbacks for the actual workflow. Do not infer UI responsiveness, GPU performance,
world-leading speed, or a production tail guarantee from this CPU harness.
