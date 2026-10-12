# Installed Affinity: display-result publication and mask research boundary

Status: bounded static round 3, 2026-10-12. This report contains independently worded behavior and interface findings. It does not contain vendor source, reconstructed code, or implementation instructions. Implementors and the coordinator consume this report, not the restricted evidence.

## Result

One concrete shipped display route is now bounded. The native document-render window constructs a DirectX tiled-view renderer. Its tile allocator constructs a concrete tile whose byte and floating render-result callbacks retain buffers. A subsequent display draw uploads those buffers and submits geometry. The worker callback is not itself the texture-upload boundary.

The callbacks and display drawing share a tile display lock. This protects their buffer handoff. It does not, by itself, establish that a result belongs to the current document revision. The examined callback bodies have a teardown-related eligibility check; no current-revision or captured-dependency comparison was established there.

The end-to-end stale-result question remains **unresolved**. Neither a safety guarantee nor a vendor race defect follows from the examined code. Additional notification dispatch, mutation ordering, and display policy could change the answer.

The editable-mask probe is **deferred**, not completed. Following the notification route exposed another type-dispatch boundary. This round stopped at its six-analysis limit rather than expand into several new subsystems. Display selection/marching-ant buffers are not evidence of editable layer-mask semantics.

## Scope and evidence rules

The target is the current installation at `<local Affinity installation>`. Round 1 identified the installed set as `3.2.3.4646`. The older `.bak` assembly was not used.

All observations are static. No Affinity process, installed helper, UI, Wine launch, GPU experiment, or target test ran. No AFFINT build, test, benchmark, formatting, `cargo`, or Rust compiler command ran. No installation, user-configuration change, or nested worker occurred. Account and license behavior was not investigated.

Observation means that source-bound instructions, metadata, or file bytes support a fact. Interpretation means that several observations support a relationship without demonstrating execution. Unknown means that the evidence does not establish the answer. Names alone are discovery, not behavior evidence.

A publication boundary is where a result becomes available to a consumer. A dependency check compares the input identity behind a result. A completion fence establishes when submitted GPU work has finished. A lock can protect storage without validating either dependency identity or GPU completion.

## Source identity and evidence register

The three analyzed originals matched their round-01 hashes before extraction and at final verification. The two mask-discovery libraries also matched those hashes at final verification.

| File | Bytes | SHA-256 | Round-3 use |
|---|---:|---|---|
| `Serif.DirectX.dll` | 1,314,312 | `fe149204fb3f0ea4b92924b5b0296a7466c937c909968ffcaaf71cabeef6802f` | Concrete tile, upload, display submission, and local resource release. |
| `libpersona.dll` | 359,297,544 | `285b2b2634029ddb05e7ef436a4da5aefec973362187a88e848b361d22ee1678` | View invalidation and locking. Reused verified round-2 worker evidence. |
| `Serif.Interop.Persona.dll` | 59,277,832 | `dcf43441e01dbf3fbe83ccb3e3b3279929d89ad8ea9f10a76745aea6a16219dc` | Managed notification wrapper and native render-window dispatch. |
| `librenderer.dll` | 81,864,712 | `4d087c59b50c1213597bc78cf9b4e24ff8725a176bfed4f0d51828378e617bb6` | Mask entry-point discovery only. |
| `libraster.dll` | 71,407,112 | `ee3904ba5b05010bbf7d763ae407f594c8bf0245f0106a9b015c5e6bf6dc4d84` | Matching identity for the queued mask/raster follow-through; no new body analysis. |

Restricted records reside under `.research/vendor-affinity/round-03/`. The evidence register binds functions to original hashes, preferred PE addresses, file offsets, entry-range hashes, split ranges, and warnings. These are local evidence IDs, not REA Evidence IDs. Preferred PE addresses are not live relocated addresses.

| Evidence ID | Address or record | Supported scope |
|---|---|---|
| A3-DX-1800cd720 / A3-DX-1800c6fb0 | `0x1800cd720`, `0x1800c6fb0` | Tile allocator and constructor. |
| A3-V | `native/dx-concrete-vtable.json` | Constructor-installed primary table; byte/floating callback and draw entries. Adjacent data is not claimed as another method. |
| A3-DX-1800c8c30 / A3-DX-1800c8e00 | Corresponding addresses | Byte/floating result reception, retained ownership, placement, and display lock. |
| A3-DX-1800c8fd0 / A3-DX-1800c9290 | Corresponding addresses | Retained-result placement and later typed upload selection. |
| A3-DX-1800c7d70 / A3-DX-180037450 | Corresponding addresses | Concrete display drawing and geometry submission. |
| A3-DX-18003ee60 / A3-DX-18003f1c0 | Corresponding addresses | Byte/floating texture reuse or replacement, host synchronization, transfer, and ownership. |
| A3-DX-18003e520 / A3-DX-18003e660 | Corresponding addresses | Typed texture creation with source dimensions and host samples. |
| A3-DX-1800c76a0 / A3-DX-1800c8700 | Corresponding addresses | Concrete destruction/rebinding release paths. Not a GPU-completion proof. |
| A3-P-1910b3ed0 / A3-P-1910aa440 | Corresponding addresses | Full invalidation across views and tiles. |
| A3-P-1910b4410 / A3-P-1910aae70 | Corresponding addresses | Logical-region invalidation across views and affected tiles. |
| A3-P-1910b4ff0 / A3-P-1910ac380 | Corresponding addresses | View draw-lock boundary. |
| A3-P-1915ef9c0 | `0x1915ef9c0` | Teardown-marker store only; the remaining body is truncated. |
| A3-B-180e1af70 | `0x180e1af70`; metadata token `0x06008305` | Native render-window construction and renderer association. |
| A3-B-180e25c10 / A3-B-180e20370 | Corresponding addresses | Native notification eligibility, controller association, overlay handling, and unresolved registered callback dispatch. |
| A3-B-180e2d540 / A3-B-180e2d590 | Corresponding addresses | Raster-selection display updates. Not an editable layer-mask renderer. |
| A3-M-0600A7CB | Token `0x0600A7CB`, method RVA `0x00df6460`, 357 CIL bytes | Managed notification forwarding and preservation of handled status. |
| A3-I / A3-U / A3-VERIFY | Verified import tables, unwind index, and `verification.json` | Import identity, bounded exception metadata, and byte verification. |
| A2-P-1915fb4a0 / A2-P-1915f14f0 / A2-P-1915f19f0 | Verified round-2 records | Worker abort sampling, separate cache dependency check, and cancellation/rescheduling. See `affinity-raster-tile-spec.md`. |

Selected source bindings:

| Evidence | Initial file offset | Initial-range SHA-256 |
|---|---|---|
| A3-DX-1800c8c30 | `0xc8030` | `e5a31792a74521195190151d33f3604ad172fc7762a461ecd34965264e7fe263` |
| A3-DX-1800c8e00 | `0xc8200` | `c9587ab6bc9b1be3aa2c036c7cd3c67d312586334a7e7225d97106d10f3c653f` |
| A3-DX-1800c7d70 | `0xc7170` | `3d2c6a2ba892a8bbc5f85c3611a4fca8b7cd19629405afbd14ca161d8bce921a` |
| A3-B-180e25c10 | `0xe25010` | `497303b01d7cec466f7ed8e47d923b07f31dc4081f3c60e469305050007a1e4c` |

These bind initial ranges, not entire recovered C++ methods. Split/cold ranges have separate source bindings in the restricted plans.

## Concrete render-result contract

### Construction identifies a real shipped consumer

The native render-window constructor creates a DirectX tiled-view object and associates it with the window's view-controller machinery [A3-B-180e1af70]. The DirectX tile allocator creates a concrete tile and invokes its constructor [A3-DX-1800cd720, A3-DX-1800c6fb0]. That constructor installs a primary dispatch table whose byte and floating result entries resolve to examined bodies [A3-V].

This replaces round 2's pure-call base-class boundary with one concrete implementation. It is a static construction route, not proof of which instance or backend was active under Wine. Other subclasses and rendering configurations remain outside this trace.

### Reception retains buffers; drawing uploads them later

The byte and floating callbacks first test a tile marker associated with teardown. The examined base destruction entry sets that marker, although its later flow was not recovered [A3-P-1915ef9c0]. The result callbacks acquire the tile display lock, retain supplied buffers, update placement information, and mark display work [A3-DX-1800c8c30, A3-DX-1800c8e00, A3-DX-1800c8fd0].

The normal route preserves a typed retained result. Another display-mode branch supports additional retained results. Their exact presentation policy is not a revision-validation contract.

A later concrete draw consumes the pending upload state. Its typed upload helper selects the byte or floating transfer routine. Display drawing then binds retained graphics resources and submits geometry [A3-DX-1800c7d70, A3-DX-1800c9290, A3-DX-180037450].

Thus, the scoped data flow is worker output, retained tile result, later texture transfer, then display submission. These are separate events. Completing the worker does not establish that the texture has been uploaded or that a GPU submission has retired.

### Storage protection is not freshness authority

The callback lock is the same tile display lock acquired by the examined base draw around concrete display operations [A2-P-1915efac0, A3-DX-1800c8c30, A3-DX-1800c8e00]. This supports a protected handoff between result reception and display consumption.

No comparison with a current document revision or the request's captured cache-trigger identity was established in the examined callback bodies. Their visible eligibility test must not be relabeled as a revision check. Untraced assignment helpers, external ordering, and other consumers prevent a universal absence claim.

The worker's renderer-abort sample remains a distinct gate. Round 2 located it after raster work and before result dispatch, including `0x1915fcf2c` through `0x1915fcf39`. The retained-cache identity comparison remains a separate check under another lock [A2-P-1915fb4a0]. Neither observation proves one atomic edit-to-display publication boundary.

An edit or history seek between evaluation, callback reception, and later upload is therefore still a required ordering question. This report does not demonstrate that obsolete content actually reached the display. It also does not establish that such content cannot remain eligible. Retaining a previous frame while replacement work runs might be intentional; the vendor's complete display policy was not established.

## Notification and invalidation boundary

The managed render control forwards received notifications to the native render window when that window exists. It preserves an already-handled status while incorporating the native handler's result. With no window, the examined wrapper retains missed notifications [A3-M-0600A7CB]. CIL corroborates the forwarding; reconstructed managed types alone are not the authority.

The native notification entry requires an attached view controller. For notifications recognized by its first classifier, it compares an associated pointer with the current document controller and skips a mismatch. The association is interpreted as controller filtering, not revision filtering. The classifier body remains untraced [A3-B-180e25c10].

Further classification reaches an examined overlay handler or an indirect registered callback selected from a type-related lookup. The concrete registration and invocation for image-edit/history-seek invalidation were not resolved. The overlay helper also handles view-oriented changes. Its behavior cannot stand in for all document edits [A3-B-180e25c10, A3-B-180e20370].

The lower image invalidation interfaces are independently bounded:

- Full view invalidation reaches tile dependency replacement and invalidation [A3-P-1910b3ed0, A3-P-1910aa440, A2-P-1915f3dc0].
- Logical-region invalidation visits affected tiles and reaches their region-invalidation interface [A3-P-1910b4410, A3-P-1910aae70, A2-P-1915f19f0].
- The examined invalidation traversal uses a structure lock. The view exposes a distinct draw lock. The tile display callback uses another local lock. Their presence does not establish a common revision authority across mutation and publication [A3-P-1910aa440, A3-P-1910aae70, A3-P-1910ac380].

Round 1 established command processing and notification delivery on the examined history-seek route. The missing link is the specific notification consumer and its ordering relative to the callback/display path described here. No claim is made that every seek emits the same invalidation or that notification delivery settles all rendering work.

## Transfer precision, lifetime, and retirement

The examined upload routines can reuse an existing texture when dimensions, transfer representation, and a recreation option permit it. Otherwise, they create replacement texture/view resources. Both routes synchronize the source host cache. The reusable route transfers host samples with row-pitch handling [A3-DX-18003ee60, A3-DX-18003f1c0].

The byte transfer uses four bytes per pixel. The floating transfer uses sixteen bytes per pixel. This establishes local display-transfer width. It does not establish every intermediate's precision, mask precision, color interpretation, HDR fidelity, or CPU/GPU tolerance.

The source dimensions feed typed texture creation [A3-DX-18003e520, A3-DX-18003e660]. These are viewport-buffer dimensions, not proof of exact export evaluation or original-source resolution policy.

This concrete display route includes a host-sample transfer. It is not evidence of universal zero-copy GPU display. Conversely, a host-cache synchronization call is not proof that every invocation performs a GPU readback. Actual device choice, transfers, latency, and residency remain unmeasured.

Tile ownership retains ordinary byte/floating results through upload on the examined normal path. That upload helper does not clear their owning tile slots on return. Concrete destruction and rebinding release texture/view references; destruction also releases retained sample buffers [A3-DX-1800c9290, A3-DX-1800c76a0, A3-DX-1800c8700].

No application-level completion-fence or retirement/reuse proof was recovered on this path. Graphics-interface reference release, host-buffer ownership, renderer locking, cooperative cancellation, and GPU completion are different facts. Backend-managed lifetime rules were not analyzed. Exception cleanup remains incomplete.

## Editable masks: explicit deferral

This round does not answer editable mask alpha versus luminance interpretation, operation ordering, source/target transforms, color precision, or complete cache identity. The alpha-related display callbacks examined here concern selection or display overlays. They must not be generalized to layer or live-filter masks.

The queued bounded probe has concrete discovery seeds, not behavior findings:

1. `libpersona.dll` mask-raster content entry at `0x1867f19f0` and its renderable dispatch table at `0x19291bf28`.
2. `librenderer.dll` byte-RGBA current-mask-buffer entry at `0x1839d8990` and floating-RGBA counterpart at `0x18411d7f0`.
3. The renderer/raster calls actually reached from the selected mask content entry. Do not select a raster implementation from names alone.

The later probe should follow one simple mask input through its real consumer. It should establish editable dependency scope, coverage interpretation, transformations, ordering, and precision before comparing cache behavior. It must remain separate from selection-display masks and exact export. No later mask analysis was started.

## Independent AFFINT opportunities

These are independent AFFINT verification and design choices, not vendor algorithms or defect claims. The prototype already has original typed revisions, precise declared masks, bounded history, and explicit queue ownership. No vendor representation is proposed as a replacement.

1. Validate revision, view request, dependencies, and quality at the actual presentation boundary, including after deferred texture upload. Earlier worker acceptance is not enough. Reuse AFFINT's existing typed identities.
2. Separate last-frame display policy from exact-output policy. An intentionally retained frame should not satisfy an exact render/export request or become a falsely current cache entry.
3. Keep output ownership and its byte lease alive while the UI or an upload retains samples. Account for simultaneous sample buffers, replacement textures, queued staging, and resources awaiting retirement.
4. Couple graphics-resource reuse to the selected backend's completion contract. Cancellation or device unassignment alone must not authorize premature reuse.
5. Give edit, undo, redo, seek, and branch changes one documented render-invalidation/publication relationship. A protected buffer swap and a controller-association check solve different problems.
6. Verify cached and uncached mask behavior using AFFINT's declared alpha/linear-luminance policy. Do not alter that policy based on the unrelated display overlays examined here.

Coordinator-owned synthetic checks should delay a finished result before reception, delay upload after reception, then commit an edit or seek history. Repeat with unchanged image dimensions, changed viewport/quality, and branch creation. Check that the current result wins, dirty work is retained, rejected results release leases, and cancellation does not release resources still in use. A separate mask suite should cover local transforms, hidden mask nodes, crops, translucent edges, operation order, and full-resolution cached/uncached agreement. This round ran none of those checks.

## Tools and limitations

REA CLI `6.4.0` was recorded. The investigation reused the successful direct focused Ghidra route instead of repeating prior whole-library timeouts. No new native REA Evidence result or active REA binary session was created.

Six Ghidra `12.1.4` invocations ran sequentially. Each used a 240-second outer bound, a 25-second per-function decompiler bound, and one analysis CPU. All exited with status zero. All owned headless processes settled. No additional native analysis was started after the sixth invocation.

The canonical record contains 41 distinct functions and 91 source-equal mapped slices, totaling 34,509 bytes with overlap. Including the superseded import-limited bridge pass, retained output contains 44 function records and 101 slices totaling 38,078 bytes. This is bounded coverage, not a fraction-of-app completeness claim.

Initial PE parsing omitted later imports. Supplemental raw descriptor/thunk parsing verified 9,438 normal persona imports, 13,039 normal bridge imports plus 116 delay imports, and 1,359 DirectX imports. The final bridge pass uses the expanded labels. Initial output remains retained with its label limits; verified imports do not retroactively repair inferred decompiler types.

ILSpy `11.1.0.9782` supplied metadata and focused managed reconstruction. Its CIL member selector emitted a whole-assembly listing despite the requested member. That raw output remains restricted; only the separately extracted 357-byte notification method is used here. Windows managed dependencies were not fully resolved.

Important remaining limits:

- The base tile destruction body truncates after its teardown-marker store. No complete destruction ordering follows from that record.
- Result placement and dirty notification contain unresolved terminal indirect-flow warnings. The relevant normal-path stores and lock operations are narrower evidence.
- Ghidra reports overlapping-global annotations and incomplete parameter/type recovery. Completed decompilation is not a correctness proof.
- Split/cold branches have source bindings, but indirect targets, registered notification handlers, language-specific exception cleanup, and all graphics backend behavior remain incomplete. Unwind metadata was retained, not fully interpreted.
- The concrete callback route is bounded; edit/history-seek ordering, completion fences, live backend attribution, editable-mask semantics, and exact export remain unresolved.

No old report or evidence was changed. Raw output remains ignored under the owned restricted directory. Future work is queued as questions only.
