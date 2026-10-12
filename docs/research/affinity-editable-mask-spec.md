# Affinity editable raster masks: bounded content-to-evaluation specification

Status: static binary research, round 04, 2026-10-12. This is an independently
worded behavior/data-flow specification, not reconstructed vendor source.
Implementors may consume this document. They must not read the restricted vendor
pseudocode, disassembly, or extracted bytes.

## Result and exact scope

The selected mask-raster content method supplies its bitmap as a **transparency
input**, alongside a separate solid fill. For the examined byte-RGBA bitmap render
target, the consumer selects a bitmap transparency sampler and reaches scanline
processing that invokes color/transparency callbacks before its final blend.
This establishes a content-evaluation route, not merely a selection/display-mask
buffer getter [A4-P-1867f19f0, A4-R-1812c1e60, A4-R-1839cfb60,
A4-R-1839cc010, A4-R-18397c580, A4-R-18139eb60].

**The exact conversion of source samples into mask coverage remains unknown.**
The sampled pixel callbacks and final blend bodies were not decompiled. This round
therefore does not establish alpha-only interpretation, luminance weights,
alpha-times-luminance, transfer space, inversion arithmetic, or numerical parity
with AFFINT's masks. Names containing “alpha” do not close those gaps.

The investigation starts at the installed mask-raster content entry, not at a
verified UI action or document deserializer. It follows the ordinary, non-isolated
branch, with no visible child content, conditionally using the examined bitmap
render-target class. Object construction and actual runtime target selection are
not proved. The child-bearing branch is described only at its content-method
boundary. Selection overlays, marching ants, broad notification dispatch, and
DirectX display submission were not reopened.

No Affinity process, target helper, Wine session, UI, product test, build, or
benchmark was run. No product code or earlier research artifact was changed.

## Authority and source identity

“Observed” below means an observation of shipped bytes and their static control or
value flow. It does not mean execution was observed. “Derived” means several such
facts were connected with an explicit condition. “Unknown” identifies the
remaining boundary, not a vendor defect or an absent feature.

The installed release remains the previously identified `3.2.3.4646`. Selected
current native images are PE x86-64 with preferred image base `0x180000000`:

| Artifact | Bytes | SHA-256, equal before and after this round |
|---|---:|---|
| `libpersona.dll` | 359,297,544 | `285b2b2634029ddb05e7ef436a4da5aefec973362187a88e848b361d22ee1678` |
| `librenderer.dll` | 81,864,712 | `4d087c59b50c1213597bc78cf9b4e24ff8725a176bfed4f0d51828378e617bb6` |
| `libraster.dll` | 71,407,112 | `ee3904ba5b05010bbf7d763ae407f594c8bf0245f0106a9b015c5e6bf6dc4d84` |

Only persona and renderer functions received focused native analysis. Raster
interfaces were identified through verified renderer imports; their implementation
bodies were not analyzed in this round. The three file hashes were checked against
round 01 as well as against this round's initial hashes. Historical `.bak` files
are outside the target scope.

Evidence IDs beginning `A4-` identify locally retained, source-bound Ghidra records;
they are not REA-generated Evidence IDs. The restricted audit register is
`.research/vendor-affinity/round-04/evidence-register.json`. It retains full source
and function digests, source RVAs, file offsets, individual body-range bindings,
analysis commands, assembly/pseudocode digests, and limitations. Implementors do
not need to open that register or its raw companions.

## Content interface and source dependency

### Observed

The selected method obtains bitmap state through a raster interface. It queries
source dimensions and bounds, prepares opacity/blend settings, and constructs a
bitmap-backed fill descriptor. The bitmap descriptor is supplied as the
transparency input of a render-target blend operation. A separately constructed
solid fill supplies the color input [A4-P-1867f19f0].

The bitmap-fill constructor retains a strong reference to the provided bitmap. It
also records sampling/edge settings and optional mutation-related collaborators.
It does not copy the bitmap's pixel payload in this constructor. The descriptor
constructor retains the fill object and stores its six affine coefficients and
settings [A4-R-180166e70, A4-R-18013cfb0].

The bitmap sampler rejects absent source backing, zero dimensions, and incompatible
source representation before normal sampler admission. It checks deferred-load
state and hardware availability when selecting specialized or general paths. The
general path also carries quality and mutation-related settings forward
[A4-R-18397c580]. The final decode/error-reporting contract is not recovered.

### Derived, with limits

The render input depends on bitmap content and sampling state, not just on the
mask node's label or a detached display overlay. Its local owners retain the
bitmap and fill while evaluation is prepared. This is evidence of a retained
content relationship, not proof that every future source edit is automatically
reevaluated correctly.

A source mutation host/target can participate in this route. Their presence is
not proof of a particular brush behavior, immutable-source design, or complete
non-destructive editing model. The raster-interface bitmap getter and those
collaborators remain untraced. No arbitrary hidden-layer-source evaluation policy
is established.

## Coordinates and bounds

### Observed

The content method consults raster bounds, can substitute the full bitmap extent,
and calls node base/spread transform interfaces while preparing bounds. A
node-controlled flag changes its bounds/edge-policy branch. The flag's complete
user-facing meaning is not established [A4-P-1867f19f0].

The bitmap descriptor's local affine construction uses half the source width and
height for scale and translation. The reached bitmap sampler uses the reciprocal
source dimensions with a factor of two and a negative unit translation, then
combines the descriptor with render-target affine state. These reciprocal
constructions support a normalized bitmap-coordinate convention. The constants
and arithmetic are source-bound; they are not inferred merely from method names
[A4-P-1867f19f0, A4-R-18013cfb0, A4-R-18397c580].

Sampling setup transforms the requested rectangle and intersects it with the
source extent. It can choose a different resampling path based on transform-related
conditions and target settings. The final sampler receives the resulting affine
state, sampling choice, and edge setting [A4-R-18397c580].

The render-target blend wrapper derives bounded geometry from its clip and affine
state before invoking the polygon drawing interface. The byte consumer then
computes a clipped rectangle for populator admission [A4-R-1812c1e60,
A4-R-1839cfb60; the latter is assembly-only evidence].

### Unknown

The exact composition direction across node base, spread/document, render target,
and final sampler coordinates is not fully established. Nor are pixel-center
placement, edge-extension semantics, singular-transform policy, source/target
attachment rules, mip selection, or precise interpolation kernels. Do not import
the vendor's internal normalized convention into AFFINT as a required interface.

## Coverage and ordering

### Observed local order

On the selected simple branch, the content method prepares settings and bitmap
sampling state, then blends solid color through its bitmap transparency descriptor
[A4-P-1867f19f0].

In the byte polygon consumer, static assembly shows this setup order: color-fill
populators, color-transform populators, blend-range populators, then transparency
populators. The bitmap transparency descriptor reaches a concrete bitmap-type
branch. That branch is corroborated by shipped type-descriptor bytes and the
subsequent bitmap field/dimension accesses, not by a mask-like name alone
[A4-R-1839cfb60, A4-R-1839cb9f0, A4-R-1839cc010].

The reached bitmap transparency helper constructs a sampling context tied to the
source, selects callback addresses, retains the context around registration, and
passes the callback bundle to the transparency-populator registration interface
[A4-R-18397c580]. The source-binding setter and registration implementation are
not analyzed, so their internal source ownership and exact list-maintenance
semantics remain boundaries.

For scanline evaluation, the reached CPU span routine invokes preparation callbacks,
synchronizes destination host caches where required, invokes its two populator
lists for each surviving span, and then invokes the selected final blend. The
blend receives destination pixels, optional mask-buffer samples, populated scratch,
and geometric coverage separately. This keeps geometric clipping, source
transparency, and an already-current mask buffer distinguishable in the observed
interface [A4-R-1839fdf40, A4-R-18139eb60].

The clipping variant combines intersecting geometric coverage before the same
populator/blend boundary. That byte-coverage combination is **not** evidence of how
editable bitmap samples become mask coverage [A4-R-1813a7d60].

On the child-bearing branch, the content method requests a temporary composite,
sets up a matte, blends its own bitmap, draws two child lists in a fixed local
order, and requests composite-as-transparency handling. Those composite and child
bodies were not traced [A4-P-1867f19f0].

### Unknown

There is no recovered end-to-end ordering for crop, exposure, filters, adjustments,
layer opacity, nested masks, or arbitrary source operations. Preparation order in
one draw routine is not a complete editable-operation graph. The opacity helper,
compound-operation meanings, children, and final blend formulas remain untraced.
There is no numerical evidence for the alpha-versus-luminance coverage question.

## Precision, ownership, and cache boundaries

The examined byte CPU span path addresses four destination bytes per pixel and
allocates/reuses span scratch at four bytes per pixel. Its geometric coverage is
byte-oriented; the clipping variant performs byte coverage combination. Affine
setup uses double-precision scalar arithmetic. These are local storage/arithmetic
facts, not a guarantee about every mask intermediate or about linear-light color
[A4-R-18139eb60, A4-R-1813a7d60, A4-R-18397c580]. The general sampler carries a
quality-policy setting from the target; the setting's value on an actual request
was not observed. Source-dimension queries are not proof of exact-resolution
export or of a universal prohibition on degraded sampling.

The byte and floating current-mask-buffer getters both return a retained reference
to an existing target-owned buffer. Neither converts pixel values. The floating
getter therefore establishes ownership only; it does not establish floating mask
coverage semantics, buffer sample width, HDR preservation, or byte/float agreement
[A4-R-1839d8990, A4-R-18411d7f0]. These getters alone do not identify an editable
layer-mask source.

The draw setup retains the current target mask buffer and passes its host-sample
access and owning reference into rasterizer state. The observed state update
acquires the incoming owner and releases the replaced one
[A4-R-1839cfb60, A4-R-18206e510]. Normal source/descriptor cleanup also releases
retained objects. This is not a complete exception-path or asynchronous lifetime
proof.

The CPU span routines synchronize host caches before use and invalidate the
hardware cache for modified destination regions after uncancelled work. They also
contain gated accelerated dispatch and a CPU retry path when that dispatch fails
without cancellation. The examined gate excludes the ordinary mask blend-mode
value selected by the content entry. This is a narrow static gate observation,
not proof that all masks use the CPU or that a live GPU backend was used
[A4-P-1867f19f0, A4-R-18206e510, A4-R-18139eb60, A4-R-1813a7d60].

These host/device cache operations are not a semantic render-cache key. This round
recovers no complete dependency identity, revision token, source-content version,
mutation-to-invalidation ordering, or stale-result acceptance contract. A retained
pointer cannot be treated as sufficient cache identity. GPU completion, resource
retirement, peak residency, and performance remain unmeasured.

## Independent AFFINT contract and useful checks

No existing AFFINT contract should change on the strength of these findings.
AFFINT's target-local Alpha/linearLuma masks, hidden-source evaluation, ordered
crop/exposure operations, f32 linear-premultiplied reference evaluation, and typed
revision acceptance remain independent product decisions. This round establishes
no parity claim for them.

Useful independently designed interface obligations are:

- Carry the declared coverage interpretation, source dependency, transform, edge
  policy, requested quality, and precision policy into evaluation and cache identity.
- Keep original source ownership separate from sampled scratch and from geometric
  clipping coverage. Retain each resource's budget lease while that resource is live.
- Distinguish semantic dependency invalidation from host/device residency updates.
  Validate revisions at publication, not merely at preparation or pointer retention.
- Make deferred source failures explicit. Cancellation must not publish incomplete
  spans as a successful exact render.
- Reuse bounded span scratch and immutable shared sources where measurements justify
  it. No speedup or vendor bottleneck has been measured here.

Coordinator-owned verification should use opaque colored sources, transparent
white sources, partial-alpha colored sources, translated/scaled/rotated masks,
out-of-source samples, hidden sources, operation-order changes, and source edits
without dimension changes. Compare cached/uncached output under AFFINT's declared
policy, exercise undo/redo and branch revisions, and check that rejected/cancelled
results release only resources no longer in use. Include declared byte/float
error tolerances where those paths exist. None of these checks was run by this
research worker.

## Function provenance and extraction limits

Addresses are preferred PE virtual addresses, not live process addresses. Source
RVAs equal the listed address minus `0x180000000`. The following entry bindings
identify the bounded records; full body-range and output digests are retained in
the restricted audit register. Byte lengths below describe the entry's source
region, not necessarily its whole non-contiguous function body.

| Evidence ID / function address | Source file offset | Entry-region bytes | Entry-region SHA-256 |
|---|---|---:|---|
| `A4-P-1867f19f0` | `0x67f0df0` | 2,631 | `c47c5b0ad37811b30539905796671f98d5310aed735239f699372de0f5597570` |
| `A4-R-180166e70` | `0x166270` | 353 | `1e7bab5f7b633aa21403573519c6f82db39c850e307d21c205850bb8714937ff` |
| `A4-R-18013cfb0` | `0x13c3b0` | 170 | `5d4d0dfbeb6ca516fd9d8823fd7ea4399510a4fb75d20269f32b3e94ae53de53` |
| `A4-R-1812c1e60` | `0x12c1260` | 1,145 | `aa992e8cdcd45161b5e7f0706604b5fc6b139db549253301d869a73438a1f8be` |
| `A4-R-1839cfb60` | `0x39cef60` | 34,311 | `e89d555998d8d9ac1d5c822112b27cd2fe4b886f2b29306cadc6a14d6692fdce` |
| `A4-R-1839cc010` | `0x39cb410` | 1,038 | `69eedd090a3d0bd1d544fd2ffacb023fbffedbbda45a7e17ce0e44e58f46859e` |
| `A4-R-18397c580` | `0x397b980` | 3,957 | `f17d884768230b4984e28b763975646c700d6e3da82285320237ea1fe39f33c9` |
| `A4-R-18139eb60` | `0x139df60` | 4,240 | `042872c27d8d862078eb60a3be76bcbed17db75e088fd2691ae37e2d8a1b0cc9` |
| `A4-R-1813a7d60` | `0x13a7160` | 3,352 | `22190684077e20db482981aa5afdedff1aca1051bc9481bfa3351d2d39c8d1ba` |

REA CLI `6.4.0` and the current REA skill were consulted. This round reused the
prior successful direct focused Ghidra `12.1.4` route. No native REA session or new
REA-generated native Evidence record was created; no session remains to close.

Six sequential headless invocation attempts used one analysis CPU, a 240-second
outer timeout and a 25-second per-function decompiler timeout. The first attempt
was rejected before import because Ghidra disallows dot-prefixed project-path
components. The retry changed only the private analysis-project location; raw
outputs remained under the owned restricted directory. Five invocations then
exited successfully. The 34,311-byte byte polygon function exceeded its 25-second
decompiler limit; its static assembly and call sites remain usable and are marked
assembly-only. It was not retried.

A combined byte/floating polygon extraction exceeded the slicer's 64-KiB
per-analysis bound before starting Ghidra. Its generated slices were preserved
and byte-verified, but are not counted as analyzed functions. The follow-up used
only the byte function, within the bound. The floating polygon body remains
unanalyzed.

Canonical analyzed coverage is **14 distinct functions**, **15 extraction records**
(one duplicate wrapper record), **69 mapped slices**, and **70,405 summed bytes**.
Thirteen distinct functions have completed pseudocode; one is assembly-only.
Including the rejected combined request, retained evidence contains **87
source-equal slice files**, totaling **143,860 bytes**. Sums include overlap.
All selected source hashes are unchanged. These counts are not an app-completeness
claim.

Decompiler types and signatures are inferred. Overlapping-global warnings,
split/cold ranges, unresolved indirect calls, and incomplete exception cleanup
remain explicit. The persona dispatch-table seed includes neighboring interface
metadata; it was not treated as one flat method table. Concrete table installation
and mask-node construction were not analyzed. Conditional target dispatch is
corroborated by the examined renderer table entries, not by a live target instance.

## Unexecuted next probes

A later authorized round could answer the remaining questions without reopening
broad notification/display tracing:

1. Follow the selected bitmap transparency callbacks at renderer addresses
   `0x183a1c390` and `0x183a1b620`, then only the raster sampling calls they actually
   reach. Alternative general callbacks are `0x183a3c840` and `0x183a3c700`.
   Establish source format, sample-to-coverage conversion, inversion, and precision
   from their bodies before assigning alpha/luminance meanings.
2. Trace transparency-populator registration at `0x1813aff60` and the selected final
   blend body to bind sampler output to the blend's coverage arguments.
3. Verify mask-node construction and the raster-interface bitmap getter reached
   from the content entry. Then follow one source edit into its dependency/version
   change, rather than treating the bitmap pointer as content identity.
4. Recover the actual transform helper bodies and one non-identity mask attachment
   before declaring source/target coordinate parity.
5. Analyze the floating polygon consumer at `0x1841149a0` separately, within a
   fresh bounded round. Getter similarity is not floating-render equivalence.

These are queued questions only. No seventh native invocation, pixel-callback
analysis, installed-target execution, or coordinator build was started.
