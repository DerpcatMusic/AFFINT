# Faithful rendering contract

The initial reference renderer supports the document module's declared subset:
filled vector paths, original RGBA8 sRGB raster assets, groups, affine transforms,
local crops, exposure, alpha/luminance masks, layer opacity, and source-over.
Other color models, profiles, blend modes, filter kernels, typography and stroke
semantics are explicit future work, not silently approximated features.

## Evaluation

1. Read original source content into linear-sRGB floating-point premultiplied color.
2. Evaluate live operations in the layer's local coordinates, in stored order.
3. Evaluate any mask dependency in those local coordinates and apply coverage.
4. Apply layer opacity and affine placement; composite into its parent.
5. Convert to the requested display/export representation only at the final output.

Exposure multiplies linear RGB without prematurely clipping highlight information.
Cropping changes coverage and remains editable; it never deletes source samples.
Interpolating translucent pixels uses premultiplied linear color to avoid fringes.
Mask luminance has a declared linear-sRGB interpretation; alpha masks use coverage.
Mask-node visibility controls normal page painting and does not deactivate its
mask contribution. Its normal ancestors are not applied while it is a dependency.

## Resolution and precision

Every render request states output dimensions, region and precision. Source assets
remain at original resolution. Zoom/pan changes how artwork maps onto display pixels;
it does not authorize reducing source or effect-processing resolution. A reduced
render mode, if later requested explicitly, must be visibly named and must never
enter an exact export or an exact cache entry.

The CPU reference uses f32 linear premultiplied intermediates and exact-size output:
straight sRGB RGBA8 for explicit SDR export, or linear-premultiplied RGBA F32 for
native presentation. These are explicit precision choices, not a claim
of full HDR/ICC production support. Accelerated paths must match declared tolerances
against original fixtures, especially translucent edges, masks, transforms, crops,
exposure, large canvases and nested groups.

## Resource ownership and scheduling

Output buffers, temporary coverage, source pixels, cache residency and queued jobs
all have byte budgets. Reserve before allocation; use checked dimensions and fail
with an actionable error. Depth and total render work are bounded. A dense output
request must not bypass limits just because its source document is sparse.

Preparation and evaluation happen away from the UI thread. Results carry revision,
viewport and quality identity; stale results are discarded. GPU-resident source
tiles and explicit dependency invalidation are the intended optimized path. Avoid
routine full-frame readback followed by upload to another UI graphics device.

## Performance evidence

Reference hardware observed on this machine: AMD Ryzen 7 7800X3D; discrete AMD
Navi 23 (RX 6600 family, PCI 1002:73ff) and integrated Raphael graphics. Actual device,
driver, clock/power mode and output scale must be recorded per benchmark.

Benchmarks will cover: full-resolution 24MP/48MP photo edits, many editable vectors,
nested masks/groups, cold and warm cache, pan/zoom, and long undo/redo sequences.
Record timings and peak bytes. Initial unprofiled release baselines now exist under
.research/benchmarks with hardware/profile/scope metadata: 24MP CPU p50 about3.05s,
64curve512×512 about303ms, and the same curves entirely offscreen about312ms.
These identify improvement work; they are not interactive responsiveness, accelerated
performance or world-leading evidence. See docs/performance/reference-baseline.md
for harness semantics and the work log for completed observations.

The verified conservative improvement preserves the same source/output dimensions,
linear evaluation and coverage oracles. Observed p50 is about2.69s at24MP,
5.38s at48MP,67ms for64 visible curves and1.49ms for the offscreen curve fixture.
The distributions, byte accounting and measurement caveats are recorded in
docs/performance/reference-improvement-01.md. These remain CPU observations.

## Typed working output

ADR0007 implements explicit `SrgbRgba8Straight` export and
`LinearSrgbRgbaF32Premultiplied` native output. Format participates in exact
request equality. A frame owns one budgeted representation, exposes typed borrowed
pixels and refuses mismatched format access explicitly. F32 output preserves
the evaluator's linear-premultiplied highlights through native backdrop composition;
RGBA8 export retains lossy SDR conversion and exact requested size. Exactly
zero-alpha working pixels are canonical zero; RGBA8 output also zeros RGB when
positive working alpha rounds to zero in its final byte. This explicit correction
does not alter original asset bytes or editable operation parameters.

Acceptance preserves all existing goldens and checks direct floating results,
format-sensitive handoff, cancellation and mixed-format leases. White alpha32/255
at+2EV has an independently derived `[128/255,128/255,128/255,32/255]` working pixel;
its fixture tolerance is1e-7 absolute. Supported opaque +16/+32EV power-of-two
controls must remain65536/4294967296. Those bounded oracles are not a universal
arbitrary-HDR error guarantee. Actual document-to-GPU display proof must agree
within one8-bit display code after final checker composition. Coordinator
`linear-f32-native-05` passes all 39 renderer tests and the doctest, with the full
workspace build/lints also passing. Real GPU proof `linear-f32-runtime-02` verifies
exposure, subnormal-alpha highlights, masks, exact row-band uploads, the Hybrid
seam and actual retirement callback acknowledgement. Independent working error is
at most9.16e-8 in its finite fixtures, and display error is at most1/255. This does
not establish GPU document evaluation or HDR-monitor color management.
