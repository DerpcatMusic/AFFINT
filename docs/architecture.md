# Architecture direction

Status: retained-renderer/camera increment passes153 tests plus renderer doctest,
formatting, Clippy and native build; real GPU and command/persistence smoke pass,
2026-10-12. This is an early editor, not complete product/UI parity.

The earlier F32 working-output checkpoint passed the full120-test workspace and real
GPU/editor smoke. CPU evaluation produces one leased typed representation; native
display uses a fixed uploader and private GPU candidate. Actual owner-drop callbacks
retire texture charges asynchronously, with explicit acknowledgement in diagnostics.
See ADR0007/0009 and the work log for evidence and separate resource partitions.

The document module owns validated content, relationships, immutable source asset
references, transactional edits, and history. The renderer consumes committed
snapshots; storage serializes and validates them; the UI submits edits and presents
results. No UI type enters the document module.

The initial seam is intentionally small: inspect a committed snapshot, transact
one edit, and traverse undo/redo. The transaction validates its staged result before
publishing it. Generational/monotonic identities and revision tokens must survive
history branching without collisions. Relationships remain acyclic.

Rendering is a separate workload from chrome/UI drawing. A toolkit selection must
show how full-resolution raster tiles, vector coverage, masks and live effects reach
the display without a repeated GPU→CPU→GPU round trip. A correct CPU reference and
explicit color/precision contract precede accelerated optimizations. Viewport
sampling is a display operation, not permission to reduce document resolution.

Storage must use atomic replacement and complete validation. Original source assets
and unknown imported content remain recoverable. Undo is not implemented by
duplicating every asset, and save is not implemented by flattening the document.

Planned module responsibilities, subject to research: document, rendering, native
storage, native app shell, and headless workflow/verification. Add a separate crate
only when it earns locality, reuse, or a real seam; avoid speculative wrapper crates.

The document representation is Arc-owned immutable state with rpds persistent
ordered maps, explicit child order, and one validated transaction/gesture seam.
Its allocator watermark is deliberately outside undo. See ADR0001.

The reference renderer evaluates bounded tiles in f32 premultiplied linear sRGB.
tiny-skia supplies vector/crop coverage rather than 8bit artwork color composition.
Frame ownership retains the renderer's budget lease. The native UI scheduler must
cancel obsolete jobs and reject results whose revision or view request is stale.

Native storage streams exact sources beside a bounded strict JSON manifest,
deduplicating identical sources and validating checksums before graph publication.
Same-directory staging/sync/replacement returns both the saved revision and the
directory durability outcome. See ADR0004. Unsupported content is currently
refused; preservation of external unknown content is not implemented yet.

The selected native shell is Matari UI at pinned commit
c33c4451e8814562997171a1e00e9fb41fa955e6, Vello Hybrid 0.2.0, wgpu 29.0.4
and registry winit 0.30.13. AFFINT inserts an external document texture before
MUI chrome in one Hybrid scene/presentation. Artwork does not use image-fill atlas
fallbacks. GPU color/clip/ordering proof passed on RX6600/Vulkan; native observations
cover 150% scaling, menus, keyboard clipboard and protected discard.
The earlier egui/wgpu30 choice is a fallback, not a second active stack.
Host and toolkit-specific logic belong in the app; no UI dependency enters the core.

The first document evaluator is CPU reference, uploaded for a newly accepted
revision/view. An independent GPU display pass composites the checker/artboard in
linear light and encodes the final display. Ordinary presentation has no GPU
readback. Full GPU document evaluation remains work. See ADR0005 for lifecycle,
precision, resource partitions and recovery limits.

Render jobs are bounded and replaceable. Publication and UI acceptance carry
revision, exact view, serial and device generation. Output ownership and its budget
lease persist across handoff. Graphics resources awaiting completion remain owned
and charged; cancellation is separate from retirement. File and OS clipboard work
use fixed workers with bounded mailboxes. Idle windows wait for events or deadlines.

Application admission must partition and coordinate document sources/history,
load/decode staging, output/scratch, UI textures/atlases, and GPU jobs. Module limits
are currently separate and cannot honestly be described as a global process RSS
cap. Source pixel ownership is shared; UI and worker code must retain each output
lease while its pixels are live and must not evade budgets through buffer copies.

Pi binary investigators keep raw vendor output under ignored research storage and
publish independently worded contracts with evidence and unknowns. Implementors
consume sanitized specs and audited open-source material. A lock is not proof of
revision freshness; cancellation is not proof of GPU completion. AFFINT verifies its
own publication and ownership invariants independently of those research findings.

After native display proof, release workloads guide the next improvement. The
harness separates setup, admission probes, transactions and exact rendering.
Measured RSS is separate from module admission; small-sample p50/p95 are observations.
Optimization must preserve the same fidelity oracles, immutable originals and
bounded owners. Saved history/branches, advanced vector/photo/layout, broad
interchange, color management and native accessibility remain roadmap work.

The active-page increment preserves one document/session and makes PageId explicit
in selection, import and captured export. All-page native save remains independent
of transient active-page preference. Strict geometry adapters keep the pinned
Kurbo representation while refusing unknown members; constructors validate before
canonicalizing. Typed transaction payloads move into staging, and the native codec
uses correctly rounded f64 parsing. These corrections pass independent persistence
probes and the full workspace; ADR0006/0008 record their exact scope.

The next bounded GPU evaluator is still a reviewed plan, not an enabled backend.
It starts with one original raster and an optional raster mask, shared CPU coordinate/
coverage preparation, explicit numerical admission and whole-request CPU fallback.
Source residency must account for retained allocation capacity as well as GPU bytes;
result leases must survive every later sampling submission. Headless independent
precision/ownership gates and paired cold/warm costs precede native adoption.
The open arithmetic, packet-bandwidth and budget questions are recorded in
docs/research/gpu-first-slice-plan.md.
