# First GPU document sampler: coordinate and numeric admission

2026-10-12. **Design/source research only. No implementation, compilation, tests,
product/probe execution, or performance measurement.** Renderer acceptance files
remain frozen. This note proposes gates; it does not authorize implementation or
claim that the current native upload path evaluates documents on the GPU.

## Smallest concrete slice

Use the existing wgpu 29 device, original integer RGBA8 source textures, explicit
four-tap sampling, and separate RGBA32Float input/output tiles. Keep current CPU
8× coverage, Kurbo flattening, and Hybrid presentation. Start with **CPU-generated
coordinate packets**, then consider a cheaper rebased affine packet only after
its independent error bound passes. This admits ordinary 6000×4000 and 8000×6000
photos by actual bytes/device limits, rather than excluding them by size alone.
It trades CPU coordinate work and upload bandwidth for a smaller fidelity proof;
there is no speedup evidence. Reuse decisions remain those in [gpu-reuse.md](gpu-reuse.md).

## Reference and arithmetic hazards

The CPU evaluates `I * (output_x + .5, output_y + .5)` in f64 using Kurbo's inverse
placement, subtracts `.5`, checks the transparent-border domain, then floors and
casts fractions to f32. Four original taps are decoded by Palette's 256-entry f32
sRGB table and premultiplied before horizontal/horizontal/vertical interpolation.
Palette uses `x + factor * (y - x)`; WGSL's `mix` has another inherited expression.
Keep crop, exposure, target-local hidden alpha/luminance masks, opacity, isolated
children and parent source-over in the existing order. Crop affects sampled
coverage, not source-tap eligibility. [A, P, K]

WGSL permits subnormal flushing, reassociation and fusion, and does not specify a
rounding direction. Runtime overflow can produce an indeterminate value under its
finite-math assumption: a later finiteness check is insufficient. Its basic
addition/multiplication accuracy is correctly rounded, subject to those rules.
Neither WGSL `fma` nor separate `let` statements promise CPU rounding. Concrete
shader floats are f32/f16; host f64 does not become a portable WGSL f64 feature.
[WGSL 15.7.2–5](https://www.w3.org/TR/2026/CRD-WGSL-20260921/#floating-point-evaluation),
[types](https://www.w3.org/TR/2026/CRD-WGSL-20260921/#floating-point-types).

Consequently CPU-valid white alpha 2^-130 followed by four +32EV operations must
retain RGB 0.25 and positive subnormal alpha, or take the exact CPU path. A pure
ordinary-f32 kernel cannot silently replace that result with zero. This differs
from the native display pass receiving trusted, already-evaluated working RGB.
Half storage is also unsuitable: opaque +16EV is 65536, above 65504.

## Initial coordinate packet

For each raster evaluation and output tile, a future shared private CPU coordinate
function produces `{ left: i32, top: i32, fx: f32, fy: f32 }`, exactly 16 payload
bytes. It must use the actual reference evaluation, including its border test and
fraction cast, rather than recomputing an ideal affine expression. Fractions may
round to 1.0; retain that result and its original left/top. An outside sample uses
an explicit `(-2,-2,0,0)` sentinel. Each tap gets an explicit integer bounds check
and transparent return; out-of-bounds texture behavior is not the border policy.
Source dimensions and in-domain floor coordinates must fit the packet integers.

By construction this removes GPU coordinate/floor/fraction disagreement, including
rotation, shear, half translations, large translations and badly conditioned but
CPU-admitted matrices. CPU geometry errors still propagate. The builder is not
implemented or executed. Source pixels are not read back or sampled on the CPU
by this proposed coordinate step.

A 256² packet tile is 1MiB. Charge CPU packet, GPU packet, staging, sources, coverage,
working tiles, output and pending owners; reuse only after their last use completes.
Do not allocate a dense coordinate image. Cumulative packet upload per raster is
384,000,000 bytes at 24MP or 768,000,000 at 48MP, excluding padding/other resources.
Record that traffic, CPU preparation and cold/warm source uploads separately.

Original `Rgba8Uint` texels give integer LUT indices. Upload the CPU's 256 decode
values and 256 alpha values (`f32(byte)/255`) in a read-only storage buffer,
2KiB total, avoiding hardware sRGB conversion, shader `pow`, and shader division
differences. Use explicit premultiply
and Palette-shaped interpolation, with an arithmetic tolerance rather than a
bit-equality promise. Query actual texture usages, dimensions and binding limits;
separate sampled float inputs/write-only storage outputs avoid optional float
filtering, blending and read-write storage requirements. [P, W]

## Later rebased packet, with a refusal gate

For tile origin `(ox,oy)`, compute in CPU f64
`q0 = I*(ox+.5,oy+.5) - (.5,.5)`. Split it into integer `base=floor(q0)` and
fraction `f0=q0-base`. Upload base, f0 and inverse linear coefficients. The shader
evaluates only `r=f0+a*i+c*j`, then integer `base+floor(r)` and fraction
`r-floor(r)`; never add the large source integer in float before extracting the
fraction. Bound integer addition and both axes separately.

For `0<=i,j<=T-1`, an admission bound must include

`Ecoord <= |delta f0| +(T-1)(|delta a|+|delta c|) + E32 + E64`.

Here deltas are actual host f32 cast errors, E32 encloses allowed expression
reassociations/rounding, and E64 encloses the difference from the CPU's global
evaluation, including cancellation. With zero-or-normal intermediates and no
overflow, a conservative gamma bound uses `u32=2^-23`, not round-to-nearest's
half-ULP assumption, and bounds the sum of absolute terms. No blanket bound for
all admitted affines has been proved here. Integer/border transitions need their
own handling; uncertain tiles use exact coordinate packets or CPU rendering.

Independent arithmetic inference: near both 6000 and 8000, binary32 spacing is
2^-11 source pixels. Half that error at a black/white edge can approach 0.80 encoded
8-bit codes near black before gains or other errors; a full spacing approaches 1.61.
Rebasing removes this large integer contribution, not coefficient or conditioning
error. Identity/integer/half placements with bounded dyadic increments are promising;
rotation/shear need the bound. Huge cancellation or ill-conditioning can force
packets without forcing all large photos to fail.

## Color admission and error envelope

Upload exactly the CPU-validated f32 operation parameters/gains; do not replace
host exposure conversion with shader `exp2`. The initial portable kernel should
admit **zero or normal finite operands/results only**, with explicit headroom for
all permitted arithmetic forms. Reject subnormal vector fills/opacity/gains on
the host. Guard potential underflow/overflow before multiplication/addition using
conservative exponent/range bounds; a post-result check cannot repair overflow.
Preserve materialization between ordered operations instead of collapsing exposure
chains or moving masks across groups. Guard masks and source-over intermediates too.
If lower/upper bounds are inconclusive, request CPU evaluation; this is not itself
a document `ColorRange` error. The CPU decides whether the document actually fails.

A concrete first candidate range is zero or `2^-120 <= |value| <= 2^120` for
retained working channels and evaluated nonzero intermediates. This leaves room
inside binary32's normal range while supporting the +16/+32EV controls. Use integer
exponent bounds before products; do not form an overflowing product to test its
range. A range crossing or unresolved cancellation takes CPU. This conservative
policy can decline CPU-valid colors; it never changes their declared precision.

This requires a later reviewed guard implementation, not just this prose. A bounded
4-byte atomic status can flag `NeedsCpu` before unsupported numeric evaluation. Copy/map
that scalar after the job, disclose its latency/traffic, and keep the candidate
private until status/completion succeeds. Ordinary presentation needs no pixel
readback. Flags, cancellation or obsolete revisions preserve the accepted result.

For original premultiplied taps in [0,1], bilinear coordinate error contributes at
most `Ex+Ey` per channel before arithmetic; transparent extension remains continuous
across source borders. Propagate interval/error bounds through the actual graph:
exposure multiplies error by its gain; masks add coverage-error times pixel magnitude;
source-over adds destination-magnitude times source-alpha error. HDR increases that
sensitivity. After composition with a backdrop in [0,1], RGB and alpha errors both
matter. For the ideal sRGB encoder, the maximum slope 12.92 gives a contribution
in code units bounded by `255*12.92*(Ergb+Ealpha)`; shader encoding/quantization error
is additional. These are mathematical tools, not an established arbitrary-HDR
one-code guarantee. Record a working-float budget and encoder bound before enabling
any admitted slice; a few passing fixtures cannot replace graph-wide admission.

Proposed initial sampler acceptance target: absolute error at most `2^-17`
(about 7.63e-6) in each [0,1] RGBA channel, before operations. The exact-coordinate
packet contributes zero coordinate error; multiplication/interpolation still need
an audited arithmetic bound against CPU results. This target is **unproved**, not
a default tolerance for HDR graphs. Propagate its bound and refuse jobs that exceed
the selected final budget; power-of-two interior controls retain their exact oracle.

One prospective job owns snapshot/source identities, exact request/format, revision
and device generation, plus admitted resources until final submitted use completes.
It yields a checked GPU result or an explicit CPU requirement. Keep the pass plan
private; no generic executor, new geometry package or detached pixel-owner escape
is required. [W, existing ADR0007]

## At most three coordinator probes, all unexecuted

1. **Coordinate/source sampler:** 24MP/48MP procedural black/white low-light and
   translucent edges; identity, half translation, rotation, shear, cancellation-heavy
   large translation and ill-conditioned placements across tiles 17/256/2048.
   Diagnostic packets must match CPU address/fraction bits; GPU output gets a declared
   float bound. Separately evaluate any rebased packet against its claimed Ecoord.
2. **Operation precision:** independent translucent +2EV, opaque +16/+32EV,
   exposure recovery, hidden alpha/luminance masks, crop and isolated-group oracles.
   Include alpha 2^-130/+128EV and impending overflow/underflow: accepted GPU results
   meet declared bounds, unsupported cases take CPU, and no invalid candidate publishes.
3. **Real adapter handoff/admission:** diagnostic final display within one code for
   those fixtures; record adapter/driver/features, float error, status bytes/latency,
   packet/source/coverage uploads and retained peak bytes. Cancellation/device changes
   retire resources correctly. No universal accuracy or performance claim follows.

## Primary evidence appendix

- [P] Palette 0.7.7, MIT OR Apache-2.0, published VCS
  `0a516fed5e94c9ef4b12469410c1981fa6ba7d06`:
  [decode table interface](https://github.com/Ogeon/palette/blob/0a516fed5e94c9ef4b12469410c1981fa6ba7d06/palette/src/encoding/srgb.rs),
  [interpolation](https://github.com/Ogeon/palette/blob/0a516fed5e94c9ef4b12469410c1981fa6ba7d06/palette/src/macros/mix.rs).
  Local published-source SHA256 respectively
  `3fd461d097ff935258747dcf29134e16c6d1cfd5ae86233eaf3e326baa4f2f38` and
  `57a9d153408d3c27093e01a394158e7db36320617495dd6c29d03e3aa1d8fd0d`.
- [K] [Kurbo 0.13.1 affine](https://github.com/linebender/kurbo/blob/838b69ef35f3381cefc007d86aa8364f5a6dc673/kurbo/src/affine.rs):
  inverse line 426, point multiplication line 589; SHA256
  `7288c89bb5ce16acd7d1b046f189df5855cfafa0454d667fc7b0dc4454fa2663`.
- [W] [wgpu-types 29.0.4 formats](https://github.com/gfx-rs/wgpu/blob/e99f5305ded96ff7006f0714d043a7f735bd45c2/wgpu-types/src/texture/format.rs),
  [completion](https://github.com/gfx-rs/wgpu/blob/e99f5305ded96ff7006f0714d043a7f735bd45c2/wgpu/src/api/queue.rs).
  Format SHA256 `0a8f0cf2506ae25d2e185e9dbc22edfe292ef82ad392f58b99a7e82ede45e5da`;
  retained upstream pins/checksums are in `.research/gpu-reuse/manifest.json`.
- [A] Read-only CPU seam inspection: `crates/affint-render/src/compositor.rs`
  SHA256 `0b85420226f5d0f7a40edee761042ef819921063dec62aa71d245bfd23ee59a8f`;
  [render contract](../render-contract.md), [ADR0007](../adr/0007-linear-working-output.md).
- WGSL evidence is the dated 21 September 2026 W3C draft linked above. Firecrawl's
  structured extraction mislabeled sections and overflow behavior; it was discarded.
  Direct primary text established the statements. No raw vendor source was read.
