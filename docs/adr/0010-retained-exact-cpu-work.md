# Retain exact CPU geometry and layer tiles

Status: implemented and coordinator-verified, 2026-10-12. The combined increment
passes 153 workspace tests plus a renderer doctest, formatting, all-target Clippy,
native build, real GPU proofs and native edit/save/export smoke. Twelve public-seam
retention regressions pass. Release and isolated native observations are recorded
in [the retained-work report](../performance/retained-work-01.md); their scope does
not establish a general speedup or product parity. Default CPU reference/export
remains uncached.

## Decision and problem

The viewport evaluator flattened reached curves during preflight and again while
evaluating their color tiles. A new edit or camera request then reevaluated every
layer. Native request replacement already keeps revision/view identity, but it
does not retain completed work. The independently worded installed-Affinity report
establishes retained tiles, distinct camera/tile transforms and logical dirty
coverage; it does not supply a complete cache key or numerical reuse guarantee.
See [the sanitized view report](../research/affinity-view-pipeline-spec.md) and
[the source interaction findings](../research/native-interaction-bottlenecks.md).
No proprietary source or pseudocode was consumed.

Add opt-in `Renderer::new_retained(limits, cache_max_bytes)`. Clones share immutable
exact prepared geometry, evaluated layer tiles, proved empty layer results and
resource admission. The public additions are `retained_stats()` and
`clear_retained()`; Frame/RenderRequest/revision/cancellation interfaces stay the
same. `Renderer::new` is the fresh uncached oracle, ordinary export policy and
existing CPU benchmark path. No new dependency or backend interface is introduced.

Prepared geometry keys contain node identity, original path semantics, exact
effective placement and output dimensions. Paint does not affect this stage.
Preflight still validates fill/opacity/placement and configured limits. A cold
retained request uses a count/support pass and an emission pass to produce one
exact device-space path per reached node/placement, shared across its tiles.
The bounded per-request memo retains it even if the cache cannot admit residency.
The memo has at most 4,096 admitted slots; extreme slot or shared-byte pressure
falls back to the existing per-tile preparation, preserving exact output.

Layer tiles are retained **before** final root composition and page clipping.
Each key owns semantic records of the reached dependency tree: NodeId, immutable
content, transform bits, opacity/visibility, ordered operations/children, mask
source/mode/inversion and transitive source content. Original paths compare exact
verbs/point bits, with a fast path for the same pinned immutable path owner; fill
compares exact channel bits. Raster source identity combines a pinned immutable
original owner, dimensions and fixed source interpretation. Normal owner ancestry
and labels do not enter target-local mask evaluation. A group's ordered isolated
result is cached as a group, preserving group opacity and isolation.

There is no hash-only equality, pointer-only layer key, revision-wide invalidation
or retained whole Snapshot. Old recipes may match undo or another session only
when every actual semantic input matches. New local IDs alone never establish
equivalence. Final root order, page size/coverage and final RGBA8/F32 representation
are evaluated afresh; they therefore cannot reuse an obsolete final page buffer.
The internal layer representation remains the same f32 premultiplied linear policy
for both output formats. Frame identity still carries the current revision and
exact requested page/view/size/format for ordinary stale-worker rejection.

## Exact camera policy

Same placement/tile/dimensions can reuse any fully evaluated layer result.
Translation across tiles has a deliberately smaller proved domain:

- Both tiles must be complete canonical 256×256 coverage blocks. Partial edge
  blocks and alternative color scratch tiling miss for cross-camera reuse.
- Every reached line-only vector and crop must have identical coefficient bits
  for the **actual** `scale(8) * translate(-block_origin) * effective_placement`
  mapping. Their tiny-skia paths and clipped scanning domain are then identical.
- Every reached raster must have a diagonal inverse with signed powers of two
  between 2^-16 and 2^16, translations exactly on a 2^-17 grid, and bounded terms
  below 2^30. All source sample multiply/add operations are exactly representable
  in f64. Equal inverse linear coefficients and the first sample position prove
  the same source positions for all pixels. Other raster mappings miss.
- Child and target-local mask mappings obey the same rules recursively. Translated
  curves miss: output-space flattening arithmetic cannot be assumed invariant
  merely because the camera shift looks integral.

Coverage remains output anchored at 256, explicitly scanned at 8× and quantized
once to a byte; f64 curve flattening remains at 1/1024 output pixel. No camera
bucket, approximation, source decimation, magnified viewport, checker transform or
unfinished exact-frame claim is added. Arbitrary fractional pan/zoom may miss.
Only full-resolution evaluated pixels are reused; old displayed coverage and
newly revealed strips remain the native presentation module's separate work.

## Ownership and admission

Every retained allocation has leases against both the cache ceiling and the
shared renderer ceiling. Evaluated Buffer storage moves into immutable ownership
and transfers its local scratch lease to shared admission; pixels are never cloned.
Map slot capacity, semantic record/child/operation capacities, prepared geometry,
tile allocations and conservatively charged owner/control storage are admitted.
Canonical original vector geometry is conservatively charged with the document's
fourfold path-element metadata policy. These conservative metadata charges can
overlap across recipes; no allocator/RSS equivalence is claimed.

Original raster owners are interned through a separate bounded admitted weak
registry. Concurrent misses and different recipes/tiles share **one** capacity
charge for one actually retained immutable allocation. `RasterAsset::allocation_bytes`
includes caller spare capacity; pixels are never copied or hashed on every tile.
Weak control storage is conservatively included in registry admission. Active
evicted owners stay discoverable and charged. Clearing releases entry/map capacity
and expired source registry storage; an active borrower keeps its immutable owner
and charges until it drops. Cache and document source budgets deliberately overlap;
they are separate module admission policies, not a global process memory cap.

Native adoption should start with cache128MiB inside renderer256MiB. One tightly
packed 6000×4000 RGBA8 original is 96,000,000 bytes, leaving about 38MiB of that
cache allowance for exact viewport tiles and metadata. A 1024×768 F32 layer view
has 12MiB of tile pixels and 12MiB of separate final output. Geometry, scratch,
other sources, spare capacity, current output and concurrent jobs still count;
actual admission/refusal remains authoritative. 32/64MiB total cache allowances
cannot retain this original and must report truthful misses.

Pressure evicts retained entries before refusing required transient/output
admission. Cache map capacity and per-request geometry owners can be released for
uncached fallback. Owners held by a concurrent render cannot be falsely retired;
remaining pressure may truthfully refuse a request. Optional retention refusal
uses ordinary CPU evaluation. Arithmetic, cancellation and rendering errors are
propagated. A later frame cancellation/error may leave already complete immutable
semantic entries resident, but no partial Frame is returned. Immediate cancellation
populates none. Callbacks run outside the map lock.

## Verification and measurement

The authored `tests/retained.rs` crosses the same public seam as real callers. It
compares both RGBA8 and bit-exact F32 with separately constructed uncached renderers
through same-owner fill edits, one moved curve among64, cold fractional camera,
eligible grid translation, phase/linear changes, crop/exposure/opacity/path/mask
changes, hidden-source normal ancestry, root/group reorder, reparent/isolation,
visibility, undo/redo, page size/switch, dimensions, failed/canceled rendering,
source spare capacity, concurrent clones, active eviction and owner release.
All previous fidelity and refusal oracles remain required.

Root ran the serialized verification lane after the coordinated source batch was
frozen. `retained-camera-03` passed the full workspace and native build. The two
earlier attempts corrected test API/diagnostic compilation errors without changing
the exact-output assertions. Workers ran no formatter/build/check/test.

`examples/retained_workloads.rs` adds two bounded release probes: one moved element
among64 curves at512×512 and pan over one original24MP raster at1024×768. It uses
256MiB shared/128MiB retained admission, an initial exact warmup, and a fresh
uncached bit-exact oracle outside every measured render sample. Transactions,
setup, oracle and owner drops are excluded from the retained render interval.
CSV reports tile/geometry hits/misses, evaluated tiles, prepared curves, eviction/
refusal, source/pixel capacity, conservative metadata admission, total render
charges, output dimensions/format/bytes and oracle status. The photo path visits
13 bounded whole-block positions, then repeats; repeated-position hits must be
distinguished from newly entering strips. Root records hardware/build/RSS context
and distributions separately, using its exclusive build/measurement lane.

Dense final output writes, full native upload, GPU conversion and presentation
remain costs. Static saved-work counts establish reuse; only coordinator release
and native measurements can establish a latency improvement.
