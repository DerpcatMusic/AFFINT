# ADR 0008: Strict geometry serialization and consumed typed staging

Date: 2026-10-12. Status: implemented and coordinator verified.

The independent [core quality review](../research/core-quality-review.md) identified
one source-established staging allocation ordering problem and two native fidelity
probes. The coordinator ran the native probes in
`.research/verification/native-fidelity-probes-01/02-test.log`: an otherwise valid,
checksummed crop containing `future_crop_rule` loaded successfully and lost that
member; a finite page-width value `1.8233341452376575` (`0x3ffd2c606cb79d18`) decoded
one ULP away from its saved value. These are reproduced native fidelity failures.
The document-owned change here addresses strict geometry and typed staging. The
storage-owned `serde_json` correctly rounded parsing feature and bit-level native
oracle are separate work; this document module does not select a JSON codec.

We retain Kurbo 0.13.1 as the public and mathematical representation. The private
`geometry_serde` module uses Serde remote definitions for `Rect`, `Point`, and
`PathEl`, with unknown-field refusal on each rectangle/point definition. The derive
checks those remote field/variant definitions against the pinned upstream types.
`LiveOperation::Crop(Rect)` and `VectorContent` keep their public types. Only their
persistence-facing fields select the strict adapters. Existing finite geometry,
subpath, positive crop, graph, and resource validation still run at their existing
seams before publication.

The supported wire representations remain:

- A crop is `{"Crop":{"x0":0.25,"y0":0.5,"x1":8.0,"y1":10.0}}`.
- `MoveTo` and `LineTo` contain one point object, for example
  `{"MoveTo":{"x":0.0,"y":0.0}}`.
- `QuadTo` contains a two-point sequence and `CurveTo` contains a three-point
  sequence. Each point contains only `x` and `y`.
- `ClosePath` is the unit variant string `"ClosePath"`.
- `BezPath` remains its original newtype sequence of those elements; the vector
  record remains `{"path":[...],"fill":[r,g,b,a]}`. Field names, variant order,
  Serde type names, and native schema version remain unchanged.

The adapters preserve the supported derive representations, including the existing
struct sequence form available to Serde deserializers, rather than imposing a new
JSON-only document type. They reject missing, duplicated, and unknown point/rect
fields and unsupported path variants. Path serialization walks borrowed elements
and delegates one element at a time to the strict remote definition; it creates no
whole-path vector. Deserialization builds Kurbo elements directly, rather than an
intermediate vector of separate geometry DTOs, then passes through the existing
validated `VectorInput` ownership conversion. It does not trust a deserializer's
allocation hint. The storage decoder must still cap input bytes before decoding;
this change is not an allocator or process RSS limit for arbitrary Serde inputs.

Alternatives were a storage-only JSON member prewalk and a fork of Kurbo's Serde
derives. A prewalk would duplicate native schema knowledge in storage, introduce
another object-tree traversal, and leave other document deserializers permissive.
A dependency fork would spread AFFINT's persistence policy to unrelated geometry
users and require carrying a geometry fork for a narrow field-acceptance change.
The private adapters concentrate that policy in the document module without new
dependencies, new public wrappers, a format migration, or a serializer framework.
Primary references: [Serde remote derive](https://serde.rs/remote-derive.html),
[field adapters](https://serde.rs/field-attrs.html),
[unknown-field policy](https://serde.rs/container-attrs.html), and the pinned local
Kurbo source at `src/{bezpath,point,rect}.rs`.

Typed staging now consumes the `Edit` already owned by `Session::transact` or
`update_gesture`. `Session::stage` keeps its label/count context and passes owned
changes to `staging::apply`. Operation `Vec`s, layer content, and asset handles move
into the candidate state. The old `SetOperations` and `LayerSpec` paths cloned an
entire operation vector before the final metadata check; under a 32 KiB policy,
the allowed 32,768-operation input could therefore create a second payload larger
than 1 MiB before refusal. Those explicit bulk clones are removed. Existing state
clones still share persistent maps and immutable Arc-owned payloads.

Previously, `Vec::clone` also normalized spare capacity to the live element count.
For a small consumed vector with large spare capacity, staging now reduces spare
capacity only when checked live-element bytes fit the metadata policy. Any possible
resize request is therefore within that policy. A larger input moves unchanged
without requesting a second large vector; final validation refuses it if it remains
in the resulting state. A later change may replace it in the same atomic batch.
The allocator's actual residual capacity is still charged by final validation.
No unsafe allocator instrumentation or custom allocation framework was added.

The coordinator's `core-pages-fidelity-02` run passed owned-staging scenarios and
the original document tests, then reproduced an upstream `BezPath::from_vec`
debug assertion for a malformed first element. The correction checks that specific
upstream constructor precondition in the strict adapter. The shared borrowed
`VectorContent` factory performs full finite/subpath validation before its canonical
`from_vec` call, including the public typed constructor. This preserves one full
validation pass, canonical ownership, normal wire shape, and structured errors;
no panic-catching fallback or weakened oracle was introduced. Empty paths and
explicitly restarted subpaths remain valid. Targeted malformed-start, closed-subpath,
and typed nonfinite controls await the coordinator's next verification run.

A preflight that sums every incoming replacement would reject a valid remove-then-
insert batch or a series whose early payload is superseded. An intermediate full
graph validation would also reject relationships repaired later in the same batch.
Ownership transfer avoids the extra vector while preserving complete final-state
validation and the existing sequential edit semantics. It does not claim every
transient existing typed-input owner is below the final admitted-state budget:
caller-created inputs can already be larger, and string/Arc headers, map paths,
and copy-on-write relationship lists remain separate allocations. No global RSS,
allocation-measurement, or performance improvement is claimed from this source pass.

Publication and history remain unchanged: an error drops the candidate; committed
artwork/revision and undo/redo survive; IDs issued during a failed batch still burn;
old snapshots retain the live allocator watermark; accepted gesture updates have
distinct revisions and commit once; source pixels retain their original owners.
Final source/metadata admission, outstanding snapshot accounting, and all graph
validation still occur before registering or publishing the candidate.

Added document-interface evidence, not yet executed by this worker:

- An independently authored schema-1 archive control covers every path variant,
  finite crop/exposure, original field names, and geometry values. It is compared
  with explicit literal JSON and the pinned Kurbo serialization shape.
- Unknown member mutations target crop and every point slot of MoveTo, LineTo,
  QuadTo, and CurveTo; valid controls, duplicate-field refusal, and existing
  subpath/crop validation make parser failures meaningful.
- Owned buffer identity scenarios cross insertion, property replacement,
  publication, gesture preview, and undo/redo. Separate oversized-input scenarios
  check rollback, both history sides, burned IDs, and retained watermarks.
- Valid remove-then-insert batches, superseded oversized owned inputs, and small
  inputs with large spare capacity guard against arbitrary summed-payload refusal.
- The original 24 document tests, including both property tests, remain required.
  New scenario tests establish semantics and owner identity; they are not proof
  of allocation timing or an RSS cap. Absence of the extra operation `Vec::clone`
  is a source observation. The independently rebuilt/checksummed native crop probe
  in the storage crate remains the acceptance test for the original fidelity defect.

The bounded source-only improvement pass reviewed the two session staging callers,
payload ownership, fallible visitor propagation, revision/ID timing, history, and
remaining allocations. It found no need for a new wrapper crate or representation
rewrite. No build, formatter, test, helper executable, or benchmark ran in this
worker. The coordinator should format under its exclusive lane, then run the
combined document/storage verification before recording this increment as verified.

Coordinator outcome: `core-pages-fidelity-03` passes all 41 document tests and
16 storage tests, plus the remaining workspace, formatting, all-target Clippy
and native binary build. The unchanged native unknown-crop refusal and 4,096-case
f64 bit corpus pass. Round02 caught a malformed-start constructor panic; round03
checks malformed serialized and typed paths before Kurbo construction. Existing
valid wire shapes, property tests, original pixels, history and owner scenarios
remain passing. These checks establish bounded behavior, not allocator/RSS claims.
