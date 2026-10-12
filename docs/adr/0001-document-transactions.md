# ADR 0001: Persistent artwork snapshots and irreversible identity allocation

Date: 2026-10-12. Status: initial core implemented and coordinator-verified.
Strict nested geometry and owned staging follow-up is recorded in
[ADR0008](0008-strict-geometry-and-staging.md); that increment awaits its own
coordinated verification.

Verification: `scripts/verify --package affint-document --label document-core-03`
passed formatting, 24 scenario/resource/property tests, and Clippy for all targets
with warnings denied. Earlier integration fixes added test crate documentation
and corrected workspace lint priority. No performance claim follows from this check.

The document module must stage atomic changes, preserve editable original content,
publish worker-safe snapshots, and support undo, redo, branching, and coalesced
gestures without copying original image pixels. A corrupt imported relationship or
failed edit must never become visible artwork. The first vertical core contains
pages, ordered groups, filled Bézier paths, RGBA8 sources, local crops, exposure,
and masks; it does not claim a complete editor or interchange compatibility.

We chose `rpds::RedBlackTreeMapSync` for page, layer, and asset tables, with immutable
`Arc<State>` snapshots. The ordered tree is explicit: each layer records one owner,
and each page/group records its back-to-front child order. Changed child/operation
lists use `Arc<Vec<_>>` copy-on-write; large sources and geometry remain shared.
Map edits share unchanged tree structure. History stores state roots and labels,
so a failed batch simply drops its staged root. No generic backend interface is
introduced: there is one real representation and one mutation owner.

Two realistic alternatives were considered. `Arc<BTreeMap<Id, Arc<Layer>>>` with
standard-library copy-on-write would avoid a dependency and share content, but
every map change with a live snapshot would clone the entire map's entries and
tree nodes. That is an unattractive default for interactive edits and background
render snapshots. A mutable arena plus inverse-command journal would make basic
edits cheap, but introduces separate logic for mutation, rollback, undo, redo,
gesture cancellation, and stable snapshots; inverse operations can fail or omit
new semantic fields. Its smaller potential overhead does not justify duplicating
correctness logic before measurements. Persistent maps concentrate these rules
behind one validated staging path. No speedup is claimed without a benchmark.

Identities share one monotonic document counter across page, layer, and asset types.
The counter is outside reversible artwork and never rolls back. Accepted previews,
abandoned history branches, and even failed batches may burn IDs. Snapshots share
the allocation watermark; serializing an old snapshot therefore still records IDs
issued after its artwork was created. Import rejects zero/duplicate IDs across all
tables and any watermark that fails to exceed stored IDs. One session owns edits
to a lineage; coordinating simultaneous independent sessions is not yet supported.
Process-local revision tokens use a unique session namespace and monotonically
increasing events, including previews and restored artwork. They are not persisted.

Raster storage is an immutable `Arc<Vec<u8>>` inside `RasterAsset`: construction
moves the pixel buffer, clones share its owner, and no mutable accessor exists.
Pixel capacity is charged once per owning allocation, including sources held by
history and outstanding committed/provisional snapshots. Weak owner registries
avoid extending those lifetimes. Published states are similarly tracked so held
snapshots cannot escape metadata admission. Metadata is deliberately overcharged
per state, with persistent-map/Arc allowances; this is conservative admission,
not a measured allocator/RSS claim. Count/depth limits bound validation work.
History has entry and metadata budgets; oldest undo entries, then farthest redo
entries, are evicted after transitions. The staged state must fit alongside the
currently retained states before publication, even if a commit would later evict
some history. Storage must cap serialized input bytes before invoking its decoder;
an arbitrary Serde deserializer can allocate before this in-memory module sees it.

Validation rejects inconsistent tree ownership, missing references, non-group
children, and cycles in the combined child/mask dependency graph. Iterative
topological/tree traversal avoids input-dependent recursion, enforces depth,
and checks finite invertible local and composed tree affines. Dimensions, colors,
and shared geometry are validated by constructors and Serde conversion, while
raw editable opacity/operations are validated before publication. Import preflights
counts, name lengths, edge totals, path elements, operations, and source bytes before
building persistent tables. AFFINT-owned geometry Serde adapters enforce unknown
field refusal inside crop rectangles and every vector point, while Kurbo remains
the math representation. The original delegated geometry parser left a nested
unknown-field gap reproduced by the coordinator; ADR0008 records its correction
and the independent native acceptance oracle.

Rendering semantics are explicit. Content is evaluated, live operations apply in
stored order, then mask coverage and isolated layer opacity apply, then the affine
places the result in its parent. Groups compose children source-over before group
operations/mask/opacity. Crops alter local coverage and retain all source pixels;
exposure multiplies linear RGB by `2^stops`, leaving alpha intact. A mask source
uses its own content/operations/mask/opacity/affine in the target's local coordinates,
ignoring its normal ancestor transforms and normal visibility. Hiding the source
suppresses its regular tree painting without disabling the mask. The document
module owns data/invariants only; it contains no UI, filesystem, decoder, or GPU types.

The small interface is `Session::{new, from_archive, snapshot, transact, history}`
plus explicit begin/update/commit/cancel gesture methods. Gesture updates return
separate validated preview snapshots; the committed snapshot and undo/redo remain
unchanged until commit, and commit records one edit. Cancel retains the prior
artwork/history and reserves all preview identities. `Snapshot::archive` returns
a fallible editable record with shared original assets for storage to serialize.

Reuse audit, verified against primary registry/source documentation on 2026-10-12:

- [rpds 1.2.1](https://docs.rs/rpds/1.2.1/rpds/) and its
  [upstream source](https://github.com/orium/rpds): MIT; registry release
  2026-05-15; Rust 1.85; fully persistent maps with Arc-backed sync variants. The
  application exposes no rpds types. Deterministic ordered iteration suits archives
  and tests. The dependency's internal pointer machinery is distinct from this
  application's `forbid(unsafe_code)` policy; a complete transitive security/license
  audit remains the coordinator's responsibility once the graph is locked.
- [Kurbo 0.13.1](https://docs.rs/crate/kurbo/0.13.1) and its
  [manifest/source](https://docs.rs/crate/kurbo/0.13.1/source/Cargo.toml):
  Apache-2.0 OR MIT; Rust 1.85; maintained Linebender geometry, f64 Bézier paths,
  affines, finite checks, and Serde support. It preserves editable curve geometry
  rather than imposing a renderer's tessellation or flattening policy.
- [Serde 1.0.228](https://docs.rs/serde/1.0.228/serde/) and
  [thiserror 2.0.21](https://docs.rs/thiserror/2.0.21/thiserror/):
  MIT OR Apache-2.0. Serde converts validated scalar/content wrappers and rejects
  unknown document fields; thiserror keeps module failures structured. Serde's
  `rc` feature shares in-memory asset handles but cannot preserve cross-file pointer
  identity; reopening allocates new owners with exactly the original pixel values.
- `im` was not selected: its current
  [maintenance advisory](https://rustsec.org/advisories/RUSTSEC-2026-0250.html)
  on `im-rc` motivated checking the maintained rpds alternative. The choice is based
  on representation and maintenance, not an unmeasured performance comparison.

Not supported in this increment: text/fonts, strokes, blend modes beyond source-over,
profile-aware/float raster originals, vector booleans, raster painting, asset
collection/removal, page removal/reordering, unknown-content retention, persistent
history, simultaneous-session merging, or external document formats. These operations
are absent rather than enabled stubs. Renderer-specific numerical/output limits and
decoded-input byte budgets remain separate obligations. Next measurements should
compare edit latency, worst-case sibling reorder, full validation, history residency,
and outstanding snapshot retention on representative documents before changing the
representation or introducing incremental validation.
