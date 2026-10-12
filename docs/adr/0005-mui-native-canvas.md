# ADR 0005: MUI chrome and an application-owned native canvas

Status: implemented first slice; coordinator compilation, 13 app tests, Clippy and
headless document smoke verified. Full GPU seam/display proof and advisory audit
verified; corrected native menu and graceful dirty-discard close confirmed.
Active-page increment passes the full workspace and 19 app tests, multipage smoke,
GPU proof and native page inspection at both required sizes. See ADR 0006.
The F32 upload/publication replacement below passes coordinator
`linear-f32-native-05` (24 app tests, full workspace120 plus doctest) and real GPU/
editor smoke `linear-f32-runtime-02`. Earlier RGBA8 evidence remains historical.
ADR0009 records the callback-acknowledgement correction and exact proof scope.
Date: 2026-10-12.

The owner prefers Matari UI and a compact unified native desktop editor. The source
audit in `docs/research/mui-fit.md` establishes a public same-device texture seam at
MUI commit `c33c4451e8814562997171a1e00e9fb41fa955e6`. The earlier egui decision is a
fallback, not a parallel implementation. Framework responsiveness does not establish
imaging speed or parity with Affinity.

AFFINT owns its winit `ApplicationHandler`, safe `Arc<Window>` surface, wgpu device
and queue, native file policy, input edges, system clipboard, command scopes, and
worker scheduling. MUI owns intrinsic chrome layout, text fields, ordinary widgets,
hit surfaces, focus, IME caret geometry, and UI painting. Document, rendering, and
persistence interfaces contain no MUI types or generic toolkit adapter.

## Display ordering and color

The ordinary document evaluator remains the exact CPU reference. Native requests
explicitly select linear-premultiplied F32 output, including that format in revision,
page, view, serial and device-generation acceptance. One fixed upload worker borrows
the leased frame into two reused 8 MiB row-band buffers and an exact RGBA32Float
source texture. Accepted presentation retains metadata and its GPU pair, with no
CPU frame or full-frame pixel clone. Explicit PNG export selects straight sRGB RGBA8.

An AFFINT WGSL pass uses nonfiltering `textureLoad`, composites working RGB over
the linear checker/artboard, clips the final display range and encodes an opaque
RGBA8 non-sRGB UNORM display texture. Highlights survive until checker composition.
The renderer canonicalizes exactly zero-alpha RGB; the float shader consumes RGB
unconditionally because an arithmetic alpha comparison may flush a positive
subnormal alpha carrying normal HDR RGB. No nonlinear transfer is applied to
premultiplied channels. The legacy RGBA8 and synthetic RGBA16Float modes remain
explicit diagnostics. See ADR 0007 and ADR 0009 for the replacement contract.

The host resets Hybrid's scene once, appends the clipped physical document sample
using `SampleRect`, binds its view with `TextureBindings::insert`, pops the clip,
then appends transparent-root MUI chrome and selection overlays through
`mui::vello::paint(Affine::scale(dpi))`. Hybrid renders once and the surface is
presented once. The document is below all chrome and floated menus/modal shields.
Hybrid's mandatory clear therefore cannot erase a separately drawn document.

Document output never passes through MUI `Fill::Image`, whose atlas pipeline can
substitute a solid fill for a refused image. Surface formats are restricted to
`Bgra8Unorm` or `Rgba8Unorm`; an unsupported surface is an explicit startup error.
The display texture is already sRGB encoded, so an sRGB surface would encode twice.
The resolved logical viewport's edges are snapped once into physical coordinates;
the exact render affine includes the snap-origin displacement.

Ordinary rendering performs CPU-reference-to-GPU upload and GPU display conversion.
It performs no GPU readback. It is **not a fully GPU accelerated document renderer**.
`--gpu-proof DIRECTORY` separately permits diagnostic readback and fails explicitly
if it cannot obtain an adapter. Its half-float fixture includes a >1 highlight,
translucent coverage, and zero alpha; it compares display conversion against a
linear-light reference within 1/255, then verifies external texture preservation,
physical clipping and MUI-over-document ordering. Artifacts and actual adapter
details are emitted for coordinator review.
The expanded proof adds real-document F32 exposure, canonical zero alpha,
subnormal-alpha HDR, target-local mask/group isolation, and a nonuniform 1281×1100
document requiring three padded upload bands. It compares independent scalar f64
working oracles within 3e-7 for these fixtures and actual opaque display within
1/255 on both checker cells. Same-worker publication/cancellation/retirement probes
exercise the ordinary interfaces; artifact generation is not yet passing evidence.

Coordinator runtime evidence at
`.research/native-editor-evidence/second-runtime-02/gpu` passed on AMD Radeon
RX 6600, Mesa RADV 26.2.4-arch3.1, Vulkan. Maximum display error was 1/255. Resolved MUI
chrome was `(28, 0, 10, 32)`; the legitimate overlap sample `(33,16)` was exactly
`[25,51,204,255]`. External document samples were preserved and no clip leakage
occurred. An earlier fixture used implicit sizing/placement and a hardcoded sample,
then failed its overlap assertion; corrected explicit start anchoring and sampled
resolved geometry verified the same production ordering without shader/display
changes. Failed proof images and observations now persist before assertions.

## Scheduling and ownership

The render workload has one running job, one replaceable pending snapshot, and one
completed result. Replacement increments a checked atomic serial and cancels
obsolete evaluation through `render_with_cancel`. Publication rechecks that serial
under the mailbox lock. The UI also checks revision/view serial and device
generation. Stale frames are dropped with their renderer lease.

File operations use one fixed worker and bounded request/result channels; a new file
command is refused while occupied. Native rfd dialogs, decode, load, save and exact
export run away from painting. A second fixed worker keeps the real arboard
clipboard owner alive. Paste tickets include the focused field, selected node and
document revision. Clipboard and IME text are limited to 4096 bytes before MUI
shaping; the OS clipboard library can allocate its initial returned string before
AFFINT checks its length. Native startup/device rebuild uses at most one worker at
a time. Upstream panics at workload seams become actionable errors and wake the UI.

Pointer motion is coalesced into one pending snapshot. Button, key, text and IME
edges flush prior motion and are processed immediately. A bounded second UI frame
consumes MUI widget responses even when input stops after a release. MUI capture is
retained through native release; focus loss, Escape and occlusion cancel document
gestures. Pixel wheel deltas divide by DPI; line deltas are already logical units.
Settled windows use `ControlFlow::Wait`, with `WaitUntil` for MUI deadlines/recovery.

Resource partitions are explicit rather than a claimed global RSS cap:

- Document sources: 256 MiB; metadata: 96 MiB; retained history metadata: 48 MiB.
- Shared renderer output/scratch lease budget: 256 MiB, including current, queued,
  and concurrently exported frames; exact output limit: 24 million pixels.
- PNG input: 128 MiB encoded, 24 million pixels, 64 MiB decoder workspace. Native
  load validates against the document policy; old artwork remains live until a
  complete replacement is accepted, so replacement staging is additional.
- Native CPU results: at most two pixel owners, one with the uploader and one in
  the render worker's running/completed slot. Busy upload leaves completion in its
  mailbox. Export may add an RGBA8 owner only within the shared byte budget.
- Canvas source/display textures: 320 MiB and a hard two-pair ceiling, charged
  before allocation. Current plus private candidate or retired pair consumes both
  slots. The prior image survives refusal, obsolescence, upload/conversion errors
  and surface retries. Promotion follows validated conversion and Hybrid submit;
  retirement follows the final GPU submission including Hybrid sampling.
- Explicit upload staging: two fixed 8 MiB MAP_WRITE|COPY_SRC buffers, 16 MiB
  total, with 256-byte-padded rows. One upload worker performs the pixel copies,
  GPU waits and remapping. Its finite poll services retirement even during UI idle.
  Device replacement cannot duplicate the pool while old buffers remain charged.
- MUI image atlas: lazy, one 512×512 RGBA8 atlas (at most 1 MiB); MUI intermediate
  layers: four at most 1024×1024 RGBA8 textures (at most 16 MiB). The actual public
  atlas texture size is logged. Surface/backbuffer allocations and Hybrid's private
  alpha/paint/LUT caches are separate; no total GPU or process hard cap is claimed.
- Visible layer rows are shaped in a bounded window; spacer geometry preserves the
  scroll extent. Layer/name count limits also bound traversal and UI text admission.

## Working slice and deliberate limits

The native shell supplies a single document viewport with active-page navigation
and atomic blank-page creation, original synthetic
artwork, real new/open/save/save-as/import/export commands, layer selection,
continuous move, reorder/remove/visibility, rectangle/ellipse creation, editable
fill, crop, exposure, opacity, an editable hidden ellipse mask, undo/redo and
pan/zoom. Native files preserve supported operations and original source bytes.
Background save records the saved revision; newer edits stay dirty. Pending rename
text is committed before save, selection changes, close or replacement.

Dirty replacement/close offers Save, Discard and Cancel. It gates underlying
commands, pointer hits and keyboard focus. Failed decode/render/file jobs preserve
the prior valid document/frame. GPU timeout/outdated/lost-surface recovery is bounded;
device loss recreates device-local state and invalidates generation tickets.
If save-and-close publishes the file but directory synchronization actually fails,
a protected Keep open / Close anyway decision keeps that failure visible before
exit. An older saved revision cannot finish a pending close. `PlatformUnverified`
is separately reported policy; the file may be published without a verified
platform directory-sync implementation.

Panels are fixed in this increment. Multi-document tabs, page name/size editing, docking,
full accessibility bridge/conformance, advanced typography/layout, arbitrary masks,
ICC/HDR/color management, GPU document evaluation, broad interchange and measured
performance remain work. MUI semantic roles and keyboard focus are used, but no
AccessKit native adapter is currently shipped. Opening a supported multi-page file
preserves every page; the first page is initially active. Navigation scopes layer
commands, viewport and full-page PNG export to the chosen page. Queued export
captures the selected page/request with its snapshot before the native dialog.
See ADR 0006 for undo/redo reconciliation and verification status.
Enabled controls execute real commands; unsupported tools are absent.

## Reuse and notices

MUI is a git revision dependency rather than a mutable local path or its complete
plugin workspace. Its MIT notice is retained under `crates/affint-app/licenses`.
The compatible family is wgpu 29.0.4, vello_hybrid/common 0.2.0, registry winit
0.30.13 and Kurbo 0.13.1. There is no egui UI runtime or wgpu 30.
Bytemuck 1.25.2 is pinned with default features disabled for fallible borrowed
`[f32;4]` to byte views. Its MIT notice is retained; AFFINT introduces no unsafe
code, detached allocation or serialization copy. ADR 0009 records this reuse.

The winit dependency disables defaults and explicitly enables `rwh_06`, `x11`,
`wayland` and `wayland-dlopen`. This removes its optional Adwaita client-side
decoration font chain (`sctk-adwaita` → `ab_glyph` → `owned_ttf_parser` →
`ttf-parser`), which carries the unmaintained advisory
[RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html). No advisory
ignore was added. The pinned primary manifest and
`src/platform_impl/linux/wayland/window/state.rs:47–50` show that winit uses
Smithay's built-in `FallbackFrame` when Adwaita is disabled. Its 24px header and
border retain move, close, maximize/minimize and resize actions without the old
font dependency. Client-decorated Wayland windows lose Adwaita's styled/text title
rendering; compositor/server decorations and Windows/macOS native code remain.
This is an explicit titlebar styling tradeoff, not removal of Wayland support or a
claim of native behavior testing on every compositor. The coordinator regenerated
the lock to 398 packages and the serial advisory audit passed without ignores
(`.research/security/native-mui-20261012-r2/result.json`).

`rfd` 0.17.2 (MIT) supplies native dialogs. Its pinned manifest defaults to
`xdg-portal` and `wayland` with pollster; older async-std/tokio feature guidance does
not apply. The primary source was checked at
https://github.com/PolyMeilex/rfd/tree/0.17.2. The Linux portal/fallback environment
is used as installed; AFFINT installs no user software or configuration.
`arboard` 3.6.1 (MIT OR Apache-2.0) supplies system text clipboard, with optional
Wayland data-control support. Platform behavior remains to be verified.

`epaint_default_fonts` 0.34.3 is a bytes-only font dependency. Only Ubuntu Light
is used; its Ubuntu Font Licence is retained. No egui UI is imported. Icons, logo,
WGSL and synthetic artwork are independently authored AFFINT material. No vendor
artwork or proprietary implementation is incorporated.

## Verification at this increment

`mui-native-workloads-05` passed formatting, all 13 app tests, the 21 renderer
integration tests and doctest, all-target Clippy with warnings denied, and a normal
binary build. Tests exercise old-save dirty behavior, canceled/atomic gestures,
stale frame lease disposal, revision/view/device/serial acceptance, wheel DPI units,
zoom anchoring, actual MUI copy/cut handoff, custom keyboard activation/focus and
save-and-close durability decisions.

The built-binary smoke workflow preserved source SHA-256
`cb9e6b985ae6f1f8445ec3dcb8a16dcfe147bf69272426f9dd1f94ba33a929a8`, exercised exposure,
crop, mask, opacity, move/reorder and undo/redo, saved/reopened an editable file, and
produced identical exact 960×720 output after reopening. This is a correctness
workflow, not an external native-file compatibility or performance claim.

Owned native-window captures at 1024×720 and 1440×940 logical sizes at 150% DPI
showed artwork and compact panels. Real OS Ctrl+A/C/X/V clipboard roundtrip and
dirty-New modal shielding/Escape preservation passed. One floated menu expanded to
window height and had misplaced origin; its explicit start anchor and resolved
trigger placement were confirmed at 1024×720 logical size/150% DPI. Escape closed
the menu, and the native dirty-discard close path exited gracefully with status 0. A settled
focused window showed zero CPU ticks over one five-second observation; this is
limited idle evidence, not a latency/tail/performance result. Native pointer tools,
portal dialogs, IME composition, accessibility and other compositors/platforms are
unverified.
