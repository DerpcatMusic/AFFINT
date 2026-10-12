# Retained renderer independent source review, round 1

Status: frozen source independently reviewed; coordinator verification pending,
2026-10-12. No remaining concrete source defect was found in the reviewed scope.

Scope: `affint-render` public renderer interface, compositor, spatial proofs,
retained implementation, public-seam retained tests and retained workload harness.
The review consumes AGENTS/PRODUCT/CONTEXT/architecture/work-log, ADR0010 and the
independently worded installed camera and retained-invalidation specifications.
No vendor implementation, raw proprietary evidence or Affinity window was read
or manipulated. No product source was changed and no build, test, formatter,
benchmark, app, helper or desktop tool was run by this reviewer. Only this report
was authored. The app's separate presentation work is outside this round.

## Findings and corrective source

### P1 — Optional prepared geometry could prevent valid uncached admission

Corrective locations: `crates/affint-render/src/compositor.rs:130` prepared memo
allocation, `:193` whole-job admission fallback, and `:452`
`Evaluator::preflight_workspace` mandatory flattening-workspace admission.

The original retained path allocated a prepared-geometry memo unconditionally.
On whole-job pressure it cleared memo entries and residency, but kept the memo
Buffer and its lease alive. A request at the uncached renderer's valid byte
threshold could therefore be refused solely due to the optional slot allocation.

The renderer owner corrected memo allocation to become optional on allocation or
budget refusal and changed final job pressure to drop the whole memo before cache
clearing and retry. The authored
`tests/retained.rs:380`,
`optional_prepared_memo_does_not_refuse_the_uncached_admission_threshold`, derives
the admission threshold from a fresh uncached refusal, then requires uncached and
retained renderers with identical limits to return identical pixels and release
all owners. This is a meaningful public-interface regression, not an assertion
that duplicates a constant.

A related earlier route was identified: cached-prefix geometry can remain pinned
by the per-request memo while support/memo metadata has increased since those
entries were prepared. Mandatory flattening-workspace admission clears cache
residency but cannot release those memo owners through the original
`reserve_transient` helper. It can therefore fail before the final job fallback
is reached, although uncached preflight holds only one flattening workspace at a
time. Pure cold preparation itself holds that workspace while admitting each
geometry owner and consequently preserves its headroom; it is not the fixture
for this second route. The frozen evaluator-owned workspace fallback drops the
memo and retries mandatory admission. Pressure eviction at
`src/compositor/retained.rs:193` now skips borrowed entry Arcs, whose removal could
not release their admission charges, while explicit clearing still removes their
map handles and leaves charges with the borrower. This also preserves useful
completed geometry while an optional admission cannot progress.

The frozen cached-prefix test at `tests/retained.rs:404` primes complete geometry
before a deliberately late color error, adds hidden metadata and a guaranteed
cold visible suffix, and derives the fresh uncached threshold. It requires partial
priming, actual geometry hits and a positive `preflight_reclaims` count, followed
by exact output and complete owner release. If it passes coordinator execution,
those conditions establish that the intended mandatory preflight route occurred;
an all-hit scene alone cannot satisfy it. Both corrective paths and their authored
regressions have been independently source-reviewed. Tests were not executed by
this reviewer.

This concerns optional retention overhead. Live output or work borrowed by a
concurrent render must remain charged and can still cause a truthful refusal.

## Semantic and numerical review

Layer recipes (`src/compositor/retained.rs:561`) pin reached immutable content,
exact transform/fill/opacity bits,
visibility, ordered operations and children, mask source/mode/inversion, and
transitive raster owners. Recipe equality walks the current reached dependency
tree instead of relying on a revision, hash or NodeId alone. Target-local masks
follow the same transform rule as the evaluator; normal mask-source ancestry
is deliberately excluded. Group results include isolated child composition and
group-level operations, masks and opacity. No complete Snapshot is retained.

Final root composition, root order, page clipping and output representation are
reevaluated. Layer tiles can consequently be reused after root reorder without
reusing an obsolete final page buffer. Page size and RGBA8/F32 conversion are
outside the retained layer identity because they are applied freshly afterward.
The authored tests compare these paths with separate fresh uncached renderers,
including shared-path fill changes, masks, operations, group/root ordering,
reparenting, isolation, visibility, undo/redo, page size/switch and dimensions.

Prepared curve geometry compares the exact path owner/verbs and effective
placement, with output extent included because its support proof uses that
extent. Fill-only changes can retain shared geometry while invalidating paint;
the shared-path/different-fill regression checks the original path pointer and
fresh output. A bounded per-request memo carries cold derived geometry across
color tiles. Two traversals count then emit the prepared path; the reported
prepared-curve count is not a claim of one source-element traversal.

Cross-camera tile reuse (`src/compositor/retained.rs:580` and `:621`) is restricted
to complete output-anchored 256-by-256
coverage blocks. Line/crop reuse compares the actual 8x coverage affine's exact
coefficient bits. Raster reuse admits only the diagonal power-of-two inverse
domain, exact translation grid and bounded multiply/add terms before comparing
the first absolute sample and linear coefficients. Child and target-local mask
mappings are checked recursively. Curves, partial edge blocks and unproved
fractional/linear changes conservatively miss. The inspection found no approximate
camera bucket or hidden source/output decimation in this path.

The raster proof is a source arithmetic argument, not a measured result. The
authored grid-pan oracle exercises full blocks and rejects phase, scale and shear
changes; it compares exact RGBA8 and bitwise F32 outputs. Existing independent
fidelity goldens remain required because cached and uncached renderers share the
underlying compositor.

## Ownership and concurrency review

Retained tile pixel storage moves from scratch into immutable ownership without
copying the full allocation. Data fields precede charges; shared renderer/cache
leases follow the actual retained owner after map eviction. Final Frames retain
separate output leases. Recipes charge record, child and operation capacity and
conservative vector-owner metadata. Geometry, pixel buffers and bounded map
capacity are admitted by bytes.

Raster originals (`src/compositor/retained.rs:442`) are interned through an
admitted bounded weak registry, keyed
by a pinned immutable allocation and dimensions. Spare pixel capacity is charged.
Concurrent recipes can share one Source charge for the same actual allocation;
evicted active owners remain discoverable. Weak-control storage is conservatively
charged by registry slots. Pixel owners are never copied into keys.

Entry lookup holds the map lock only to select/clone a candidate; semantic checks
and cancellation callbacks occur afterward. Source interning holds its separate
lock and may evict map entries, while `clear` releases the map lock before pruning
the source registry. Source/geometry/tile destructors do not acquire those locks,
so the inspected paths do not form a reverse lock acquisition pair. Candidate
touch verifies owner identity before updating a potentially reused slot.

Authored public tests cover concurrent clones, one spare-capacity source charge,
active tile eviction from a cancellation callback, owner release, session drop
and distinct same-local-ID sources. These are reviewed source oracles, not tests
executed by this reviewer. Admission counters and residency diagnostics are
atomic observations, not a coherent process RSS measurement.

## Workload scope and remaining limits

The workload harness uses 256 MiB shared renderer admission and 128 MiB retention
for one moved element among 64 curves, and pan over an original 24 MP raster with
a 1024-by-768 viewport. Fixture creation, transactions, separate uncached bitwise
oracles and Frame drops sit outside each retained render interval. CSV distinguishes
hit/miss/evaluated/prepared counts, original capacity, retained capacity, metadata
admission, output format/dimensions/bytes and charges. The 13-position photo path
then revisits positions; repeat-position hits must not be presented as only
newly entering-strip reuse.

The source still writes a complete dense final output. Native upload/conversion/
presentation is a separate cost and is not timed by this harness. Arbitrary zoom,
fractional pan and translated curves can miss; this increment does not eliminate
all possible whole-viewport evaluation. Exact-work reuse counts do not establish
interactive latency, release speedup, maximum cache suitability or commercial
workflow parity. Only coordinator checks and recorded release/native evidence can
qualify those claims.
