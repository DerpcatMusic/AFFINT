# Linear working output and native display follow-up

Status: source review and independent scalar analysis, 2026-10-12. No implementation,
AFFINT test, GPU execution or benchmark was performed for this report. App source
remains frozen. Renderer coverage/culling work is separate. Reviewed checkpoint:
`3d810bf48daaf5825d56f17cb1d1b63760f0beff`; conclusions concern its color/output seam,
which was also inspected in the current working tree.

The ordinary native canvas has a measurable **precision limitation at the RGBA8
handoff**: exposed translucent highlights are clipped before the native checker is
composited. Document evaluation itself retains floating highlights. This follows
the explicitly declared first-milestone RGBA8 output contract; it is a fidelity gap
against an unclipped linear working display, not evidence of a vendor defect or a
failed internal exposure operation. A linear-premultiplied F32 output choice is the
smallest useful next seam. It improves display fidelity without itself accelerating
document evaluation or adding HDR-monitor/ICC support.

## What the current source establishes

The [render contract](../render-contract.md) specifies linear-sRGB premultiplied f32
evaluation, ordered live operations, masks, opacity, isolated groups and source-over,
followed by exact-size straight RGBA8 sRGB output. Its first-milestone precision
qualification is explicit. Relevant implementation evidence:

- [Renderer interface](../../crates/affint-render/src/lib.rs), `RenderRequest`,
  `Frame`, `output_bytes`: format is currently fixed. `Frame` borrows `rgba8()` and
  keeps its accounting lease; it has no deep clone or owned-pixels escape.
- [Compositor](../../crates/affint-render/src/compositor.rs), `node`, `raster`,
  `source_pixel`, `write_tile`: original RGBA8 raster texels decode to linear color;
  bilinear interpolation is premultiplied. Exposure multiplies only RGB by
  `2^stops` with range checks, without clipping. Masks and opacity scale RGB and
  alpha. Group children composite into isolated floating buffers in stored order.
- `write_tile` clips each premultiplied channel to `[0, alpha]`, divides by alpha,
  sRGB-encodes and quantizes it; alpha separately rounds to eight bits. Alpha zero
  becomes canonical zero RGB. This is the explicit lossy final **document** output.
- [Native upload](../../crates/affint-app/src/gpu.rs), `Host::upload`: borrowed RGBA8
  uploads to `Rgba8Unorm`, keyed by revision and exact request. Source/display
  textures are reused or retained and charged until completion.
- [Display shader](../../crates/affint-app/src/display.wgsl): source mode zero
  decodes straight sRGB, premultiplies, then adds a linear-light opaque checker.
  Checker shades are encoded sRGB `0.78` and `0.90`. Source mode one instead uses
  supplied linear-premultiplied RGB directly. Both paths range-clip only when
  encoding the final opaque display texture.

The converter cannot recover information already lost by `write_tile`. For
translucency, clipping the unassociated document color before adding a backdrop
differs from adding the backdrop to the unclipped premultiplied working color.
The existing half-float GPU fixture verifies the latter shader path for its own
supplied samples. It does not verify ordinary document-to-float evaluation.

## Independently computed example

Use a uniform original white raster with RGBA8 `[255,255,255,32]`, identity placement,
an interior sample, opacity one, no other layers/masks/crop edges, and exposure
`+2 EV`. This avoids interpolation, coverage and alpha-quantization ambiguity.
Let `a = 32/255`, gain `g = 4`, and decoded checker intensity `B`:

```text
D(s) = s / 12.92                         when s <= 0.04045
       ((s + 0.055) / 1.055)^2.4         otherwise
E(l) = 12.92 * clamp(l, 0, 1)            when clamp(l, 0, 1) <= 0.0031308
       1.055 * clamp(l, 0, 1)^(1/2.4)
         - 0.055                       otherwise

Working document pixel: (P, a) = (4a, a)
RGBA8 document result:  [255, 255, 255, 32]
Current display:       E(a  + B * (1-a))
Linear working display:E(4a + B * (1-a))
```

These are independent scalar calculations using the transfer formulas in Python
double precision, then nearest eight-bit rounding. They are predicted code values,
not captured AFFINT/GPU pixels. Per RGB channel:

| Source alpha byte | Checker sRGB | Current linear sum | Unclipped linear sum | Current / working display bytes | Difference |
| --- | --- | --- | --- | --- | --- |
| 32 | 0.78 | 0.624381944278 | 1.000852532513 | 207 / 255 | 48 |
| 32 | 0.90 | 0.814089962883 | 1.190560551119 | 233 / 255 | 22 |
| 64 | 0.78 | 0.678282293081 | 1.431223469552 | 215 / 255 | 40 |
| 255 | 0.78 | 1.000000000000 | 4.000000000000 | 255 / 255 | 0 |

For the first row, `P = 0.501960784314`, `a = 0.125490196078` and
`B = D(0.78) = 0.570481595475`. The current encoded result is `0.812008654956`.
The opaque control explains why an ordinary white highlight fixture can miss this
limitation. A translucent vector edge or mask can create the same kind of loss;
its coverage and alpha rounding need separate fixtures. Outside-page pixels are
transparent under the page-coverage contract, so this report does not apply the
white sample to the outside-artboard background and claim an observed artifact.

This also fixes the future fidelity target: native working display and transparent
RGBA8 PNG export need not look identical after a third-party application supplies
its own backdrop. PNG export must remain an explicitly lossy SDR representation;
it must not secretly bake the editor checker into artwork.

## Smallest owned renderer extension

Keep the change in `affint-render`, using its existing budget module and cancellation
interface. No new wrapper crate, UI types or generic backend trait is needed.

1. Add an explicit output choice to validated `RenderRequest`, for example
   `OutputFormat::{SrgbRgba8Straight, LinearSrgbRgbaF32Premultiplied}`. Preserve the
   existing constructor's RGBA8 behavior and add a named validated way to request
   F32. Format participates in exact request equality, scheduling, cache/upload
   identity and stale-result acceptance, alongside page, dimensions, view, revision
   and device generation. A format mismatch cannot reuse a same-sized texture.
2. Keep one `Frame` owner with private storage variants `Buffer<u8>` and
   `Buffer<[f32;4]>`, returning a borrowed `FramePixels<'_>` enum. Callers explicitly
   match straight sRGB bytes versus linear-premultiplied F32 arrays. Migrate the
   unconditional `rgba8()` call rather than make it panic, implicitly convert or
   return empty pixels for the F32 variant. Use one representation per request.
3. Write floating output directly from evaluated tiles, preserving finite RGB above
   alpha/one, alpha in `[0,1]` and zero-alpha canonical zero RGB. Do not unpremultiply,
   encode or clamp RGB in this path. Continue structured overflow/underflow errors;
   F32 is finite working range, not unlimited HDR. Preserve the existing RGBA8
   conversion exactly for explicit PNG/export/reference requests.
4. Use checked `pixels * 4` or `pixels * 16` admission before allocating. Current
   renderer clones, output owners, scratch and pending/export jobs share the same
   budget. Moving a frame or cloning `Arc<Frame>` shares its existing allocation
   and lease; returning an owned raw buffer would evade that invariant.

The borrowed enum is a small representation seam, not a replacement compositor.
Both outputs must traverse the same document evaluation. Export requests always
select RGBA8 explicitly; native working requests select F32 only after host admission
and the tests below are verified. Over-budget requests fail while retaining the
previous valid canvas, without resizing or quietly reverting to clipped precision.

## Upload, format and resource policy

The inspected primary crates are cached under
`<local Cargo registry>/`. The exact pinned
wgpu 29.0.4 source is authoritative for this recommendation:

- `wgpu-types-29.0.4/src/texture/format.rs`, `guaranteed_format_features` and
  `sample_type`: `Rgba32Float` has base sampled/copy, render-attachment and storage
  usages, including read-only/write-only storage support. Its filtering requires
  optional `FLOAT32_FILTERABLE`. Existing `Float { filterable: false }` binding plus
  `textureLoad` avoids that feature. Do not assume fixed-function float blending;
  explicit source-over compute passes need no such capability.
- `wgpu-29.0.4/src/api/{adapter,device}.rs`: inspect actual device features/limits.
  An adapter's extra format features are restricted unless the corresponding
  `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` device feature is requested. Validate
  intended usages on the created device, rather than accepting adapter maxima.
- `wgpu-29.0.4/src/api/queue.rs`, `write_texture`: input is a borrowed byte slice in
  texture format, but wgpu makes a staging copy. Borrowed upload is not zero-copy
  GPU transfer. `on_submitted_work_done` requires runtime polling/submission to drive
  callbacks; callbacks should only publish short completion notifications.
- `wgpu-types-29.0.4/src/texture.rs`, `TexelCopyBufferLayout`: queue writes do not
  require 256-byte row alignment; encoder buffer-to-texture copies do. Budget row
  padding when using a mapped reusable staging ring.

Upload linear arrays to a non-sRGB `Rgba32Float` source texture and retain the
existing final opaque `Rgba8Unorm` display texture/shader transfer policy. Never
apply sRGB transfer directly to premultiplied linear RGB. Match the complete format
identity when reusing texture pairs. Keep MUI/Hybrid external display sampling and
physical clipping unchanged.

For safe byte borrowing, the already locked `bytemuck` 1.25.2 supplies
`try_cast_slice::<[f32;4],u8>` without an allocation; propagate its error and retain
the frame during upload. Its `src/pod.rs` implements POD for f32 and four-element
arrays, and `src/lib.rs` documents pointer/byte-count preservation. No AFFINT unsafe
conversion is required. A direct dependency would need an explicit manifest/lock
review, but need not introduce another crate version. Verify supported target byte
order; a platform needing repacking uses a declared, charged, bounded tile buffer,
never a hidden full-frame pixel copy. The F32 host must separately admit outstanding
wgpu staging payload/alignment until completion, even with a zero-allocation Rust
byte borrow. Private driver overhead is not thereby a measured or hard global cap.

Allocation payloads alone, excluding scratch, originals, staging, coverage, toolkit
caches, surface images and allocator/driver overhead:

| Exact output pixels | CPU F32 frame | GPU F32 source + RGBA8 display | Two live GPU pairs |
| --- | --- | --- | --- |
| 4 million | 61.04 MiB | 76.29 MiB | 152.59 MiB |
| 24 million | 366.21 MiB | 457.76 MiB | 915.53 MiB |
| 48 million | 732.42 MiB | 915.53 MiB | 1831.05 MiB |

These are arithmetic, not measured RSS or GPU residency. The app's current 256 MiB
renderer/96 MiB canvas partitions cannot simply admit the same maximum pixels in
F32. Even two 4MP texture pairs exceed 96 MiB. Review admission with current and
retired textures, all outstanding staging, retained frames and scratch included.
Wait/coalesce bounded graphics work or refuse admission; never drop a retired lease
at CPU cancellation while a GPU submission still references it.

Start with dense F32 viewport output that honestly fits those limits. For large
accelerated work, use output-anchored bounded tiles, not one whole-image SSBO:
wgpu defaults are 8192 maximum 2D dimension, 128 MiB storage-buffer binding,
256 MiB buffer and four storage textures per shader stage
(`wgpu-types-29.0.4/src/limits.rs`, `Limits::defaults`). A 256×256 RGBA32Float tile is
1 MiB with 4096-byte rows; its RGBA8 display tile is 0.25 MiB. Validate partial-edge
tile strides, dimensions, dispatch counts and checked total residency against the
actual device. Limit the number of admitted tiles/staging slots and retire them on
completion. Tiling does not authorize source reduction or seam-dependent coverage.

Do not substitute `Rgba16Float` merely because the diagnostic fixture uses it.
Binary16 maximum finite value is 65504: opaque white at supported `+16 EV` is 65536
and already overflows; `+32 EV` is 4294967296. Binary16 spacing near one is 1/1024,
and tiny coverage can underflow. These are material changes from F32. Any future
half-precision mode needs explicit gain/range/coverage bounds and quantified error
tolerances before it can enter an exact request or cache.

## A useful first accelerated document slice

After the F32 seam and numerical policy are verified, add a concrete GPU evaluator
inside `affint-render`, accepting the host's existing wgpu 29 device/queue. Keep the
CPU evaluator as the oracle. Returning a leased GPU working texture/tile set with
revision/request/device identity is a real distinct output owner; avoid designing
a general backend abstraction before this actual interface exists.

The first slice should execute raster sampling, ordered crop/exposure, alpha or
luminance masking, opacity, isolated groups and ordered source-over on resident
RGBA32Float tiles. CPU-generated tiny-skia vector/crop coverage can be uploaded as
bounded R8 coverage initially; this preserves the established coverage semantics
while removing CPU artwork-color processing. Label that division honestly.

- Keep immutable original RGBA8 sources resident by source identity/content,
  uploading only on a real source/device change. Decode each sampled texel to
  linear and premultiply **before** bilinear interpolation. Match original texel
  centers at `0.5`, transparent borders and the CPU's four-neighbor sampling.
- Schedule ordered operations and isolated group children exactly as the CPU.
  Separate sampled read inputs and write-only storage output; ping-pong distinct
  F32 tile textures, avoiding input/output aliasing and optional float blend modes.
  Crop/vector coverage still follows the exact output-anchored coverage grid.
- Evaluate masks in the target's local placement context, with the source's own
  transform/operations/masks/opacity. Ignore its normal ancestors and bypass its
  normal visibility only for dependency evaluation. Luminance is premultiplied
  linear `0.2126R + 0.7152G + 0.0722B`, clamped to coverage before inversion.
  Cache identity includes dependency role/placement, not merely mask NodeId.
- Preserve f64 host geometry validation. WGSL f32 coordinate arithmetic can lose
  fractions at large translations; derive tile-relative coordinates and establish
  a numerical admissibility range before claiming CPU agreement. Unsupported
  precision/range takes a documented CPU path or returns a structured refusal.
- Bound graph preparation, depth/work, dependency tiles, source residency,
  staging and submissions. Coalesce latest requests and stop obsolete preparation;
  submitted work must finish/retire. Accept only matching revision/view/format/
  generation results. Convert the resident working output directly to the existing
  display texture on the shared device. Ordinary painting never reads it back.

This is substantive raster/operation/composition acceleration even with CPU
coverage. Measure setup/source upload separately from warm edits, pan/zoom and
end-to-end display. Existing CPU timings identify whole workloads, not an isolated
GPU-eligible hot function, and justify no speedup claim for this proposed slice.

## Future acceptance evidence and reuse audit

Required new evidence crosses the real request/frame/native-converter interfaces:

1. The analytic interior fixture above returns F32 `(128/255,128/255,128/255,32/255)`
   and unchanged RGBA8 `[255,255,255,32]`. Display both formats on actual checker
   cells and compare predicted 207/233 versus 255/255, with at most one display code
   of measured conversion error. Inspect working floats too; final clipping alone
   can conceal a wrong HDR implementation.
2. Exercise translucent colors, zero-alpha source RGB, bilinear borders, fractional
   affine placement, output-tile boundaries, crops, hidden alpha/luminance masks,
   nested isolated groups, repeated positive/negative exposures and opacity.
   Original bytes/checksums and editable operations remain unchanged. Choose working
   float absolute/relative tolerances from accumulated operation/range analysis;
   the existing 1/255 display bound is insufficient for arbitrary HDR intermediates.
3. Exercise format-change identity, stale handoff, cancellation, device loss,
   retired uploads, limit refusal and complete lease release. Concurrent explicit
   RGBA8 export and F32 native work must share admission. PNG/export bytes retain
   the old SDR contract. Diagnostic readback is allowed only in explicit validation;
   absence of a suitable adapter is a reported incomplete proof, not a passing skip.
4. Before calling the GPU slice accelerated, compare real CPU/GPU working output
   and exact RGBA8 export under the same fixtures, then record cold/warm timing,
   upload/readback bytes, peak admitted allocations and measured process residency
   on named hardware. Include 24MP/48MP only with explicit sufficient admission.

Reuse stays with already pinned modules. Cached manifests record wgpu/wgpu-types
29.0.4 as MIT OR Apache-2.0 (Rust 1.87); palette 0.7.7 as MIT OR Apache-2.0;
tiny-skia 0.12.0 as BSD-3-Clause; Kurbo 0.13.1 as Apache-2.0 OR MIT. Existing notices
and source attribution remain. Bytemuck 1.25.2 is Zlib OR Apache-2.0 OR MIT; its
cached README describes the default feature set as stable with Rust 1.34 support,
while added features have different compatibility promises. Use only the required
safe slice cast and retain notices if promoted to a direct dependency. This review
inspected `src/internal.rs::try_cast_slice`: it preserves the borrow, checks alignment
and whole-element byte length, then creates the slice with dependency-owned unsafe.
It does not constitute an audit of every optional bytemuck feature. A GPU renderer
addition also needs to reconcile its current Rust 1.85 manifest declaration with
wgpu's Rust 1.87 floor; the current workspace toolchain is already newer.
`half` 2.7.1 is MIT OR Apache-2.0 but is not
recommended for this output policy; it was inspected only to verify binary16 limits.

The coordinator's existing 398-package advisory audit passed without ignores
(`.research/security/native-mui-20261012-r2/result.json`). That is evidence for the
locked graph, not a future maintenance guarantee or proof of the proposed GPU
evaluator. Re-run it for any manifest/feature changes. New upstream-release status,
other adapters/platforms, the large-coordinate CPU/GPU error envelope and bounded
GPU scheduling costs remain unverified. No new dependency or implementation is
introduced by this review.
