# Recommended first GPU document increment

2026-10-12. Independent source review only; no product edits, builds, tests,
formatting or execution. Root reports `linear-f32-native-04` passes 39 renderer
tests and the doctest; native publication/retirement proof is still being repaired.
Complete that proof before switching the app to another output producer.

## Minimum useful scope

Implement a **single painted raster with an optional single raster mask** through
the real document/render interface. Evaluate original pixels on the GPU into
RGBA32Float; this is document evaluation, not the existing CPU-frame display pass.

Eligibility must inspect the entire requested page before work: exactly one visible
paint root, with raster content; each participating leaf has at most eight ordered
Crop/Exposure operations, finite CPU-validated parameters and opacity. An optional
mask source is a raster leaf with its own bounded operations/opacity and no mask.
Hidden normal roots remain allowed. A visible group/vector, another painted root,
vector mask, recursive mask or unproved numeric domain returns an explicit
**whole-request CPU requirement**. Do not mix partial GPU and CPU page results or
flatten a group to force eligibility. Existing features remain available through
the same exact-size CPU request.

Mask ancestry need not be restricted to root ownership. The actual evaluator calls
the source with the target placement, then applies `source.transform` once; normal
ancestors and source visibility are ignored during dependency evaluation. Source
sampling → its stored Crop/Exposure operations → its opacity produces mask RGBA.
Then alpha or clamped premultiplied linear luminance, optional inversion, target
coverage multiplication, target opacity and final page coverage follow. A normally
visible mask may also paint the page: if that creates a second painted root, use
CPU. Crops stay in stored order and affect sampled output coverage, not source taps.
Do not combine exposure chains across crops/masks or presume group equivalence.

This scope covers imported-photo placement, live light/crop/opacity and raster
masking. Isolated groups and vector masks are the next semantic extensions after
evidence, not prerequisites for the first bounded slice.

## Module seam and preparation

Keep rendering knowledge in `affint-render`; add one optional `gpu` module using
pinned wgpu 29.0.4 and the already-audited safe byte-casting dependency. No wrapper
crate, trait-based backend framework, app document evaluator or new geometry engine.
Root owns the dependency/feature/MSRV decision: the present render package claims
Rust 1.85 while wgpu 29 requires newer Rust; the actual workspace toolchain is newer.

A prospective `GpuRenderer::attempt(&mut self, &Snapshot, RenderRequest, cancel)`
runs on the fixed render worker and returns a checked `GpuFrame` or typed
`CpuRequired(reason)`; existing document errors remain errors. Inject the shared
device/queue, device generation and explicit admission settings. Do not create a
second device or instantiate another CPU renderer per fallback. The caller uses
its existing shared CPU renderer with the unchanged request when needed.

Extract only two shared private preparations from the reference implementation:
validated f64 sample coordinates/tap packets, and existing canonical 8× coverage.
The CPU path must call the same helpers and retain all 39 current oracles. Preserve
coverage blocks, work/depth/byte admission, geometry errors and cancellation. GPU
preparation borrows the snapshot; it never asks callers for pre-evaluated artwork
pixels. A job can own a cheap snapshot while queued, then release it after source
uploads/preparation finish and retain only result metadata/resources.

Use the 16-byte CPU tap packet from [gpu-affine-precision.md](gpu-affine-precision.md):
integer left/top and the reference's exact f32 fractions, including fractions that
round to one. Integer RGBA8 source loads, uploaded Palette decode/alpha tables,
premultiplication and horizontal/horizontal/vertical interpolation perform the
sample on the GPU. Main/mask use their respective target-local inverse placements.
Use current CPU byte coverage for every crop and page clip; `Full` means identity
coverage, not a fabricated full-image allocation. No global WGSL affine/f64 attempt.

Keep a small private leaf schedule, materializing operations in order through
separate sampled-input/write-only RGBA32Float scratch textures. A main tile, mask
tile and reusable temporary suffice; no whole-frame intermediate per operation.
The final requested-size working texture is dense and explicitly admitted. Do not
pretend that tiled scratch eliminates the dense output or retained old result.

## Residency and ownership

Each source entry owns a cheap `RasterAsset::clone`, its original-byte GPU texture,
and separate CPU-source/GPU-source accounting. The core's clone shares
`Arc<Vec<u8>>`; it does not clone pixels. Deduplicate by pinned pixel-allocation
identity plus dimensions/color policy/device generation. Pinning prevents pointer
reuse; AssetId alone is not a sufficient cross-document cache key.

**CPU retention needs capacity accounting.** Core already measures unique pixel
owners by `Vec::capacity()`, and a cache can keep that owner alive after its prior
Snapshot/Session drops. Public `RasterAsset` exposes only size/pixels; its owner
accessor is crate-private. A narrow read-only `allocation_bytes()` or equivalent
ownership descriptor is a prerequisite for correct cache accounting, without
exposing mutable pixels or an owned escape. Charge each cache-retained allocation
once, including evicted-but-submitted owners. Its shared presence in a live document
does not make the cache retention free. Module admission totals are not global RSS.

Use one fixed worker, two bounded upload/packet slots and no growing staging belt.
For example two 4MiB MAP_WRITE|COPY_SRC staging buffers can stream original row bands
and main/mask packets plus bounded coverage into COPY_DST GPU resources. Two 256²
packet pairs occupy 4MiB GPU payload; three working tiles per slot occupy 6MiB.
Charge CPU packet capacity, coverage/schedule buffers, LUTs, staging, status/readback,
source ownership, scratch, output and every in-flight reference before allocating.
Query actual alignment/limits and reuse a slot only after its copy/compute use ends.

Lease owners should be `Arc` resource records with **one charge per allocation**;
callbacks clone the record, not its lease. Register
`CommandEncoder::on_submitted_work_done` with captured owners **before submission**.
Pinned wgpu 29 stores deferred actions on the encoder/command buffer; `Queue::submit`
drains them and registers completion callbacks after submitting. Dropping an
unsubmitted encoder/command buffer drops the stored closure and its captured
owners without inventing completion. This avoids both early release and a separate
manual decrement that could release twice. Explicitly test successful submission,
no-submit abandonment, cancellation, eviction and later sampling.

Keep `GpuFrame` opaque: no owned raw texture escape or frame-to-CPU fabrication.
For later consumption, a renderer/frame-owned submission-use method supplies a
borrowed view inside an encoding closure, registers an Arc frame owner on that
encoder, and submits through the same context. Borrowing alone does not retain an
accounting lease inside a wgpu command buffer; every later sampling submission
needs the completion owner. Application callers must not clone/store raw handles
outside that seam. Generation changes, submission errors and device loss must
preserve the old accepted frame; stopping a worker is not proof of GPU completion.

## Precision and actual pinned APIs

Reuse `Adapter::get_texture_format_features`, `Device::limits/features`,
`create_texture/create_buffer/create_compute_pipeline`, `begin_compute_pass`,
`copy_buffer_to_texture/copy_buffer_to_buffer`, `map_buffer_on_submit`,
`on_submitted_work_done`, `Queue::submit`, and serviced `Device::poll`.
Rgba8Uint sampling and RGBA32Float sampled-input/write-only-storage usages must be
supported by the actual adapter/device. No optional float filtering/blending or
half storage; default adapter capabilities do not override requested device limits.

Qualification runs the **actual document kernel** against independent CPU F32
oracles, not another implementation of its shader formulas. Check packet bits,
transparent borders, encoded-byte controls, low-light/translucent interpolation,
Crop/Exposure ordering, hidden transformed alpha/luminance masks and opacity.
Keep opaque +16/+32EV power controls exact where admitted; preserve the CPU tiny-alpha
oracle through whole-request fallback. Record backend/driver/features and every
precision-policy version in qualification and cache identity. Passing a few probes
does not prove all allowed floating arithmetic: the source error/range admission
must also be reviewed before the app switch.

The earlier `2^-17` sampler error target is unproved. Establish a real sampler
bound, propagate it through each admitted gain/mask/opacity step, and reserve
backdrop/encoding error separately. High gains on fractional samples can exceed
the final budget and need CPU even though a power-of-two interior control passes.
Never substitute a universal one-code tolerance for that propagation.

Guard potential overflow/underflow before arithmetic; WGSL finite-math/FTZ rules
invalidate post-result finiteness as the sole check. Use a bounded GPU u32 status
and a 4-byte readback after the job for dynamic `NeedsCpu` guards. Both GPU status
and CPU readback buffers are charged. This production scalar readback and its
completion latency are disclosed; ordinary output pixels remain GPU resident.
Diagnostic float/display pixel readback exists only in named qualification/probes.
The candidate remains private until status, submission errors and metadata checks
succeed. A false-positive numeric refusal takes CPU, not an invented ColorRange.

## Paired measurement and next task

The existing `photo24/photo48` workload has two editable groups. Preserve it and
its baseline; do not flatten/relabel it as a first-slice GPU result. Add explicitly
named narrow 24MP/48MP fixtures sharing the existing original procedural pixel
generator, then run **the same new document** through CPU F32 and GPU evaluation.

Measure CPU coordinate generation, coverage preparation, source upload,
packet/coverage upload, command encoding, GPU completion/status wait, and end-to-end
result readiness. Report total p50/p95, source CPU/GPU retention, scratch/staging,
output/current/retired bytes, RSS separately, status traffic and actual uploads.
Cold samples evict sources and retire them before timing; warm samples retain only
admitted sources/pipelines. Coordinates still regenerate/upload for each request.
Preparation/upload/status costs belong in GPU end-to-end time; kernel timestamps
are supplementary and require actual optional timestamp support.

A main 24MP packet stream is 384MB; main+mask doubles it. At 48MP these become 768MB
and 1.536GB per render, before coverage. Fixed 256² tiling produces hundreds of
submissions/dispatches, and coverage remains CPU work. Warm source residency alone
does not remove this cost. Dense 48MP F32 output is 768MB; two 48MP original sources
add 384MB, already above 1GiB before scratch/staging. Native 320MiB canvas policy must
refuse that dense request; a headless benchmark may use an explicit larger budget,
not silently change native policy. No speedup or GPU-memory RSS guarantee exists.

Recommended next implementation task, after native retirement proof:

1. Add the narrow source ownership descriptor and shared private preparation seams,
   preserving all existing CPU oracles and formats.
2. Implement renderer-local resident originals, bounded sample/operation tiles,
   submission owners and scalar status through the narrow eligibility contract.
3. Add a headless real-document GPU proof plus paired narrow CPU/GPU workloads;
   root alone verifies correctness, no-submit/cancel/later-use ownership and costs.
4. Only after gates pass, connect the opaque working texture to the existing native
   candidate/retirement path without a CPU pixel round trip or duplicate F32 output.

Unresolved objections: admitted shader arithmetic still needs a sound bound;
capacity exposure needs core review; coordinate traffic may erase useful speedups;
native adoption must coordinate its canvas accounting with GPU result ownership;
device-loss/error retirement cannot be guessed. If the matched warm slice is slower,
keep CPU as default and investigate compact, independently bounded coordinate
packets before adding full groups/vector masks or a generic graph system.

Primary pinned source: wgpu 29.0.4 VCS
`e99f5305ded96ff7006f0714d043a7f735bd45c2`, especially
[`command_buffer_actions.rs`](https://github.com/gfx-rs/wgpu/blob/e99f5305ded96ff7006f0714d043a7f735bd45c2/wgpu/src/api/command_buffer_actions.rs),
[`command_encoder.rs`](https://github.com/gfx-rs/wgpu/blob/e99f5305ded96ff7006f0714d043a7f735bd45c2/wgpu/src/api/command_encoder.rs),
[`queue.rs`](https://github.com/gfx-rs/wgpu/blob/e99f5305ded96ff7006f0714d043a7f735bd45c2/wgpu/src/api/queue.rs).
CPU evidence: current `compositor.rs` mask/operation order, `budget.rs` drop order,
core `model.rs` RasterAsset sharing and `session.rs::live_pixel_bytes`; Palette,
Kurbo and WGSL primary pins are retained in the preceding precision report.
