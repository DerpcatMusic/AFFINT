# Reference improvement 01: conservative support and full rectangles

Status: coordinator verification `reference-improvement-01` passed formatting,
9 new unit tests, 21 unchanged integration tests, one doctest, and all-target Clippy
with warnings denied. The coordinator then completed all four same-argument release
comparisons with status `ok` and no timeout. No worker build, test, profile, or
benchmark has been run. The baseline and exact commands are in [reference-baseline.md](reference-baseline.md). Production API,
source bytes, requested dimensions, f32 premultiplied linear composition, output
conversion, and coverage precision remain unchanged.

## Evidence and scope

The release baseline's identical 64-curve scenes rendered at 512×512 in p50
0.303433183 s when visible and 0.311809807 s when wholly outside the view. Source
inspection confirms every visible vector previously generated/reduced 8× masks
for every color tile. Raster nodes also visited every destination center before
rejecting individual outside-source samples. These are demonstrably unnecessary
operations in controlled scenes, although no call-stack attribution was available.
The 6000×4000 and 8000×6000 photo workflows took p50 3.048554139 s and 6.145321754 s;
those complete-workflow timings motivate testing coverage savings without assigning
cost to a particular loop.

This increment rejects only proven empty raster/vector leaves and skips scanning
only proven full axis-aligned page/crop rectangles. Groups still evaluate their
children in the existing order and isolation scope. Curves still use the existing
1/1024-output-pixel global Kurbo geometry and 8× canonical mask rasterization at
boundaries. No backend replacement, resolution/quality mode, persistent cache,
new dependency, scheduler, parallelism, manual SIMD, or unsafe code is introduced.

## Measured comparison

All numbers below are complete render-call observations from one process run per
workload, on the same machine/settings and unchanged procedural workload source.
The coordinator used the same warmups, samples, exact dimensions, and limits as
the baseline; retained artifacts are `.research/benchmarks/reference-<workload>-02/`.
The after-build/source hashes and release environment are recorded in
`.research/benchmarks/reference-improvement-01/environment.json`.
The measured binary SHA256 is
`46b52216c94121f7f894ac95a38e30365f49472597913c133d4216391db09a5d`.

| Workload | Baseline p50 / p95 s | Improvement p50 / p95 s | Warmups / samples | Baseline / improvement child peak RSS KiB |
| --- | --- | --- | --- | --- |
| Exact 24MP `photo24` | 3.048554139 / 3.072478987 | 2.693647770 / 2.718140905 | 1 / 7 | 293440 / 293388 |
| Exact 48MP `photo48` | 6.145321754 / 6.149978547 | 5.381134423 / 5.433615633 | 1 / 3 | 575160 / 574160 |
| Visible 64 curves `vectors` | 0.303433183 / 0.447048570 | 0.067053664 / 0.068094014 | 2 / 31 | 16836 / 23124 |
| Same 64 curves `offscreen` | 0.311809807 / 0.473859344 | 0.001490928 / 0.001613021 | 2 / 31 | 16576 / 23036 |

The controlled offscreen scene now avoids leaf coverage evaluation, while still
performing complete preflight, output allocation, page coverage proof, and output
conversion. Visible curves avoid masks for unrelated tiles. The full identity page
clip is proven full in the photo cases; their mildly sheared crop/mask placements
remain scanned. These source-backed changes explain which work is eliminated, but
no function-level cost attribution or recovered call stack is claimed.

Photo peak render admission is unchanged at 104757312 / 200757312 bytes. Vector
peak admission rises from 12107360 to 12110944 bytes: the 64-record support table
adds 3584 bytes while conservative scratch admission remains intact. Source bytes
and output dimensions/lengths are identical to the baseline. Held frame admission
was exactly 96000000 / 192000000 bytes for photos and 1048576 bytes for vectors;
every released render admission was zero.

Small-scene child peak RSS increased by approximately 6 MiB, despite only 3584
additional exact support-owner bytes. Linux child-rusage covers the entire
fork/exec lifetime and may include an inherited launcher floor; it is not evidence
of a 6 MiB support-table allocation or an allocator/function attribution. No measured
RSS reduction is claimed. Background desktop/Pi work was uncontrolled, and p95
with seven/three samples is simply the observed maximum. These results establish
local workload improvements with preserved precision, not interactive latency,
GPU performance, a production tail guarantee, or comparative product leadership.

## Support proofs

Raster rejection maps the rectangle of destination sample centers through the same
inverse affine as actual sampling. Affine component extrema occur at its four
corners; conservative f64 multiply/add roundoff guards enlarge those intervals.
Reject only when one complete interval lies strictly outside source-space
`(-0.5, width+0.5) × (-0.5, height+0.5)`. This includes bilinear contribution from
the transparent border. It is not an image-edge or forward-bbox approximation.
Nonfinite/uncertain bounds continue through ordinary evaluation.

Vector support is collected during the existing complete preflight. Straight paths
contribute their placed points; curved paths contribute the exact global derived
line/move points emitted by preflight. A one-output-pixel guard exceeds accepted
f32 conversion and coverage edge quantization/AA support. Strict disjointness keeps
all near-edge cases on the rasterizer. If a vector is evaluated through multiple
normal/mask placements, its single record unions all observed hulls. This can
reduce rejection opportunities but cannot omit a contributing placement. Editable
Bézier paths remain intact and no additional derived path is retained.

Culling cannot silently bypass existing geometry conversion errors. A sufficient
numerical domain bounds every canonical-block mapping below 2^17 coverage samples
(f32 rounding at most 1/128 sample versus the existing 1/64 admission limit) and
excludes potentially underflowing arithmetic terms. Inputs outside that sufficient
domain use the existing evaluator and its structured refusal. Crops must also
satisfy this domain before an empty leaf can be skipped. Fill premultiplication
and opacity conversion are checked during preflight even for rejected leaves.

Mask dependencies remain separate from a target's support: even a rejected target
evaluates its target-local mask dependency, preserving its errors and cancellation.
An empty mask source contributes zero alpha/luminance before inversion, so an
inverted empty source preserves the target. Source normal visibility and unrelated
ancestor placement remain irrelevant, exactly as in the document/render contract.

## Full rectangle proof

Only affine placements with zero off-diagonal coefficients qualify. For every
complete canonical coverage block touched by a color tile, transform all four
rectangle corners with the identical f64 matrix composition and f32 conversion
used by the reference coverage builder. The resulting axis-aligned polygon's
edges must enclose the complete high-resolution block, including image boundaries.
Then every scanned mask byte would be exactly 255, so skipping multiplication by
one is exact. Negative scales/winding are supported by these edge extrema.

The proof uses complete canonical blocks, not the color tile's smaller extent;
17-, 256-, and 2048-pixel color tiles therefore cannot select different coverage
values. Fractional boundaries, a crop crossing any touched canonical block, and
uncertain numerical ranges still scan the existing mask. Rotated/sheared rectangle
bbox containment is insufficient polygon containment and is deliberately not a
fast path in this increment. The mildly sheared photo crop/mask placements still
scan; the identity page clip qualifies.

## Ownership, admission, and cancellation

A private identity-sorted `VectorSupports` table owns one bounded fallible buffer
and a shared renderer-budget lease. Each document vector has one fixed-size record,
not one record per dependency visit or output tile. Every layer-table walk and
existing flatten/dependency traversal keeps cooperative cancellation checkpoints.
The table is discarded before returning `Frame`; it retains no snapshot, source
pixel copy, or flattened path. Its actual allocation capacity is charged by the
same existing `Buffer` machinery as other owned buffers.

The complete dependency preflight, path/depth guards, original conservative
output/scratch atomic reservation, and global concurrent-frame budget remain.
The bounds allocation adds its independently held reservation; admission is not
reduced because a particular scene appears empty/full. Abstract evaluation work
charges reflect work actually performed, so rejecting a leaf legitimately avoids
its raster/coverage/operation loops. No guaranteed latency or memory saving follows
from the unchanged conservative scratch reservation. Frame ownership still holds
all output accounting until its borrowed-pixel owner drops.

## Regression gates

Existing external/analytic goldens and error budgets remain unchanged, including
all-256-channel source identity, premultiplied linear bilinear/source-over colors,
HDR exposure recovery, editable crop undo, analytic curve area/hole interiors,
independently finer full-extent coverage, exact color-tile agreement, and concurrent
frame accounting.

New tests build original fixtures with public `Session` transactions. A private
`cfg(test)` switch disables only these two optimizations while sharing the original
preflight, geometry, scan conversion, color/effect/mask evaluation, and output path.
It is not a public quality mode or a second formula implementation. Byte-for-byte
comparisons cover placed/nested groups, curves/subpixel strips, crop boundaries,
hidden target-local luminance masks, HDR recovery, mirrored/rotated views, and
17/256/2048 color tiles. Independent fixed pixels additionally cover bilinear red
border contribution across x=256, a rotated polygon's empty bbox corners, full
rectangle interiors, and zero-mask alpha/luminance inversion under affines.

Specific refusal/lifetime fixtures preserve invalid distant crop geometry,
unrepresentable offscreen fill/opacity, a visible-in-target mask's HDR overflow
when the target is rejected, post-admission cancellation, insufficient shared
budget, and complete release of bounds/scratch/output owners. An offscreen curve
succeeds within bounded abstract work while the forced-original evaluator refuses
its unnecessary coverage work; this establishes that the rejection path executes
without using a timing assertion.

## Coordinator verification and comparison

The coordinator first formats only the owned renderer package, then runs one
serialized verification batch:

```sh
flock -n .affint-build.lock cargo fmt -p affint-render
./scripts/verify --package affint-render --jobs 2 --label reference-improvement-01
flock -n .affint-build.lock cargo build --locked --release -p affint-render --example workloads -j 2
./scripts/measure-workload --label reference-photo24-02 --outer-seconds 660 -- --workload photo24 --samples 7 --warmups 1 --max-seconds 300
./scripts/measure-workload --label reference-vectors-02 --outer-seconds 660 -- --workload vectors --samples 31 --warmups 2 --max-seconds 300
./scripts/measure-workload --label reference-offscreen-02 --outer-seconds 660 -- --workload offscreen --samples 31 --warmups 2 --max-seconds 300
./scripts/measure-workload --label reference-photo48-02 --outer-seconds 660 -- --workload photo48 --samples 3 --warmups 1 --max-seconds 300
```

Retain exact binary/source/revision hashes and release environment next to results.
The runner already acquires the shared exclusive lane and has one child per process;
GNU time is absent. Linux child-rusage peak RSS includes fork/exec lifetime and a
possible launcher floor, and must remain separate from conservative admission and
actual output bytes. Other desktop/Pi workloads were uncontrolled in the baseline;
record them in subsequent runs and avoid attributing all variation to this patch.
Render timing includes the harness's deadline callback and differs from the native
worker's request-serial check. Repeated independent process runs would strengthen
any comparison; the first batch is a bounded local observation, not an SLA.

## Source improvement review

Support proofs and budgeted ownership are private to the renderer's spatial module;
color/coverage evaluation stays in the existing compositor. Fallible table lookup
and mask access propagate structured errors. Full rectangle coverage is a small
internal result enum rather than a fabricated mask allocation. Fill conversion is
one shared helper for both preflight and evaluation. No public interface or output
owner escape was added, leaving a future typed linear-output seam independent of
this optimization.

Primary implementation sources inspected locally are Kurbo 0.13.1
[`Affine` arithmetic](https://docs.rs/crate/kurbo/0.13.1/source/src/affine.rs),
[flattening](https://docs.rs/crate/kurbo/0.13.1/source/src/bezpath.rs), and tiny-skia
0.12.0's [AA scan conversion](https://docs.rs/crate/tiny-skia/0.12.0/source/src/scan/path_aa.rs)
and [line edges](https://docs.rs/crate/tiny-skia/0.12.0/source/src/edge.rs).
Palette's maintained integer sRGB decode LUT remains unchanged; no repeated-float-
decode optimization claim is made. Further coverage backend or linear display-output
work belongs to a separate measured/fidelity-gated increment.
