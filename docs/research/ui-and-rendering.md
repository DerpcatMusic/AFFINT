# UI and full-resolution rendering research

Status: first actionable report, 2026-10-12. Source-verified; **not build-, runtime-, or benchmark-verified**.
Worker: `openai/gpt-6.1-sol`, `xhigh`, confirmed from `PI_MODEL` and `PI_REASONING_LEVEL`.
Ownership: this file and `ui-research-evidence/**` only. No compilation, installation,
configuration changes, nested workers, commits, or deletion were performed.

## Recommendation

**Use eframe/egui with its wgpu backend for the first real AFFINT shell. Keep the
full-resolution document renderer independent of egui.** This recommendation is
about demonstrated integration cost, not a claim that egui is inherently faster or
more comfortable than GPUI.

The decisive evidence is that `egui-wgpu 0.36.2` exposes a shared device/queue,
GPU-texture registration, and custom GPU callbacks. Stock GPUI at the examined
upstream pin and GPUI Pre at its published pin do not expose an equivalent Linux
external-texture element through the examined public canvas/surface interface.
Their `surface` is Apple CoreVideo-only. GPUI's `canvas` invokes GPUI's paint
interface, not an arbitrary wgpu pass. See [E1], [G1], and [G2].

Build the familiar workflow deliberately: compact menu and context toolbar,
left tool rail, central document tabs/canvas, right Layers/History/Properties,
and a status/zoom bar. Panel organization, keyboard behavior, tool affordances,
selection feedback, and atomic gestures make the editor comfortable; toolkit
branding does not establish parity. Enabled commands must execute real edits.

Start with an independently correct CPU renderer and explicit color/precision
policy. Accelerate the same operations with wgpu after differential tests.
Use Vello selectively for vector rendering/coverage, **not as the unqualified
high-precision photo compositor**: the examined released renderer requires
RGBA8 output and copies imported GPU images into an atlas [V1]. Use tiny-skia/resvg
for bounded CPU vector/SVG work, not for preserving float photo intermediates [C1].

Do not introduce a general UI-backend abstraction now. Keep UI types out of the
document and renderer interfaces; the actual CPU/GPU implementations justify a
renderer-internal seam. This allows a later GPUI shell without paying for multiple
hypothetical chrome implementations today.

## Provenance and reproducibility

Evidence is persistent under `ui-research-evidence/` (private research evidence, not distributed).
The interrupted round's pinned snapshots were reused rather than downloaded again.
New published crate archives were downloaded as research source, not installed.
Their SHA-256 values were checked against the first-party crates.io version metadata.
`release-pins.json` records all 19 checksums, licenses, and declared Rust requirements.
`snapshot-blob-verification.json` verifies 98 retained repository source files against
Git tree blob identities, with no mismatches. `inventory.json` records persistent
artifact hashes; `worker-runtime.json` records the actual model and effort.
`releases/<crate>-<version>/.cargo_vcs_info.json` records published Git pins where
provided. Published artifacts take precedence over a possibly different repository
HEAD. In particular, GPUI 0.2.2's VCS metadata says `dirty: true`; its archive checksum,
not the Git SHA alone, identifies the actual published source.

GitHub's unauthenticated API hit its rate limit. Further ArtCraft pins came from
first-party GitHub HTML `currentOid`, followed by commit-addressed codeload archives.
Those HTML files, archives, and `artcraft-pins.json` remain available. Several guessed
paths returned 404; they are acquisition errors, not evidence that a capability is
absent. The Vello paths were corrected after examining the retained tree.

No Context7 CLI was run because the worker is forbidden to install packages;
first-party source and published source archives were used instead. No upstream
benchmark statement is treated as an AFFINT measurement.

### Repository snapshot pins

- Zed/GPUI: `zed-industries/zed@93d745668805029476e6d7fbb19652116fed51de`.
- GPUI Kit: `longbridge/gpui-kit@1ccf011620da24d633f7cd6d3c11ca2e97798dcf`.
- wgpu HEAD: `gfx-rs/wgpu@10ddb587ac153bc5b6b21a23e4f21ec634d2cabe`.
- Vello HEAD: `linebender/vello@235739c4f08eb48d900e1512459d453a9ab77f96`.
- egui HEAD: `emilk/egui@8f6d3d6ed99cb24d2e14c43951803d2868db40b1`.
- RAWmakase: `pch/rawmakase@0f1bc320207658c9778922a4d6cf87bd174be2b2`.
- RapidRAW: `CyberTimon/RapidRAW@2534f67ee25c1cd6c36c8e1641c12f32c3263835`.
- Graphite: `GraphiteEditor/Graphite@b4d07595ca82181297d770f54db941d338ea3af7`.
- PhotoCraft: `storytold/photocraft@6de37ce7f063e6814903650a24c8dad4936e8136`.
- VectorCraft: `storytold/vectorcraft@540505fcad636b7983504ff7a1368a4a0d792803`.
- FilmCraft: `storytold/filmcraft@6e0a6a65ed4a27ac0db1edba6aa833f798ec6a49`.
- DeckCraft: `storytold/deckcraft@be14248577b92059eb2b8c0c9460cceffbc1f01d`.

## Toolkit comparison: concrete engineering costs

### GPUI plus GPUI Component / Kit

Verified positives:

- GPUI describes itself as hybrid immediate/retained-mode GPU UI. The first-party
  `gpui.rs` page and crate README are retained; framework speed language is not
  a comparative application benchmark.
- Linux support is real source, not just a platform claim: upstream
  `crates/gpui_linux/Cargo.toml` enables X11 and Wayland by default and wires both
  to `gpui_wgpu`. Native implementations exist under `linux/x11/` and
  `linux/wayland/` [G1]. This does not verify every compositor/driver/input device.
- Kit provides styled desktop widgets and dock panels. Its current source package
  is `gpui-component 0.7.1`; `crates/base/src/dock/` owns substantial docking
  machinery, and `website/component/dock.md` describes the styled adapter [G2].
- GPUI source examined here is Apache-2.0. Do not confuse the GPUI crate license
  with every other Zed workspace package. Kit software is Apache-2.0; its current
  documentation also offers CC BY 4.0 with attribution requirements. Lucide icon
  assets carry ISC and listed Feather-derived icons carry MIT notices [G2].

Critical costs:

1. **Version identity is not interchangeable.** crates.io `gpui` is 0.2.2,
   published in 2025; Kit 0.7.1 depends on the separately packaged
   `gpui-pre = 0.3.8` and matching platform/macros crates. Its normalized package
   manifest and current workspace manifest both establish this. GPUI Pre platform
   metadata pins Zed `279fe070bb389b79652e52065b2f001edcc0b11b`. Optional
   `gpui-fast` is yet another package. A simple `gpui = "0.2.2"` plus current
   Component does not establish compatible GPUI types [G2].
2. **A canvas is not an external GPU surface.** Upstream
   `elements/canvas.rs` receives `Bounds`, `Window`, and `App`. `surface.rs`
   exposes only Apple CoreVideo variants and gates its constructor on macOS/iOS.
   GPUI Pre's released `surface.rs` has the same platform gate [G1, G2].
3. **The verified image path starts with host bytes.** `Window::paint_image`
   passes `RenderImage::as_bytes` to `PlatformAtlas::get_or_insert_with`.
   The wgpu atlas stores pending `Vec<u8>` uploads and calls `Queue::write_texture`.
   Repainting an unchanged cached image need not upload it again, but a newly
   computed GPU photo cannot enter this verified path as a GPU texture alone [G1].
4. Upstream `WgpuContext` has public device/queue fields, but access to that type
   is not proof of an application-accessible canvas insertion seam. A custom
   platform/renderer modification would have to establish draw ordering, clipping,
   event coordinates, resource retirement, error recovery, and X11/Wayland behavior.
   The source does not justify claiming that GPUI fundamentally cannot do this;
   it just does not supply the required verified public path for AFFINT yet.
5. The examined Zed HEAD uses wgpu 29, unlike the recommended released egui/Vello
   combination using wgpu 30. Sharing resources across different device instances,
   or different crate-major types, is not automatic.

**Decision gate if GPUI remains preferred:** coordinator-approved implementation
must draw an independently computed GPU texture in a clipped GPUI canvas on both
X11 and Wayland, with zero image readbacks during editing; show the exact supported
interface/fork and quantify ongoing maintenance. Native child-window embedding is
not presumed portable or equivalent to an ordinary canvas. Do not delay core
correctness to implement a toolkit fork speculatively.

### eframe / egui / egui-wgpu

- `CreationContext::wgpu_render_state` and `Frame::wgpu_render_state()` expose
  `RenderState`; its device, queue, adapter, target format, and renderer are public.
- `Renderer::register_native_texture` constructs a bind group for an existing
  `TextureView`, with `texture: None` in its bookkeeping; it does not read back
  or upload image pixels. Reuse an ID with `update_egui_texture_from_wgpu_texture`
  when its view changes. Application code retains the actual texture [E1].
- Its documented native-image path requires `Rgba8Unorm`. Do not give it an
  arbitrary float texture and assume the default image shader provides the right
  color or alpha conversion. Use a custom callback shader for float sampling and
  the explicit working-to-display transform.
- `CallbackTrait::prepare`, `finish_prepare`, and `paint` support custom commands
  and resources. `paint` draws into the egui render pass; preparation can use the
  supplied encoder or return command buffers. eframe documents their submission
  ordering [E1]. This is the practical single-device pipeline seam.
- X11 and Wayland are explicit eframe features. Accessibility is a separate feature;
  retain it intentionally when disabling defaults. Windowing is provided by winit
  0.30.13, which also has both Linux backends [E2].

Engineering costs: build AFFINT-specific compact styling, dock layout/persistence,
focus and shortcuts, drag/gesture semantics, accessibility labels, dialogs, and
multi-document state. Immediate-mode UI work must not become per-frame decoding,
whole-document copies, or GPU-resource allocation. Unicode UI text rendering is
not a substitute for an editable document typography engine.

**Recommended compatible release family:** `eframe/egui/egui-wgpu = 0.36.2`,
`wgpu = 30.0.1`, optional `vello = 0.11.0`; their examined manifests use wgpu 30.
The egui family declares Rust 1.95. The coordinator must confirm available toolchain
and the complete locked dependency graph; this worker did not run rustc/cargo.

### Direct winit / wgpu

This gives the clearest ownership of the surface, frame scheduling, precision,
render-pass order, and device lifetime. `wgpu::Instance::create_surface` provides
safe window-target creation; application unsafe is not inherently necessary [W1].
But winit is window/input infrastructure, not a complete editor chrome toolkit.
A pure direct implementation must supply widgets, text editing/IME, focus,
shortcuts, menus, accessible semantics, docking, dialogs, hit testing, layout,
theming, scaling, and repaint scheduling. Reusing `egui-winit` plus `egui-wgpu`
reduces those costs, but then it is effectively a more manual egui host.

Use eframe first. Move to a direct winit host only after a concrete eframe ownership,
color, scheduling, or surface limitation is demonstrated. That migration can leave
the document and graphics module intact.

## Can full-resolution workloads avoid repeated host round trips?

**Yes, conditionally:** all accelerated operations and presentation must share
compatible wgpu resources on one device, retain their results, and avoid crossing
CPU-only operations between GPU passes. This is supported by [E1], [V1], [R1],
and [R2]; it is not a claim that every proposed AFFINT operation is already accelerated.

Proposed path:

- Decode/inspect original assets once on bounded workers. Keep original file bytes,
  metadata/profile identity, and immutable decoded content. Upload required source
  tiles once per device/residency lifetime, not once per repaint or undo step.
- Apply masks, transforms, adjustments, and compositing in resident textures/buffers.
  A source upload, GPU-to-GPU atlas copy, and GPU-to-CPU readback are distinct costs.
- Present the resident result through a custom egui-wgpu callback or an explicit
  RGBA8 display texture registered with the renderer. Pan/zoom should update view
  constants; it should not clone full images or force image readbacks.
- Read back only when a CPU consumer genuinely needs pixels: reference comparison,
  export/encoding, explicit pixel inspection, or bounded analytics. Export reads
  back the requested full-resolution output, once per output tile/stripe as needed.
- A CPU-only filter is an explicit pipeline branch with accounted transfer costs,
  not a hidden GPU→CPU→GPU operation repeated every frame. Initially a CPU renderer
  can upload only dirty display tiles and still avoid GPU readback entirely.

A single huge texture is not a full-resolution architecture. Query actual device
limits and format capabilities, tile work larger than those limits, and cap all
resident/staging/job allocations by bytes. Neighborhood filters need exact halos;
transformed sampling needs the correct source footprint. Global operations may
need reductions or multiple passes. Arbitrary tiling is not proof of equivalent
pixels at tile seams.

## Graphics and content reuse decisions

Versions below are the observed latest stable registry versions at retrieval, not
promises of ABI stability or dependency compatibility. Exact archive checksums and
source directories are in `release-pins.json`.

### Geometry: kurbo 0.13.1

MIT OR Apache-2.0, Rust 1.85. Published Git pin
`838b69ef35f3381cefc007d86aa8364f5a6dc673`, VCS path `kurbo`.
Use its f64 paths/curves/transforms and accuracy-controlled geometry [K1].
Do not infer a complete editable vector-boolean system from a general curve library;
choose and audit a boolean implementation separately. Preserve the original operands
and live operation instead of replacing them with an irrecoverable result path.

### Vector rasterization: Vello 0.11.0

MIT OR Apache-2.0, Rust 1.89. Published Git pin
`f3000c8d9e7a9c7e6abb09b9587238bf8b4860ff`, VCS path
`research/vello_research`. Current HEAD also contains separate `vello_cpu` and
`vello_gpu` projects; those are not the same package/version as released `vello`.

`Renderer::render_to_texture` consumes a scene and a wgpu `TextureView`; target
format is documented as `Rgba8Unorm` with `STORAGE_BINDING`. It recommends rendering
to an intermediate texture and GPU-blitting to the surface. `register_texture` /
`override_image` support GPU-resident image sources, but require RGBA8/COPY_SRC,
copy into an atlas, assume unpremultiplied image alpha, and have explicit dirty
invalidation via `mark_override_image_dirty` [V1]. The atlas introduces bandwidth
and memory costs even without any CPU roundtrip.

Recommendation: prove vector coverage/compositing fidelity on a small feature set.
Do not quantize float photo intermediates through this RGBA8 path and describe the
result as high-precision exact rendering. Higher-precision vector blending and any
Vello successor integration remain proof work.

### CPU vector/SVG: tiny-skia 0.12.0 and resvg 0.48.1

- tiny-skia: BSD-3-Clause; published Git pin
  `e48b9027762d33731e60e40eb17eaa4f583b0185`. `Pixmap` is premultiplied RGBA u8.
  It supports bounded CPU fills/strokes/gradients/masks; README explicitly excludes
  ICC, non-RGBA8888 images, general text rendering, and advanced path operations.
- resvg: MIT OR Apache-2.0, Rust 1.85; published Git pin
  `68b14c4c3bccdb60344c777406486b54c36ec1a4`, VCS path `crates/resvg`.
  `render` takes a usvg tree, transform, and tiny-skia pixmap [C1].

Use for bounded SVG rendering and an initial declared-precision vector coverage
implementation. Rasterization is not editable SVG interchange fidelity. Retain
original SVG bytes and unknown structure; separate import support from rendering.
They are not a float color-managed photo engine. Audit the actual usvg version,
font/resource resolution policy, and unsupported SVG features before enabling import.

### Image codecs: image 0.25.10

MIT OR Apache-2.0, Rust 1.88; published Git pin
`76e57184f22772dad1138e96954e57945406b15e`.
Begin with explicitly enabled PNG/JPEG/TIFF support rather than every default codec.
EXR, WebP, AVIF and other codecs are feature-dependent; RAW requires separate audited
camera/decode support, not a file-extension promise [I1].

`ImageDecoder` exposes ICC bytes, orientation, metadata, and limits; `ImageEncoder`
can reject unsupported ICC writes. Retrieve metadata/profile identity before
consuming the decoder. Preserve metadata separately rather than assuming
`DynamicImage` retains everything or every encoder can round-trip it [I1].

**Important fidelity trap:** the JPEG adapter converts colorspaces unsupported by
`image` to RGB during decoding (`to_supported_color_space`). Applying an original
CMYK ICC profile to those RGB pixels would be wrong. Initially refuse unsupported
profile/channel combinations explicitly, or use a lower-level codec that preserves
those samples. This is a required fixture, not a generic "ICC supported" checkbox.

The decoder's allocation limit is documented as best-effort; default width/height
limits are absent and default `max_alloc` is 512 MiB. Add AFFINT's own checked
size/stride arithmetic, allocation reservations, and whole-pipeline byte budget.
Disabling all limits to open a large photo is not the safe full-resolution solution.

### Color management: lcms2 6.2.0 / moxcms 0.9.1

- lcms2: MIT; published Git pin `c4c96835cb10b4c2a947745903372617a2cb3db2`.
  Safe Rust interface over native LittleCMS. Its README states Rust 1.89 even though
  registry `rust_version` is absent. `lcms2-sys 4.0.7` and its vendored LittleCMS
  license are retained and MIT. Private contexts and cache flags affect Send/Sync;
  do not share a cached transform across threads based on an `Arc` alone [M1].
- moxcms: BSD-3-Clause OR Apache-2.0, Rust 1.89; published Git pin
  `1e5305c77a0523acbd96721385ebe6dadfab6bc9`. Pure-Rust ICC conversion, including
  documented RGB/Gray/CMYK/Lab cases, and a dependency of the examined image release
  [M2]. Its claims need external ICC fixtures and LittleCMS comparison before
  choosing it as the only production color engine.

Recommendation: define an explicit source→linear working→output/display policy
first. Use LittleCMS as the initial ICC reference where native dependencies are
acceptable, and evaluate moxcms for a Rust-only production implementation. Do not
include either in the document module. Preserve embedded profiles, rendering intent,
alpha semantics, and unsupported-profile errors. Keep color transforms outside
premultiplied math unless the transform policy explicitly handles it. GPU LUTs need
measured error bounds against the CPU transform; sampling a LUT is an approximation.

Monitor-profile discovery and compositor color handling on Linux X11/Wayland are
**not verified here**. Initial sRGB display behavior must be stated honestly;
professional managed-display/HDR support is not established by wgpu alone.

### Typography: cosmic-text 0.19.0 and rustybuzz 0.20.1

- cosmic-text: MIT OR Apache-2.0, Rust 1.89; published Git pin
  `c24886c2471e5606587c46090cd25dbbf209186b`, repository `pop-os/cosmic-text`.
  Provides multiline layout, bidi, fallback, and swash rendering. **This version
  uses HarfRust**, not rustybuzz, for shaping [T1].
- rustybuzz: MIT; published Git pin
  `8c52723ff75e91a33ae36e527baed871097e64bf`. A lower-level shaping option, not a
  page-layout or paragraph-composition engine. Do not stack two shapers accidentally.

Use cosmic-text for an initial independent document text layout proof, not the UI
font cache as persisted artwork. Record actual font bytes/content identities,
features, variation axes, language/script and fallback decisions; source text and
styles remain editable. Advanced paragraph layout, hyphenation, linked frames,
PDF font embedding, variable/color fonts, and exact print typography need separate
fixtures and license audits. Shaping, glyph outlines, rasterization, and page layout
are different responsibilities.

## Public application source: what to reuse, what to avoid

### RAWmakase: best residency reference, MIT

At the pinned commit, root `Cargo.toml` declares version 0.2.4, eframe 0.36.2,
wgpu 30, Rust 1.98, and MIT. `develop/gpu/resident.rs` retains intermediate state;
`develop/gpu/present.rs` explicitly finishes pixels directly into a display texture.
Its `Frame` returns that texture plus a generation and released-texture list;
`Display::drawn` prevents overwriting textures still shown. Histogram, thumbnails,
and explicit loupe samples are separately described readbacks [R1].

Actionable reuse: study texture generation/retirement, bounded resident stages,
revision/ticket-gated worker results, CPU/GPU parity fixtures, and separate display
vs export policy. `src/app/preview_build.rs` documents identity checks before
publishing background results. Do not import camera tables, profiles, models, icons,
or the entire dependency graph without per-item provenance. Its reduced previews
are not permission to reduce AFFINT exact exports. Upstream performance reports
are upstream reports, not AFFINT results.

### RapidRAW: useful technical reference, AGPL license gate

The retained root LICENSE is GNU AGPL v3. Copying source is blocked pending the
owner's project-license decision and a compatibility review; observing architecture
and independently implementing an idea is not a blanket source reuse decision.
The pinned backend is Tauri 2.11 / wgpu 29, not a GPUI/egui drop-in [R2].

`gpu_processing.rs` supports a resident source cache and `output_to_display` path
that skips ordinary pixel readback and GPU-copies into an output texture. However,
when analytics is enabled on that path, it allocates a buffer for the full output
rectangle and copies pixels back asynchronously. "No readback" therefore has an
explicit analytics exception. High-precision output is restricted to CPU-readback
renders in the examined source. Learn from residency; do not adopt its display
precision/analytics policies as AFFINT's exact render contract [R2].

### Graphite: MIT OR Apache-2.0, selective graph/render reference

Root licenses and manifests are retained. The pinned workspace uses wgpu 29 and
Vello 0.10, not the recommended wgpu 30 family [R3]. Its GPU texture cache reuses
unshared resources and caps *free* bytes; that is not a global cap on all live
resources. Its CPU→GPU conversion quantizes CPU colors into RGBA8 sRGB; GPU→CPU
conversion assumes four bytes/pixel and does an explicit mapped readback.

Actionable reuse: examine graph dependency/evaluation ideas and residency/resource
lifetime separation, not a wholesale editor transplant. Avoid implicit CPU/GPU
conversion nodes throughout a pipeline and do not adopt the 8-bit conversion as
high-precision source preservation. Document types, transaction/history semantics,
geometry, GPU precision, and error paths all need focused audit before code reuse.

### ArtCraft: permissive source, not an already-audited correctness engine

PhotoCraft 0.7.0, VectorCraft 0.9.0, FilmCraft 0.6.0, and DeckCraft 0.5.0 declare
MIT OR Apache-2.0 in root manifests, with corresponding retained LICENSE files.
They use eframe 0.36 at the inspected pins. These licenses do not automatically
cover every bundled font, fixture, model, icon, or transitive dependency [A1–A4].

- PhotoCraft has independently useful tiled/COW and scalar-brush-reference ideas,
  but **do not reuse its history accounting as complete**: `ops/src/tiles.rs`
  visits layer pixels/masks, channels and quick-mask, but does not visit
  `Document::selection`, declared as an `Option<Surface>` in `doc/src/lib.rs`.
  The omission remains visible at this pin. No execution was performed.
- The prior PhotoCraft stale effect-cache finding remains a rejection criterion,
  not a presumed fix. Current `compose/src/lib.rs` does hash `layer.effects`, but
  global effect-map keys shown here contain layer identity, region and light;
  the build also consumes color/depth context and pattern content. No complete
  cache-input audit or reproduced regression test was performed this round.
  `engine/src/smart_cmds/stack_cache.rs` is a separate smart-filter cache; its
  hashing does not prove the global effect-map cache correct.
- FilmCraft's inspected `render/src/lib.rs:media_frame` still changes decode
  errors into `None` through `.ok()?`. Other rendering paths use fallbacks for a
  failed adjacent frame. Do not copy an `Option`-returning export/render plan that
  cannot distinguish missing content, cancellation, unsupported content, and
  decode failure. These must be structured errors in AFFINT [A3].
- VectorCraft's `render/src/lib.rs` uses `vello_cpu`, glyph outlines, and an explicit
  `raster_size` preflight. It is useful evidence for region-based vector rendering
  and truthful raster-size errors, not proof of float compositing or format parity
  [A2]. Preserve editable appearance stacks rather than flattening them.
- DeckCraft is a page/slide layout source candidate, not proof of publishing-grade
  text/layout behavior. No detailed implementation reuse is approved by this
  first-pass inspection [A4].

## Minimal implementation plan

These are implementation requests for the coordinator, not work executed by this
research worker. Keep the first increment small enough to verify meaningfully.

1. **Headless correctness slice.** Independently implement the render interface
   consuming a committed snapshot and a checked requested pixel extent. First
   support an immutable PNG/RGB asset, a solid vector shape, affine placement,
   opacity, and a coverage mask. Define linear-light working space, alpha convention,
   clipping, sample location and finite-input policy before writing acceleration.
   Return structured failures. Initial restricted import/color support must be
   explicit rather than silently interpreting unknown profiles as sRGB.
2. **Atomic real workflow.** Wire one editable layer transform and opacity/mask
   change through document transactions, with one gesture→one history edit.
   Undo/redo must restore identical content and source asset identity. Failed
   imports/render jobs never mutate the current document.
3. **Actual shell.** eframe wgpu, both Linux windowing features, deliberate
   accessibility/default-font features, compact original styling. Add only working
   open/save, layer selection/reorder, tool gesture, zoom/pan, undo/redo, and export
   controls; unsupported features stay absent or unavailable with a reason.
4. **CPU presentation first.** Bounded worker requests carry document revision,
   render policy, output dimensions and view identity. Upload dirty display tiles;
   reuse texture IDs; do no heavy work in UI painting. Reject obsolete results.
   Keep a published texture immutable until no frame can still reference it.
5. **Single-device GPU proof.** Use `CreationContext`'s device/queue. Implement the
   simplest validated compositing operation in wgpu, retain float intermediates,
   and draw through a custom callback with explicit display conversion. Default
   RGBA8 registration is an alternative for display-only output, not the photo
   processing format. Unsupported GPU features fall back to the validated CPU
   operation under an explicit policy, not to blank pixels.
6. **Exact export slice.** Evaluate the same committed state at requested full
   dimensions using tiled/striped work where necessary. Viewport resolution and
   preview proxies cannot enter exact export. Validate dimensions and output profile;
   propagate encode failures; write through atomic storage policy. Initially refuse
   output formats whose row/metadata handling is not validated.
7. **Acceleration and vector proof.** Differential CPU/GPU tests precede adoption
   of each accelerated operation. Compare optional Vello vector coverage, clipping,
   fill rule, stroke and alpha behavior against the declared reference policy;
   separate expected antialias differences from unacceptable color/precision loss.

Deep-module interface requirement: renderer callers specify snapshot, checked
output extent/region, and render policy, then receive a completed result or a
structured error. Callers do not manage filter halos, pixel conversions, GPU atlas
internals, cache keys, staging allocations, or worker cancellation races. The
renderer owns those invariants. The UI presents results and submits edits; it
cannot quietly choose a different exact-render policy.

## Next proof work / acceptance evidence

Only the root coordinator runs build/check/test/benchmark through the repository's
exclusive verification entrypoint. Proposed checks, in priority order:

- Original-asset checksum unchanged across edit, undo, redo, failed transaction,
  save/reopen, crop/mask change, and export. History must charge every retained
  selection/mask/source/cache owner by bytes, including outstanding jobs.
- Cached/uncached equivalence when *each* semantic input changes independently:
  operation parameters, source revision, masks, placement, child order, blend mode,
  color profile/intent, pattern contents, vector operands, fonts, output extent,
  render precision, and implementation/policy version.
- Exact render dimensions at 1:1 and greater than adapter texture limits. Tile/halo
  seam fixtures and monolithic-vs-tiled comparisons for every local filter.
- GPU operation numeric tolerances against a CPU reference; near-zero alpha,
  out-of-gamut/negative/large float values, transparent colored pixels, extreme
  transforms, image edges, non-finite rejection, and cancellation.
- PNG/JPEG/TIFF fixtures: 16-bit ramps, transparency, EXIF orientation, embedded
  sRGB/P3/Gray/CMYK profiles, malformed metadata, dimensions/stride overflow, and
  encoder ICC rejection. Use an independent decoder/oracle, not only self-roundtrip.
- X11 and Wayland: input/IME, focus, clipboard, dialogs/portals, fractional scaling,
  maximization, pointer capture/drag cancellation, multi-monitor movement and sleep/
  device recovery. Display ICC/compositor behavior is a separate color proof.
- Upload/readback counters for initial decode, idle repaint, pan/zoom, source change,
  adjustment drag, undo/redo and export. The resident GPU proof should show **zero
  image pixel readbacks during ordinary edit/display**; analytics exceptions must
  be named and measured, not hidden.
- Measured release workloads with recorded CPU/GPU/driver/toolchain: 24/45/100 MP
  photos, multiple masked layers, and dense vector pages. Report input→present
  p50/p95, peak resident bytes, GPU allocations, transferred bytes and cache hits.
  No world-fastest claim follows from choosing a toolkit or reusing SIMD/GPU code.

Unresolved: minimum hardware/toolchain and adapter capability policy; final project
license; color-managed Linux display/HDR; supported ICC/codec/channel combinations;
advanced editable SVG/PDF interchange; complete font/layout behavior; total memory
accounting under concurrent work; GPUI external-texture integration if desired.

## Exact primary source references

All Git links below are pinned. Matching retained paths are under
`ui-research-evidence/<owner>--<repo>/`; published source is under `releases/`.
References to release source can be inspected without network access.

- **[G1] GPUI**: [canvas.rs](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui/src/elements/canvas.rs),
  [surface.rs](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui/src/elements/surface.rs),
  [window.rs, paint_image at 5053](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui/src/window.rs#L5053),
  [wgpu_atlas.rs](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui_wgpu/src/wgpu_atlas.rs),
  [wgpu_context.rs](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui_wgpu/src/wgpu_context.rs),
  [gpui_linux manifest](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui_linux/Cargo.toml),
  [GPUI Apache license](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui/LICENSE-APACHE).
  `gpui.rs.html` and `gpui.rs-text.txt` retain the first-party site.
- **[G2] GPUI Kit/Pre**: [workspace dependency aliases](https://github.com/longbridge/gpui-kit/blob/1ccf011620da24d633f7cd6d3c11ca2e97798dcf/Cargo.toml#L70),
  [component manifest](https://github.com/longbridge/gpui-kit/blob/1ccf011620da24d633f7cd6d3c11ca2e97798dcf/crates/component/Cargo.toml),
  [dock docs](https://github.com/longbridge/gpui-kit/blob/1ccf011620da24d633f7cd6d3c11ca2e97798dcf/website/component/dock.md),
  [docs license](https://github.com/longbridge/gpui-kit/blob/1ccf011620da24d633f7cd6d3c11ca2e97798dcf/LICENSE-DOCS.md),
  [Lucide notices](https://github.com/longbridge/gpui-kit/blob/1ccf011620da24d633f7cd6d3c11ca2e97798dcf/crates/assets/LICENSE-LUCIDE).
  Published `gpui-component-0.7.1/Cargo.toml`, `gpui-pre-0.3.8/src/elements/surface.rs`,
  and `gpui-pre-platform-0.3.8/Cargo.toml:29` are retained release evidence.
- **[E1] egui GPU interface**: published commit `49682f8baa058bf49e011035cfbd6e825f88a5ef`;
  [renderer.rs callbacks](https://github.com/emilk/egui/blob/49682f8baa058bf49e011035cfbd6e825f88a5ef/crates/egui-wgpu/src/renderer.rs#L44),
  [native texture registration](https://github.com/emilk/egui/blob/49682f8baa058bf49e011035cfbd6e825f88a5ef/crates/egui-wgpu/src/renderer.rs#L768),
  [RenderState](https://github.com/emilk/egui/blob/49682f8baa058bf49e011035cfbd6e825f88a5ef/crates/egui-wgpu/src/lib.rs#L107),
  [CreationContext](https://github.com/emilk/egui/blob/49682f8baa058bf49e011035cfbd6e825f88a5ef/crates/eframe/src/epi.rs).
- **[E2] Linux hosting/features**: `releases/eframe-0.36.2/Cargo.toml:55–115`;
  [winit manifest](https://github.com/rust-windowing/winit/blob/e9809ef54b18499bb4f2cac945719ecc2a61061b/Cargo.toml),
  retained `releases/winit-0.30.13/src/platform/{x11,wayland}.rs`.
- **[W1] wgpu**: [native backend README](https://github.com/gfx-rs/wgpu/blob/10ddb587ac153bc5b6b21a23e4f21ec634d2cabe/README.md),
  released `wgpu-30.0.1/src/api/{instance,texture,queue,device}.rs`, published commit
  `40f4a34ebaf56f9a046231f54125ad046239d3f3`. MIT OR Apache-2.0.
- **[V1] Vello texture/precision interface**:
  [lib.rs at published pin](https://github.com/linebender/vello/blob/f3000c8d9e7a9c7e6abb09b9587238bf8b4860ff/research/vello_research/src/lib.rs#L462),
  retained `releases/vello-0.11.0/src/lib.rs:462–604` and `Cargo.toml:105`.
- **[K1] kurbo**: retained `releases/kurbo-0.13.1/{README.md,Cargo.toml,src/bezpath.rs,src/affine.rs}`.
- **[C1] CPU vector/SVG**: retained `releases/tiny-skia-0.12.0/{README.md,src/pixmap.rs}`
  and `releases/resvg-0.48.1/src/lib.rs:34–66`, plus both release manifests/licenses.
- **[I1] image codec contract**: retained `releases/image-0.25.10/src/io/{decoder,encoder,image_reader_type,limits}.rs`,
  `src/codecs/jpeg/decoder.rs:198–225`, and `Cargo.toml`.
- **[M1] LittleCMS**: retained `releases/lcms2-6.2.0/{README.md,src/transform.rs,src/profile.rs}`,
  `releases/lcms2-sys-4.0.7/{Cargo.toml,vendor/LICENSE}`.
- **[M2] moxcms**: retained `releases/moxcms-0.9.1/{README.md,Cargo.toml,src/lib.rs}`
  and `releases/image-0.25.10/Cargo.toml`.
- **[T1] text**: retained `releases/cosmic-text-0.19.0/{README.md,Cargo.toml,src/shape.rs}`,
  `releases/rustybuzz-0.20.1/{README.md,Cargo.toml,src/lib.rs}`.
- **[R1] RAWmakase**:
  [resident.rs](https://github.com/pch/rawmakase/blob/0f1bc320207658c9778922a4d6cf87bd174be2b2/crates/rawmakase-engine/src/develop/gpu/resident.rs),
  [present.rs](https://github.com/pch/rawmakase/blob/0f1bc320207658c9778922a4d6cf87bd174be2b2/crates/rawmakase-engine/src/develop/gpu/present.rs),
  [preview_build.rs](https://github.com/pch/rawmakase/blob/0f1bc320207658c9778922a4d6cf87bd174be2b2/src/app/preview_build.rs),
  [MIT license](https://github.com/pch/rawmakase/blob/0f1bc320207658c9778922a4d6cf87bd174be2b2/LICENSE).
- **[R2] RapidRAW**:
  [gpu_processing.rs](https://github.com/CyberTimon/RapidRAW/blob/2534f67ee25c1cd6c36c8e1641c12f32c3263835/src-tauri/src/gpu_processing.rs#L2345),
  [backend manifest](https://github.com/CyberTimon/RapidRAW/blob/2534f67ee25c1cd6c36c8e1641c12f32c3263835/src-tauri/Cargo.toml),
  [AGPL license](https://github.com/CyberTimon/RapidRAW/blob/2534f67ee25c1cd6c36c8e1641c12f32c3263835/LICENSE).
- **[R3] Graphite**:
  [texture_conversion.rs](https://github.com/GraphiteEditor/Graphite/blob/b4d07595ca82181297d770f54db941d338ea3af7/node-graph/libraries/wgpu-executor/src/texture_conversion.rs),
  [texture_cache.rs](https://github.com/GraphiteEditor/Graphite/blob/b4d07595ca82181297d770f54db941d338ea3af7/node-graph/libraries/wgpu-executor/src/texture_cache.rs),
  [workspace manifest](https://github.com/GraphiteEditor/Graphite/blob/b4d07595ca82181297d770f54db941d338ea3af7/Cargo.toml),
  root LICENSE-MIT / LICENSE-APACHE retained.
- **[A1] PhotoCraft**:
  [history tile visitors](https://github.com/storytold/photocraft/blob/6de37ce7f063e6814903650a24c8dad4936e8136/crates/ops/src/tiles.rs#L14),
  [selection owner](https://github.com/storytold/photocraft/blob/6de37ce7f063e6814903650a24c8dad4936e8136/crates/doc/src/lib.rs#L799),
  [effect cache](https://github.com/storytold/photocraft/blob/6de37ce7f063e6814903650a24c8dad4936e8136/crates/compose/src/lib.rs#L1334),
  `crates/paint/src/render_reference.rs`, root Cargo.toml / LICENSE-MIT / LICENSE-APACHE.
- **[A2] VectorCraft**:
  [region renderer and size preflight](https://github.com/storytold/vectorcraft/blob/540505fcad636b7983504ff7a1368a4a0d792803/crates/render/src/lib.rs),
  root Cargo.toml / LICENSE-MIT / LICENSE-APACHE.
- **[A3] FilmCraft**:
  [media_frame decode-error suppression](https://github.com/storytold/filmcraft/blob/6e0a6a65ed4a27ac0db1edba6aa833f798ec6a49/crates/render/src/lib.rs#L519),
  `crates/gpu/src/export_renderer.rs`, root Cargo.toml / LICENSE-MIT / LICENSE-APACHE.
- **[A4] DeckCraft**: pinned `storytold/deckcraft@be14248577b92059eb2b8c0c9460cceffbc1f01d`,
  retained `source/{Cargo.toml,LICENSE-MIT,LICENSE-APACHE}` and `source/crates/render/`.

This report approves a direction and proof tasks, not wholesale upstream code reuse,
complete format compatibility, managed-display correctness, or performance leadership.
