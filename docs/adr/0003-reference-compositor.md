# ADR 0003: Bounded tiled linear-light CPU reference compositor

Date: 2026-10-12. Status: implemented and coordinator-verified.

The rendering module must evaluate original artwork at the exact requested output
resolution, preserve editable source content, and provide a numerical reference
for future accelerated rendering. We chose a synchronous, UI-independent
`affint-render` module with f32 premultiplied linear-sRGB tile buffers and final
straight RGBA8 sRGB output. It consumes the document module's validated immutable
`Snapshot`; it neither edits nor clones original pixel allocations. The first
reference is deliberately finite precision, rather than an ICC/HDR production claim.

The public seam is `Renderer::new(Limits)`,
`Renderer::render(&Snapshot, RenderRequest) -> Result<Frame, Error>`, and the
cooperative `render_with_cancel` variant. `RenderRequest::new` identifies a page,
exact nonzero dimensions, and explicit `View::Region(Rect)` or
`View::Affine(Transform)`. Regions fill the output; affines map page coordinates
to output pixel edges. Pixel sampling uses destination centers `(x + .5, y + .5)`.
`Frame` returns a borrowed `rgba8()` slice, exact dimensions, request identity, and
snapshot revision. Callers can discard obsolete results and share `Arc<Frame>`;
there is no deep clone or conversion that detaches owned pixels from accounting.
Rendering is worker work, not a UI-thread activity; scheduling belongs to its caller.

Each group evaluates its back-to-front children into an isolated tile, then applies
its live operations in stored order, mask, and opacity. Composed f64 Kurbo affines
place content directly on the requested output grid. Raster sampling uses original
full-resolution RGBA8 sources, accurate sRGB decoding, and bilinear interpolation
of premultiplied linear color with an explicitly transparent border. Identity
sampling subtracts `.5` before computing source neighbours, so aligned samples
round-trip instead of being inadvertently blended. No source pyramid, preview
reduction, transformed whole-page intermediate, or lossy effect cache exists.

Original editable Bézier verbs remain quadratic/cubic in the document. A measured
42-alpha-code clip-dependent cubic defect justified render-only Kurbo flattening in
GLOBAL f64 device coordinates at a target tolerance of `1/1024` output pixel, before
coverage blocks are clipped or points become f32. One derived path is prepared per
coverage evaluation and shared by its blocks; no source path is changed. Kurbo's
published Hausdorff target is an approximation, not an absolute analytical guarantee.
Vector fills and transformed local crop rectangles use tiny-skia's antialiased, nonzero-winding,
8-bit coverage masks on a fixed 256-pixel grid anchored at output `(0, 0)`.
Coverage is explicitly scanned at eight times the requested pixel grid, raising
tiny-skia's internal 4x4 geometric lattice to 32x32. Each 8x8 coverage block is
box-averaged and rounded to one final coverage byte. This reduces sample-threshold
amplification without reducing source or output resolution. Color scratch tile size is independently configurable: it copies exact intersections
from those coverage blocks. Fill color, exposure, source-over, masks, and opacity all
remain f32 linear premultiplied operations; tiny-skia does not composite artwork
colors. Crop multiplies geometric coverage on the requested grid while retaining
original source coordinates and pixels. Page bounds also clip the output to the
selected page. Transparent output RGB is canonical zero. Final output clamps RGB
to `[0, 1]`, demultiplies, encodes sRGB, and quantizes alpha to nearest RGBA8.

Exposure scales linear RGB by `2^stops` without intermediate highlight clipping.
A positive operation followed by its negative can recover original highlights,
including across child/group composition. Nonfinite results and nonzero values
underflowing to zero in checked color conversion, exposure, or coverage are
structured refusals. This finite f32 policy cannot represent arbitrarily large or
tiny HDR values. Source-over uses Palette's premultiplied Porter-Duff composition;
its RGB path does not clamp highlights. Alpha remains bounded coverage.

Mask dependencies follow the document contract precisely: their own content,
affine, operations, mask, and opacity apply in the TARGET layer's local coordinates.
The target-local-to-output placement is combined with the source affine once.
The mask source's normal owner/ancestor transforms and ancestor visibility are
irrelevant, and its own visibility is ignored for this dependency. Children of a
mask group retain their own ordinary visibility. A source may also paint normally
when visible. Alpha masks read evaluated alpha; luminance masks use Rec. 709 linear
coefficients `0.2126/0.7152/0.0722` on already-premultiplied RGB, equivalent to linear
straight luminance times alpha. Coverage is clamped and optionally complemented.

Memory admission is shared by renderer clones and concurrent jobs through an
`Arc`-owned atomic budget. Each render preflights reachable dependency depth and
path counts; curve counting borrows original elements and precharges a bounded Kurbo
workspace without retaining the entire derived path. It then reserves its conservative
whole-job peak in one compare/exchange admission. Output gets a split RAII lease;
scratch uses a local budget contained in the global scratch reservation. This
prevents concurrent jobs from stranding one another with partial reservations.
The global scratch lease ends after completion/error/cancellation; the output lease
lives until the owning frame pixels are dropped, even if the renderer has dropped.
Separately constructed renderers intentionally have separate budgets; the app must
share its renderer when it intends one global rendering budget.

Peak scratch admission is `(dependency_depth + 1) * tile_pixels * sizeof(Pixel)`,
plus an assembled coverage tile, one canonical coverage block, one 8x-resolution
coverage block (at most 2048x2048 bytes), the largest
reachable path's conservative scan workspace, derived path allocation, and bounded
Kurbo flatten workspace. The canonical coverage extent is clipped
only to output dimensions, never to a configurable color scratch tile.
Only the output is whole-frame. Application-owned vectors use checked arithmetic,
`try_reserve_exact` before initialization, and capacity accounting. Existing sources
are borrowed and remain charged by document-session ownership. Default limits are
32 million output pixels, 16,384 per dimension, 512 MiB shared admission, 256-pixel
tiles, depth 128, 131,072 original and derived elements per path, and twenty billion abstract evaluation
work units. Tiles can be configured from one through 2048 pixels; dependency depth
has a hard ceiling of 128 to bound the implementation's recursion. Repeated mask
DAG evaluation also consumes a bounded work counter. Requests that exceed limits
are refused, never resized or silently degraded.

The upstream tiny-skia public builder and scanner allocate infallibly. We admit
8192 bytes per original path element plus 65,536 fixed bytes and 64 bytes per
scanline pixel before calling them. The audited 0.12.0 edge clipper emits at most
18 edges per path edge; the allowance covers compact path points/verbs, geometric
Vec capacity growth, stable edge-sort workspace, and alpha runs conservatively.
Raster coverage storage itself uses our fallible buffer, moved into `Mask::from_vec`.
This is an allocation admission policy, not measured RSS: allocator headers,
thread stacks, fixed dependency state, and system-wide exhaustion in an infallible
upstream allocation are not covered by a recoverable-allocation guarantee. A full
transitive security/license audit remains a coordinator obligation after locking.
Changing tiny-skia versions requires revisiting the private workspace allowance.
Kurbo 0.13.1 flatten's private cubic quadratic/parameter Vec is precharged at 2 MiB;
public `CubicBez::to_quads` counts are guarded at 4096 using the audited 0.1 tolerance
fraction. Derived line storage uses a separately charged fallible `Buffer<PathEl>`.
Counts, work, and cancellation are checked in its callback; the library may finish
the current bounded curve before stopping after callback refusal. Changing Kurbo
versions requires revisiting this workspace/subdivision audit too.

Geometry placement/inverse mapping stays f64. Both forward and inverse affine
coefficients must be finite and have absolute magnitude at most `2^40`; determinant
must be finite and nonzero. Only transformed tile-local coverage points become f32:
their absolute coordinate ceiling is `2^20` coverage samples, with at most `1/64`
coverage sample (`1/512` output pixel) conversion error and no nonzero-to-zero conversion. Geometry outside that numerical
scope is refused with its responsible layer identity. This prevents a valid f64
path from being silently lost or mangled by a narrower rasterizer representation.
The first reference's coverage is quantized rather than analytical area integration.
The first verification exposed a real coverage defect: clipping a long cubic to
17-pixel color tiles changed alpha by 42 codes relative to a larger clip. tiny-skia's
edge clipping/chopping and capped forward-difference subdivisions are clip-dependent.
The fixed coverage grid alone did not establish fidelity. Global tight Kurbo geometry
preparation also avoids the upstream cubic cap, but tiny-skia's 4x4 sample lattice still amplified tiny geometry changes: same
geometry across clips agreed exactly, while increasingly fine flattening did not
converge at scale one. The independently finer `1/262144` geometry diagnostics
showed maxima 42/21/11/11/11 alpha codes for scale-one tolerances from `1/256` through
`1/65536`; explicit 8x coverage yielded 4/1/1/1/1. The chosen `1/1024` plus 8x coverage
policy follows that evidence. A separately finer `1/262144` path is scanned as one
full 517-pixel-extent line mask at 8x resolution in the fixture. Every pixel, including
canonical-block borders and interiors, must agree within one alpha and linear
premultiplied output code with that independent clipping/tolerance oracle.
The fixed coverage grid belongs to the numerical reference policy; color scratch
tile settings must never alter it. The seam fixture also requires byte identity
across 17-, 256-, and 2048-pixel color tiles, including alpha-zero canonicalization;
a bounded 3x3 view at the original block border also compares one-pixel color tiles.
A future accelerated comparison still needs independently established numerical tolerances.

Cancellation is cooperative: a cheap non-panicking callback is checked at tile and
node boundaries, before/after coverage calls, and at most every 4096 iterations of
application pixel/path loops. `Error::Canceled` drops all admission leases and returns
no partial frame. Upstream coverage cannot be interrupted inside its synchronous
scan; path, tile, and work limits bound those intervals, but no wall-time cancellation
latency has been measured. The module introduces no async runtime or UI types.

Reuse was verified against primary registry sources on 2026-10-12:

- [tiny-skia 0.12.0](https://docs.rs/tiny-skia/0.12.0/tiny_skia/) and
  [its source manifest](https://docs.rs/crate/tiny-skia/0.12.0/source/Cargo.toml):
  BSD-3-Clause, Linebender repository. Only `std` is enabled; no PNG codec or SIMD
  feature is required here. We call the existing curve/coverage primitive rather
  than write a new vector rasterizer. Registry tarball SHA-256:
  `47ffee5eaaf5527f630fb0e356b90ebdec84d5d18d937c5e440350f88c5a91ea`.
  Its paired tiny-skia-path 0.12.0 tarball hash is
  `edca365c3faccca67d06593c5980fa6c57687de727a03131735bb85f01fdeeb9`.
  Audited public/private paths include `src/mask.rs`, `src/edge_builder.rs`,
  `src/edge_clipper.rs`, `src/edge.rs`, `src/alpha_runs.rs`, `src/scan/path.rs`,
  `src/scan/path_aa.rs`, and tiny-skia-path's `src/path_builder.rs`/`src/path.rs`.
- [Palette 0.7.7](https://docs.rs/palette/0.7.7/palette/) and
  [its manifest](https://docs.rs/crate/palette/0.7.7/source/Cargo.toml):
  MIT OR Apache-2.0, Rust 1.71 minimum, `std` only. Registry tarball SHA-256:
  `ddeed8580d347d2abf3dcf06a5f0b3dc020258338526b277847cd4248a70fc64`.
  Source also exists in `.research/reference-crates/palette-0.7.7`.
  `src/encoding/srgb.rs`, `src/rgb/rgb.rs`, and `src/blend/{pre_alpha,compose}.rs`
  establish accurate transfer, premultiplied interpolation, and unclipped RGB
  source-over. Output uses float-to-float transfer then integer quantization.
- [Kurbo 0.13.1](https://docs.rs/kurbo/0.13.1/kurbo/) remains the document geometry
  representation audited in ADR 0001 (MIT OR Apache-2.0). No duplicate path model
  enters the public seam. thiserror 2.0.21 retains structured errors.

Using tiny-skia's convenient 8-bit colored pixmaps for the whole compositor was
rejected: encoded/quantized color composition cannot retain HDR highlights or meet
the declared linear-light contract. Writing a vector scan converter and color
transfer/composition subsystem was also rejected because maintained primitives
already solve those problems. Whole-page f32 intermediates per group/layer were
rejected because residency grows with full output size and dependency count. The
tiled reference introduces no speculative renderer backend trait or cache; future
GPU work should earn its interface through an actual accelerated implementation.

Integration fixtures cross the actual document-session seam for imported raster
pixels, vector/group insertion, editable operations and transforms, masks, and
crop undo/redo. Static independently derived color goldens catch encoded-channel
blend/interpolation, source-center misalignment, transparent color fringes, hidden
mask suppression, ancestor-coordinate leakage, wrong group opacity, and early HDR
clipping. The quadratic-edge golden is independently analytical: `y=3*(1-x/2)^2` covers
`4/(3*sqrt(3))-3/4 = 0.0198003589` of pixel `(0,0)`, yielding alpha 5. The higher
quality coverage correctly revealed this edge previously missed by coarse sampling;
fully exterior and all fully interior hole pixels still require exact zero.
Other fixtures cover curves/nonzero holes, exact views/page clipping,
tile seams, shared concurrent admission and held-frame lifetimes, cancellation,
and dimension/depth/work/geometry/derived-geometry/color-range refusals. Coordinator
verification passed: 21 renderer integration tests and one public usage doctest,
format checks, and warning-free clippy across all renderer/storage targets. The same
run also passed native storage and PNG interchange tests. Exact command:
`./scripts/verify --package affint-render --package affint-storage --jobs 2 --label renderer-png-fidelity-04`.
Evidence is `.research/verification/renderer-png-fidelity-04/result.json` and its
step logs. No worker compile/test/benchmark was run; no performance benchmark exists.

Remaining scope is explicit: no ICC/profile-aware rendering, different source color
models, floating-point original assets, HDR output, strokes, typography, blend modes
beyond source-over, image filters, vector booleans, external document-format oracle,
GPU acceleration, persistent effect/source caches, dirty-region scheduling, or speed
claims. The first render's bilinear minification is a declared sampling policy, not
a high-quality anisotropic reconstruction filter. Performance and peak RSS must be
measured on recorded hardware before optimization or responsiveness claims.
