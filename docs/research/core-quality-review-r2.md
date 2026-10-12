# Independent document/storage quality review — round 2

## Result

**No new meaningful defect found in this bounded review. No new follow-ups.**
The round-1 staging issue and two subsequently reproduced native fidelity defects
are closed in the scopes described below. The malformed-path constructor regression
also has a source correction and passing coordinator evidence. This is not a
blanket proof of all geometry, allocation, numeric, or filesystem behavior.

This worker ran no build, Cargo/rustc command, check, test, benchmark, helper
executable, or runtime probe. No nested agents, production edits, installation,
or configuration changes occurred. Only this report was written. Coordinator logs
are **existing execution evidence read by this reviewer, not independently rerun**.
No new unexecuted probe files are needed for the result.

## Frozen source and inspection

Reviewed checkpoint **`dcc02691b9b9ef1a2ed9f5418f285c4a72067b34`**. Git tree IDs:

- `crates/affint-document`: `212701c5256190efc61fc0307ec9ffe6602c37e6`
- `crates/affint-storage`: `a61d43e8aa8847ee1784b2a34f9e60993c28728d`

The scoped working-tree diff against that checkpoint was empty at the start and
at the final source check. Line spans below refer to that checkpoint. Concurrent
renderer/app work is outside this review; its changes and artifacts were preserved.

Read every production Rust file in the two crates, their manifests, the new
`strict_geometry`, `owned_staging`, and `native_fidelity` regression sources,
`native_archive`, and selected original resource/gesture/identity scenarios.
Also read AGENTS.md, PRODUCT.md, CONTEXT.md, architecture, work log, the prior
review, and ADR0001/0004/0008. Applied local Rust ownership/error/testing guidance
and deep-module/interface vocabulary. No raw vendor output or pseudocode was read.

## Positive closures

### Strict nested geometry and malformed typed paths

**Source facts:** `crates/affint-document/src/geometry_serde.rs:15–36,41–65`
selects strict remote definitions for rectangles and every point slot of each
supported path variant. These adapters are actually connected by
`src/model.rs:312–316,428–434`; the surrounding archive/layer/vector records also
refuse unknown members. Path serialization visits borrowed elements rather than
copying a whole path (`geometry_serde.rs:69–94,151–156`).

The sequence visitor checks a nonempty path's first element before its initial
`BezPath::from_vec` call (`geometry_serde.rs:110–132`). Both the public typed
constructor and deserialized `VectorInput` then use the same borrowed finite and
subpath validation before making the canonical path (`model.rs:324–348,361–367`).
This distinction matters: the adapter guards the initial constructor precondition;
the shared factory performs the full semantic check before canonicalization.
Drawing after a close requires a new `MoveTo`; empty paths remain supported.
No panic-catching fallback replaces validation.

**Public-interface acceptance expectations:** A supported native crop must still
load. Adding a new member inside its rectangle, while recomputing the manifest
length, total length, and checksum correctly, must instead fail structural decode;
it must not load and lose that member. The equivalent mutation in any supported
path point must be refused. `VectorContent::new` given a path whose first element
has been changed to `LineTo` must return a document error, not panic. Valid empty
and explicitly restarted paths must remain accepted.

**Existing execution evidence:** The unchanged native crop oracle now passes.
All nine `tests/strict_geometry.rs` cases pass in the coordinator's round-03 log,
including every point slot, duplicate fields, supported literal/Kurbo wire shapes,
malformed serialized starts/closed continuations, and typed malformed/nonfinite
paths. Round-02's log records the earlier `BezPath must begin with MoveTo` panic;
it is not being concealed as a passing run. No remaining concrete nested-member
or malformed-typed-path acceptance defect was established here.

### Bit-exact native page-width regression

**Source facts:** `crates/affint-storage/Cargo.toml:13` explicitly selects
`serde_json = 1.0.154` with `float_roundtrip`. Native text still crosses that codec
at `src/wire.rs:108,160`, so the feature addresses the actual persistence path,
not only a JSON-Value comparison or document test dependency.

**Public-interface acceptance expectation:** Encode and decode an otherwise empty
document with width bits `0x3ffd2c606cb79d18`; its reopened width must have the same
bits. The deterministic 4,096-case corpus in `tests/native_fidelity.rs:94–137`
checks this through `Store::{encode,decode}`, using ordinary positive widths in
`[1,2)` and reporting the first differing bits and manifest.

**Existing execution evidence:** The old log records case 0 drifting to
`0x3ffd2c606cb79d19`; round 03 records the corpus passing. A source diff against
`.research/core-quality-review/storage_oracles.rs` shows only introductory
comments and formatting changes in the registered native oracle: neither its
corpus nor its acceptance assertions was weakened. The reproduced width defect
is closed. This bounded corpus does not prove bit preservation for every possible
finite parameter, exponent range, or separate caller-selected JSON codec.

### Consumed operation owners without the old bulk staging clone

**Source facts:** `crates/affint-document/src/session.rs:535–576` consumes the edit
and passes owned changes to staging. `src/staging.rs:12–23,85–100,204–235` moves
`LayerSpec` and `SetOperations` payloads into candidate owners; the old whole
operation-vector clones are absent. Existing layer/state copies retain shared
payloads. Checked live-element bytes gate spare-capacity shrinking. Larger owned
inputs move unchanged, rather than requesting the old second oversized vector.

Actual resulting operation capacity remains charged by
`src/validate.rs:139–147`. Full graph, state, retained-source, and retained-metadata
admission runs before candidate registration/publication. A superseded input may
be large without making the final state large; this is the documented sequential
batch policy, not an accepted-state byte-limit bypass.

**Public-interface acceptance expectations:** Under a 32 KiB metadata policy,
a final 32,768-operation replacement must fail without changing artwork, revision,
or either history side. Failed insertion may burn an ID, but the next successful
insertion must not reuse it. A large early replacement followed by a small final
replacement may succeed. A small live list with large spare capacity must still
fit only if its resulting capacity and all retained charges fit. A failed gesture
update must preserve its last accepted preview and eventual one-edit undo target.

**Existing execution evidence:** All eight `tests/owned_staging.rs` cases pass,
covering those semantics and payload identity across insertion, replacement,
preview, publication, undo, and redo. Absence of the extra bulk clone is a source
observation; pointer comparisons are owner-identity evidence only. They establish
no allocator-request timing, OOM immunity, global RSS bound, or measured speedup.
Strings, headers, relationship copy-on-write, and deserializer allocations have
not become allocation-free or globally budgeted through this correction.

## Other examined publication and ownership rules

- `Session::stage` validates/admit-checks the candidate before `transact` changes
  history/current artwork (`session.rs:376–398,535–579`). Gesture updates likewise
  stage before replacing the preview; commit publishes once and cancel leaves
  committed content/history intact (`473–524`). No new partial-publication defect
  was found after moving the payloads.
- Identity allocation remains outside undo and shared with snapshots. Archive
  watermark capture, import duplicate/watermark checks, and monotonic allocation
  remain connected (`session.rs:146–190,277–321,767–781`; `validate.rs:159–164`).
  Failed/canceled work burning IDs is intentional, not reuse or rollback failure.
- Weak registries still account for live source owners by pixel allocation and
  retained states without themselves extending those strong lifetimes
  (`session.rs:650–764`). Capacity charging and source sharing remain in place;
  no new byte-owner escape was established.
- Native decode checks framing, manifest/source digests, descriptor/reference
  coverage, checked total source lengths, and EOF before `Session::from_archive`
  returns a session (`wire.rs:125–251`). Save borrows original pixel slices;
  deduplication includes dimensions and an exact-byte comparison (`45–122`).
- Same-directory staging propagates encode/flush/file-sync/replacement failures
  before publication. Directory-sync failure after replacement yields the explicit
  published-but-unconfirmed durability result (`crates/affint-storage/src/lib.rs:202–238`).
  No false untouched-target error was found in that source path.

## Evidence limits and provenance

Existing coordinator evidence inspected:

- `.research/verification/native-fidelity-probes-01/02-test.log`: both original
  native fidelity failures, including the exact width bits and accepted crop.
- `.research/verification/core-pages-fidelity-02/02-test.log`: malformed-start
  constructor panic before the final correction.
- `.research/verification/core-pages-fidelity-03/{result.json,02-test.log,03-clippy.log,04-build.log}`:
  41 document and 16 storage tests pass; the recorded full workspace, formatting,
  Clippy with warnings denied, and native build also pass. App/render acceptance
  is not inferred from this core/storage review.

Independent runtime execution remains **unperformed by this worker**. Arbitrary
Serde inputs, complete dependency behavior, crash/power-loss and filesystem
semantics, non-Unix directory durability, coordinated process-wide residency,
persistent history, and unsupported-format retention remain outside this result.
The documented refusal policy and separate module budgets are not new findings.

This report is independently worded from AFFINT source and retained coordinator
evidence. No open-source implementation, vendor material, or external prose was
copied into it; no additional license/attribution artifact is required. Prior
reuse/license records remain ADR0001/0004/0008. No broad refactor, score, or further
hypothetical architecture investigation is proposed. The bounded review stops here.
