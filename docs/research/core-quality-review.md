# Independent document/storage quality review — round 1

## Result and scope

Reviewed **3d810bf48daaf5825d56f17cb1d1b63760f0beff**. The two production crates
were unchanged from that checkpoint during inspection. HEAD independently advanced
to `eaffceab66c2cbcca45d32d7079351ce9f7ff6b0` before the final check; scoped
production still had no diff against the reviewed checkpoint. Read every source file and
Rust test in `crates/affint-document/**` and `crates/affint-storage/**`, their
manifests and PNG fixture provenance, plus AGENTS/PRODUCT/CONTEXT, architecture,
work log, ADR0001/0004, and the actual local Rust/design/domain skill files under
`<local skill catalog>`.

**No confirmed content-atomicity, ID-reuse, original-pixel sharing, archive-checksum,
or publication-status bug found.** One source-established, localized staging
admission improvement and two unresolved fidelity probes follow. These are not
three reproduced bugs. Priorities below indicate coordinator follow-up value;
conditional concerns have no established defect severity yet.

No builds, tests, benchmarks, helpers, installations, nested agents, or production
edits were performed. App/render production and restricted vendor raw were not
read. Dependency implementations/effective build features were not inspected;
documentation tools were unavailable, and the prohibited CLI/install path was not
used. All probes are **unexecuted and uncompiled**.

Exact source SHA-256 values:
`source-sha256.txt` (private research evidence, not distributed).
All line references below are to the reviewed checkpoint.

## 1. P2 hardening: preflight document-owned typed-edit payloads before cloning

**Classification:** Source-established allocation ordering; justified admission
improvement, not a demonstrated published-state limit violation or OOM.

**Locations:** `crates/affint-document/src/staging.rs:95–99,189–212`;
`crates/affint-document/src/session.rs:525–560`;
`crates/affint-document/src/validate.rs:136–160`.

**Source observation:** `SetOperations` checks the operation *count*, then clones
its `Vec` into the staged layer. `LayerSpec` submission also clones operations.
The metadata byte check occurs only after `staging::apply` has completed. Therefore
an oversized typed edit can first create a document-owned payload much larger than
`max_metadata_bytes`, and only then return a byte-limit error. This is distinct
from the documented allocation behavior of untrusted JSON/upstream deserializers:
it concerns an explicit application-owned `Vec::clone` after the input already
exists. Accepted artwork/history still remain atomic and bounded by final checks.

**Bounded reproduce plan:** Use a 32 KiB metadata policy, create one small group,
then submit 32,768 `Exposure(0.0)` operations in one prebuilt `SetOperations` edit.
The default count policy allows that count. The clone holds at least 1 MiB because
`LiveOperation` must accommodate a four-f64 crop rectangle. The final metadata
check should refuse it. Observe allocation requests only around `transact`, not
while constructing the caller's input. The supplied
`document_staging_control.rs` (private research evidence, not distributed)
checks refusal and unchanged revision/content/history; **passing that control does
not prove early admission**. Clone ordering is the source evidence.

**Impact/recommendation:** A low metadata budget does not prevent this avoidable
staging allocation. Preflight checked payload bytes before copying names/lists,
and keep a conservative running staged charge so several individually small
replacements cannot defer all byte admission until the end. Keep the existing
persistent representation and final invariant validation. No RSS or speed claim
is implied.

**Unresolved:** Does the intended byte policy cover prepublication allocation
requests, or only accepted-state charge? If the latter is deliberate, document
that narrower contract; retain this as localized hardening rather than a bug.

## 2. P2 probe: nested geometry must refuse unknown members

**Classification:** Strict-schema concern, not a confirmed acceptance bug.

**Locations:** `crates/affint-document/src/model.rs:313–318,426–434`;
`crates/affint-document/src/archive.rs:9–18,60–88`;
`crates/affint-storage/src/wire.rs:19–28,160`;
`crates/affint-storage/tests/native_archive.rs:241–270`.

**Source observation:** The document-owned records deny unknown fields, but crop
payloads are delegated directly to `kurbo::Rect` and vector paths to Kurbo geometry
Serde. Outer-record refusal does not establish recursive refusal inside those
payloads. The current native unknown-field test mutates only the manifest root.
The upstream nested parser behavior has not been verified in this review.

**Unexecuted oracle:**
`storage_oracles.rs` (private research evidence, not distributed),
`native_crop_refuses_a_checksummed_unknown_nested_geometry_member`, adds
`future_crop_rule` inside the serialized crop rectangle. It independently rebuilds
the header lengths and manifest SHA-256, preserving a valid unaltered control.
Expected: structural/unsupported-input refusal. If load succeeds, the probe
establishes silent loss of unknown native content, rather than checksum corruption.

**Conditional impact/recommendation:** If reproduced, this is P2 native fidelity
loss on load/resave. Use a small AFFINT-owned strict Serde geometry adapter at the
persistence seam, retaining Kurbo for geometry and the existing supported wire
shape. Adding an outer enum attribute alone does not secure a delegated payload.
Extend the oracle to serialized path points if they are object-shaped.

**Unresolved:** Does pinned Kurbo already reject the member? If yes, close this
concern with the oracle result; do not change production merely to satisfy a
speculation.

## 3. P2 probe: exact floating-point persistence needs a bit-level oracle

**Classification:** Fidelity-verification concern, not a confirmed numeric defect.

**Locations:** `crates/affint-storage/src/wire.rs:108,160`;
`crates/affint-storage/Cargo.toml:13`;
`crates/affint-document/src/model.rs:109–150,154–215`;
`crates/affint-storage/tests/native_archive.rs:105–132`.

**Source observation:** Native editable f64 values cross JSON text encoding and
parsing. The storage manifest pins serde_json without explicitly requesting its
`float_roundtrip` feature. Effective feature unification and the pinned parser's
rounding behavior are unknown here. Existing native persistence fixtures use
simple exact-binary numeric values and do not establish arbitrary finite-f64
round trips. Comparing JSON Values can also bypass the actual text parse path.

**Unexecuted oracle:** `native_page_dimensions_preserve_exact_finite_f64_bits` in
`storage_oracles.rs` saves/loads a fixed 4,096-case corpus of widths in `[1,2)` and
compares `to_bits()`. It excludes signed-zero/underflow policy questions. Any first
failure reports a deterministic input bit pattern and the complete small manifest,
which the coordinator can freeze as the minimal regression fixture.

**Conditional impact/recommendation:** If reproduced, this is P2 unchanged-editable
parameter drift on save/reopen, not pixel corruption. Verify the effective codec
features and correctly rounded parsing requirement; enable the appropriate pinned
codec option if needed, with the native interface oracle retained. A green bounded
corpus is useful evidence, not a proof for every possible f64.

**Unresolved:** Is correct-rounding support already enabled transitively in the
actual storage/app build? What exactness contract is intended for editable f64
parameters? No drift or feature defect is claimed without coordinator evidence.

## Examined seams with no actionable defect found

- Staging publishes only after validation/admission; failures preserve current
  artwork, revision, and both history sides. Irreversible ID burn is intentional.
- Snapshot watermarks share the allocator outside undo; branch/canceled preview
  IDs stay reserved. History transitions issue fresh revision events.
- Weak registries track live source owners and retained states; shared source
  aliases count once by pixel owner and pixel capacity is charged. History trimming
  and canceled/outstanding snapshots were traced against the resource scenarios.
- Native framing checks lengths, schema, references and SHA-256 before session
  publication; dedup keys include dimensions and verify exact bytes.
- PNG import checks both early and finalized metadata, invokes `reader.finish`,
  and expands admitted channel storage backwards without a second full image.
- Native/PNG save propagates encoding/finalization/flush/file-sync errors before
  replacement. A directory-sync failure after publication returns an explicit
  durability outcome, not a false untouched-target error. OS/filesystem guarantees
  and unverified non-Unix directory durability remain the documented limits.

Session-local unsaved history, absent asset collection/page removal, retained source
bytes, separate module budgets, bounded-JSON infallible allocations, and the
supported PNG precision/color subset are intentional scopes, not findings. No
performance ranking, blanket refactor, or ownership reassignment is proposed.

The coordinator may copy/include the research probe files into its owned integration
targets using existing dependencies, then run the single coordinated verification
entrypoint. No probe is automatically registered by this review.

## Coordinator outcome, 2026-10-12

After this source review froze, root registered the two storage oracles in
`crates/affint-storage/tests/native_fidelity.rs`. Both failed in
`.research/verification/native-fidelity-probes-01`: the checksummed unknown crop
member loaded successfully, and the first finite-width case changed bits from
`0x3ffd2c606cb79d18` to `0x3ffd2c606cb79d19` on native decode. These two concerns
are now reproduced defects; the earlier investigator classification records what
was known before execution. Strict geometry adapters and the pinned codec's
`float_roundtrip` feature now pass full verification in `core-pages-fidelity-03`.
The 4,096-case native f64 corpus preserves every input bit, and the checksummed
unknown crop member is refused. Geometry tests also caught and corrected a new
pre-validation Kurbo constructor panic; malformed starts and typed paths return
errors before construction. The document has 41 passing tests; storage has 16.
Consumed staging payloads pass owner-identity, history and rollback scenarios.
Removal of the extra vector clone is source-established; no allocator/OOM or RSS
claim follows from these tests. Earlier classifications remain historical evidence.
