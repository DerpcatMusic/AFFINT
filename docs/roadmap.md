# Ongoing implementation roadmap

Completion is the unified product requested by the owner, not a particular scaffold.
Each meaningful increment includes an evidence-based improvement pass. Continue
until explicitly stopped; update this record as actual behavior lands.

1. Shared document: immutable source assets, persistent scene relationships,
   validated transactions, non-destructive operations, stable identity, gestures,
   bounded undo/redo and strict archive admission.
2. Faithful canvas vertical slice: import an original image/vector fixture, select
   and move layers, reorder, mask, crop, adjust exposure, undo/redo, save/reopen,
   and export at the requested resolution. Native familiar editor shell.
3. Acceleration: profile the complete slice, implement GPU residency/dirty-region
   work and accurate accelerated evaluation, then evaluate SIMD where measurements
   justify it. Fix bottlenecks rather than reducing image quality.
4. Rich vector workflow: paths, handles, strokes, shape construction, editable
   booleans, snapping, alignment and robust vector interchange.
5. Photo workflow: mask refinement, channel/luminance workflows, live adjustments
   and filters, painting, original RAW/develop controls and color management.
6. Layout workflow: pages/artboards, typography, threaded text, styles, linked
   assets, print/color output and document packaging.
7. External fidelity and resilience: real-file corpora, original black-box reference
   fixtures, malformed-input/worker/save failures, portability, long-session tests,
   accessible controls and benchmark regression gates.

The initial bounded history is session-local and discards redo entries on a new
branch. Saved history and alternate futures are documented Affinity behaviors;
add durable, bounded history with explicit branch retention before claiming those
workflows. Undo retention has finite resource limits and is not an unlimited archive.

Order adapts to research and demonstrated dependency bottlenecks. A feature is
complete only when its real workflow, persistence, undo and output are verified.
Do not create a catalog of fake enabled controls to simulate this roadmap.

## Current capabilities

| Workflow | Verified scope | Remaining work |
| --- | --- | --- |
| Atomic document edits | Shared original assets, acyclic layers/masks, transactions, undo/redo, one-edit gestures, consumed edit payloads and strict geometry; 41 tests | Durable history/branches, richer content and incremental validation |
| Exact CPU rendering | Linear-light composition, filled curves, affine raster sampling, opacity, alpha/luminance masks, crop and exposure; typed RGBA8/F32 output; 51 tests plus a doctest, including retained geometry/tile oracles | Broader exact camera reuse, GPU evaluation, advanced blends/filters and color management |
| Editable native files | Atomic replacement, bounded metadata, deduplicated exact sources, corruption/unknown-geometry refusal and bit-exact f64 corpus; 16 storage tests | Saved history and external interchange; broader malformed-file corpora |
| PNG import/export | Bounded supported precision, explicit sRGB policy, exact exported dimensions and original sample fixtures | ICC, 16-bit, HDR, orientation and animation |
| Native MUI editor | Page creation/navigation, page-scoped editing/import, captured-page export; F32 upload/display and callback-retired publication; 45 tests and real smoke/GPU checks; native-density camera, coherent selection/picking and bounded overscan | Broader native pointer/dialog/IME/accessibility qualification and workflows |
| Vector/photo/layout breadth | First filled-shape and live-operation workflows | Path/stroke/boolean tools, arbitrary mask editing, painting, typography and page-layout workflows |
| Performance | Conservative CPU optimization and separate F32/RGBA8 observations; F32 p50 24MP2.54s/48MP5.05s with explicit output budgets; retained distributions/RSS/lease evidence | Genuine GPU document evaluation, broader measurements and comparisons |
