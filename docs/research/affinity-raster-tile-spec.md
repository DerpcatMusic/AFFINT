# Installed Affinity: raster ownership and view-tile lifecycle

Status: bounded static round 2, 2026-10-12. This report extends the history investigation without changing its first report. It contains independently worded findings, not vendor source or decompiler output.

## Result and boundary

The examined history clone route shares raster blocks through retained ownership. A clone is therefore not necessarily a copy of pixel bytes. The separate copy route attempts raster evaluation into a new backing. Its failure branch records a different backing status. That branch prevents interpreting the method name as proof of a successful deep copy.

Tile invalidation preserves dirty work and requests cooperative cancellation. The worker checks a renderer abort flag before a display-result callback. It also checks one captured dependency identity before updating a retained cache. These checks are distinct.

This round does not prove that an obsolete document revision can never reach the display. The concrete display callback, edit-to-tile notification route, and history-seek invalidation route remain unresolved. No vendor defect follows from that gap.

All findings are static. No Affinity process, installed helper, application test, GPU probe, or Wine UI ran. No AFFINT compilation, test, benchmark, formatting, or build ran. There were no installations, configuration changes, nested workers, or account/license investigations.

A backing holds raster storage for an editable object. A block holds shared sample storage. A tile covers a bounded image region. A dirty region records work that needs replacement. A dependency identity distinguishes the input behind a cached result.

Observation means that source-bound instructions or metadata support the fact. Interpretation means that the relationship follows from several static observations. Unknown means that the examined evidence does not settle the question. None of these levels constitutes runtime proof.

## Source identity and evidence

The source directory remains `<local Affinity installation>`. Both examined files match their round-01 hashes before slicing and at final verification.

| File | Bytes | SHA-256 |
|---|---:|---|
| `libpersona.dll` | 359,297,544 | `285b2b2634029ddb05e7ef436a4da5aefec973362187a88e848b361d22ee1678` |
| `libraster.dll` | 71,407,112 | `ee3904ba5b05010bbf7d763ae407f594c8bf0245f0106a9b015c5e6bf6dc4d84` |

The installed set was identified as version `3.2.3.4646` in round 1. This round does not authenticate a pristine vendor distribution. It excludes the older `.bak` assembly.

Restricted evidence resides under `.research/vendor-affinity/round-02/`. Implementors consume this report, not those files. The evidence register binds each local ID to the source hash, address, file offset, byte digest, body ranges, and warnings.

`A2-P-<address>` identifies a `libpersona.dll` function. `A2-R-<address>` identifies a `libraster.dll` function. Addresses are preferred PE virtual addresses, not live relocated addresses. These are local evidence IDs, not REA Evidence IDs.

| Evidence | Address or local record | Coverage |
|---|---|---|
| A2-P-1846ce070 / A2-P-1846ce740 / A2-P-1846cf200 | `0x1846ce070`, `0x1846ce740`, `0x1846cf200` | History clone, copy, and empty-backing entry points. |
| A2-P-1846c9c20 / A2-P-1846ca0d0 | `0x1846c9c20`, `0x1846ca0d0` | Latest and indexed backing assembly. |
| A2-P-1818e3300 / A2-P-1846ae6d0 | `0x1818e3300`, `0x1846ae6d0` | Examined RGBA8 history clone/copy specializations. Other format branches were not traced equally deeply. |
| A2-P-1818afda0 / A2-P-181cdba60 | `0x1818afda0`, `0x181cdba60` | Host-cache synchronization, per-plane tile transfer, and tile assignment on the clone route. |
| A2-P-1846bc6a0 | `0x1846bc6a0` | Hardware path for the examined copy specialization, including guards, retained buffers, and submission. |
| A2-R-180bd9f50 / A2-R-1814ef8c0 / A2-R-1814fe460 | Export thunks `0x18005e54d`, `0x18005b1ef`, `0x18003913f` | Dynamic clone, RGBA8 bitmap clone, and plane-copy boundary. IDs name resolved bodies. |
| A2-R-18051bb90 / A2-R-18051ba20 | Export thunks `0x18005aae2`, `0x18006d273` | Tile assignment and block-owner assignment. |
| A2-R-181507e70 / A2-R-18269c030 / A2-R-1825f2870 | Bodies `0x181507e70`, `0x18269c030`, `0x1825f2870` | RGBA8 bitmap, red-plane region, and tile deduplication. |
| A2-R-1825f26c0 / A2-R-1825f2530 / A2-R-182df9306 | Bodies `0x1825f26c0`, `0x1825f2530`, `0x182df9306` | Full-byte comparison, replacement ownership, and the comparison import boundary. |
| A2-P-1915ef010 / A2-P-1915f1ea0 / A2-P-1915fb4a0 | `0x1915ef010`, `0x1915f1ea0`, `0x1915fb4a0` | Begin, launch, worker evaluation, and normal completion. |
| A2-P-1915eefb0 / A2-P-1915f1490 / A2-P-1915f14f0 / A2-P-1915f19f0 | Corresponding addresses | Abort, hard abort, full invalidation, and region invalidation. |
| A2-P-1915f3a40 / A2-P-1915f3dc0 / A2-P-1915ee6c0 | Corresponding addresses | Dependency replacement, invalidation, and captured ownership. |
| A2-P-1915efac0 / A2-P-187fd4a80 / A2-P-187fd4ab0 | Corresponding addresses | Display geometry and retained byte/floating buffer access. |
| A2-P-1915ef690 / A2-P-1915eede0 | Corresponding addresses | Thread-device unassignment and job cleanup. |
| A2-I | `native/libpersona.dll.full-imports.json`, `native/libraster.dll.full-imports.json` | Supplemental source-bound import-table parsing. |
| A2-V | `native/viewtile-selected-vtables.json` | Selected base-class callback entries. Not a concrete live subclass. |
| A2-U | `native/unwind-index.json` | Exception-unwind headers and metadata prefixes. Language-specific cleanup remains untraced. |
| A2-VERIFY | `verification.json`, `evidence-register.json` | Two unchanged originals and 143 source-bound segments across seven canonical extraction plans. |

Ghidra 12.1.4 produced 67 distinct function records. Each reports completed decompilation. That status does not mean complete control flow, correct parameter types, or complete exception coverage. Important warnings appear in the limitations section.

### Selected entry-range digests

These digests bind initial source ranges. Split body ranges have separate bindings in the restricted plans. They are not hashes of an entire recovered C++ method.

| Local evidence ID | Source file offset | Initial-range SHA-256 |
|---|---|---|
| A2-P-1846ce070 | `0x46cd470` | `56c419c4d85b726b6b627a306bc812217670a28741f42655df8c52414ec76a35` |
| A2-P-1846ce740 | `0x46cdb40` | `fcb39e58d4d776379fe797f91550b465302ce61a42d5dbf6d91b1fdd062ffa5b` |
| A2-P-1846c9c20 | `0x46c9020` | `461ec06bba1d9e57e5f87eb92b1b62ef017947051fb2d84316835309cd72a243` |
| A2-P-1915fb4a0 | `0x115fa8a0` | `59f00d24a89a4677c356a1f128d0c78476044f521bd518383acb5780155212e9` |
| A2-R-18051ba20 | `0x51ae20` | `aabdc0034186e76875339c6b6f6e4f097c378b453539f036c4ee5d976c42c662` |
| A2-R-1825f2870 | `0x25f1c70` | `932dc13ff95834adccdb822c317ea704eafd695202b1b73ec31aba0a5d6c8cd1` |

## Mutable raster history

### Clone is shared ownership on the examined route

The clone entry obtains the current bitmap through the mutable-raster interface. It creates a separate backing container with the current dimensions and format. It dispatches work according to that format [A2-P-1846ce070].

For RGBA8, the normal host route synchronizes the source host cache. It transfers corresponding tiles independently for red, green, blue, and alpha planes. It then invalidates the destination hardware cache [A2-P-1818afda0, A2-P-181cdba60].

The transfer reaches tile assignment. The underlying block-owner assignment retains the same block pointer and releases the prior owner. It does not copy the block payload at that boundary [A2-R-18051bb90, A2-R-18051ba20].

This is evidence of block sharing, not a conclusion based on the word `Clone`. New containers and region metadata still incur allocation and traversal. Synchronizing a host cache can also incur transfer work. No timing or byte-traffic measurement exists.

The block-owner type is named `COW`, meaning copy-on-write. Its inspected assignment shares ownership. The mutation-time detachment operation was not traced. This report does not assert that every raster mutation preserves shared samples correctly.

Dynamic bitmap clone creates another format-dispatched wrapper. Its RGBA8 branch creates another bitmap container, copies plane state, and traverses existing mipmap entries [A2-R-180bd9f50, A2-R-1814ef8c0]. A mipmap is a smaller derived image. Its presence does not prove reduced source storage or reduced export evaluation.

### Copy has a separate execution and failure policy

The copy entry also creates a backing with source dimensions and format. It selects different per-format helpers from the clone entry [A2-P-1846ce740].

The examined RGBA8 helper can attempt hardware evaluation. The lower path checks hardware support, synchronizes hardware caches, retains source and destination hardware buffers, and calls a kernel-execution interface. On reported success, it invalidates the destination host cache [A2-P-1846ae6d0, A2-P-1846bc6a0].

This is an evaluated transfer path rather than the tile-owner assignment described above. It does not establish a whole-image host `memcpy`, an exact transferred-byte count, or execution on this machine. The kernel implementation and all backend variants remain unexamined.

The outer copy entry inspects a completion flag from the transfer. When the flag is false, it creates another backing and records a different status. That record lacks a demonstrated successful transfer [A2-P-1846ce740].

The complete consumer meaning of the status remains unknown. It must not be described as completed deep-copy storage. The entry does not expose a cancellation argument. Its helper context supplies no external cancellation pointer on this examined call.

The clone and copy entries both retain a description and source inflation values beside the backing. Inflation records how the raster extends beyond its original tile origin. Different status values and a complete-backing flag distinguish stored representations. The proprietary representation is not an AFFINT format proposal.

### Assembly depends on backing representation

History associates a sequence of backing records with the mutable-raster interface pointer. This is a local native pointer key, not proof of a persistent document identifier [A2-P-1846c9c20, A2-P-1846ca0d0].

For a record marked complete, latest assembly returns the retained bitmap with another ownership reference. It does not reconstruct the pixels at that boundary.

For the other representation, latest assembly clones the current bitmap and applies the latest retained backing through a format-specific operation. Indexed assembly checks the requested index. It can return a complete record directly or start from current storage and apply retained records backward toward the requested position.

Indexed assembly adjusts placement using the difference between stored and current left/top inflation values. A backing therefore cannot be treated as an origin-free pixel blob. Dimensions, format, and coordinate extent belong to the reconstruction contract.

The interpretation is that history supports both complete references and reconstruction-dependent payloads. The exact contents of every non-complete payload, failure-status handling, and all format variants remain unresolved. The latest-assembly decompiler also reports unstable type propagation.

### Deduplication compares contents before sharing

The examined RGBA8 bitmap deduplicator visits planes and retained mipmap entries. Region traversal reaches individual tiles. Cancellation checks occur between several traversal boundaries [A2-R-181507e70, A2-R-18269c030].

Tile deduplication excludes unavailable, disposed, and pending-load blocks. It uses CRC32 values to select candidates. It also compares the full 65,536-byte payload before replacing a candidate owner. The comparison reaches the imported `memcmp` interface [A2-R-1825f2870, A2-R-1825f26c0, A2-R-182df9306].

A matching CRC32 is not sufficient on this path. Replacement retains the chosen block and releases the old owner. A separate path preserves an existing backing reference when selecting certain shared canonical tiles [A2-R-1825f2530, A2-R-1825f2760].

The RGBA8 transfer helper uses a 256-sample tile span in each axis. The deduplicator compares 64 KiB block payloads. These are scoped storage observations, not a universal tile size for every format, renderer, or file.

Cancellation can stop later deduplication work after earlier replacements. No rollback of those replacements appears in the examined normal path. Sharing equal payloads is a storage change, not proof of atomic document mutation or full cancellation safety.

The candidate collection, counted owners, and traversal sizes do not establish a global byte budget. Deduplication savings, collision frequency, contention, and peak residency were not measured.

## View-tile lifecycle

### Begin and launch capture work

The normal begin entry refuses work when the tile is hard-stopped, paused, or lacks a document controller. It obtains tile extents and rejects empty work. It claims an in-progress state through compare-and-exchange [A2-P-1915ef010].

The tile holds separate normal and higher-quality render-target references. Begin resolves dirty regions against their tile extents and captures regions for launch. Full invalidation and region invalidation mark new work separately from those captured regions.

The fallback tile rectangle is 256 by 256. The higher-quality fallback is 512 by 512. An attached tiled view supplies its own rectangles [A2-P-1915fdb30, A2-P-1915fdb70]. These are viewport-buffer observations, not source-resolution or export limits.

Launch allocates a retained request. Its inputs include the tile, render target, transforms, output and dirty rectangles, mode flags, a cache-trigger reference, and an optional node reference. The tile and several inputs acquire native ownership references [A2-P-1915f1ea0, A2-P-1915ee6c0].

No immutable document-revision token was established in this examined request. The worker also reads current controller and view state during execution. This is not proof that revision authority is absent elsewhere.

Launch checks an active-job count against a configured limit under a lock. It either dispatches work or queues the request. Different routes use separate queue collections. The configured limit and total queued-byte bound were not traced.

### Invalidation and cancellation are separate from completion

Full invalidation marks the tile invalid, marks both regions for refresh, and stops the higher-quality timer. If no job is active, it attempts begin. If a job is active, it sets the abort state on the retained render targets [A2-P-1915f14f0].

Region invalidation transforms a supplied logical region into the tile coordinate systems. It combines the region with pending work and clips it to tile extents. It then follows the same begin-or-abort decision [A2-P-1915f19f0]. Exact edge rounding and filter expansion remain unverified.

Abort stops the timer and requests cancellation on the retained render targets. It does not join the worker or release every result immediately. Hard abort additionally sets a persistent stop field. Pause sets the pause field and requests abort [A2-P-1915eefb0, A2-P-1915f1490, A2-P-1915f33c0].

The inspected base `EndDraw` is empty. The job-cleanup helper invokes a virtual completion callback, decrements the active-job count, services queued work, and releases request ownership [A2-P-1915f09f0, A2-P-1915eede0]. Concrete completion behavior remains a subclass responsibility.

Thus, an accepted abort request and settled resource ownership are different events. This evidence does not prove the cancellation latency or exception-time cleanup sequence.

### Worker output and display drawing are distinct

The worker establishes rendering-thread and hardware-session scopes. It obtains or creates normal/higher-quality output buffers from view extents and the controller format. It also supports degraded-render state and follow-up evaluation [A2-P-1915fb4a0].

Degraded results can trigger another launch rather than completion of all requested quality. These viewport paths must not be used as evidence about exact full-resolution export.

The worker distinguishes byte RGBA output from a floating-component output route. The texture getters return retained typed buffers. They do not themselves return an identified Direct3D texture [A2-P-187fd4a80, A2-P-187fd4ab0]. Exact floating storage width, color policy, and accelerated/reference tolerances remain unverified.

`Draw` prepares transformed, clipped display polygons and invokes virtual display operations under a display lock [A2-P-1915efac0]. Its arguments include a visible rectangle, transform parameters, optional clipping, and display flags. It is not the raster-history copy operation or an export pipeline.

### What protects a result from becoming stale?

The worker reads the renderer abort flag after raster work and before the typed display-result callbacks. Source-bound checks appear at `0x1915fcf2c` through `0x1915fcf39`. A captured request mode can also suppress those callbacks [A2-P-1915fb4a0].

One retained-cache update compares the current cache-trigger pointer against the pointer captured in the request. That comparison occurs under the dependency lock at `0x1915fc6b4` through `0x1915fc6bb`. This is evidence of dependency identity at that cache boundary.

At normal completion, the worker considers both the sampled abort state and a tile-invalid field. If either requires replacement, it releases the in-progress state and attempts another begin when the tile is not paused. The corresponding checks start at `0x1915fd84f` [A2-P-1915fb4a0].

These observations support cooperative cancellation, dependency-aware cache replacement, and dirty-work rescheduling. They do not establish one atomic publication check against the current document revision.

The result callbacks occur through virtual table slots for byte and floating buffers. The inspected base slots point to a runtime stub. The concrete subclass was not identified [A2-V, A2-P-1918835f0].

An edit or history seek could have additional locks, ordering guarantees, or callback checks outside this slice. Conversely, an abort check alone does not prove that publication cannot race a later edit. Both possibilities remain unresolved.

Answer to the assigned stale-revision question: unknown at the end-to-end boundary. Do not report a stale-tile defect or a stale-tile safety guarantee from this round.

## Hardware handoff and retirement

The raster copy path carries a semaphore guard and a counted `Prolongation` object around retained hardware buffers. It checks cancellation between several preparation steps and submits work through the hardware-kernel interface [A2-P-1846bc6a0].

This establishes an explicit lifetime carrier around asynchronous-capable hardware work. Its destruction, fence completion, buffer reuse rules, and backend implementations were not traced. A native ownership reference alone does not prove that the GPU finished reading the resource.

`ClearGPUAssignment` saves the current thread device, temporarily selects the assigned device, and calls thread-device unassignment. It then restores the previous device and clears the assignment fields [A2-P-1915ef690].

That method is a thread-device assignment boundary. It is not evidence that every texture or raster allocation is freed. Its callers and complete retirement ordering remain unknown.

The worker includes host-cache synchronization in a CPU color-conversion branch. It also invalidates the corresponding hardware cache after host conversion [A2-P-1915fb4a0]. Thus, static code supports both host and hardware cache transitions.

This does not establish a routine full-frame readback, absence of readback, or an active Wine backend. Actual transfers, devices, timings, and residency require authorized runtime evidence.

## Independent AFFINT opportunities

These are AFFINT design choices. They are not proprietary implementation instructions or vendor-defect claims.

1. Keep immutable source blocks separate from mutable operation state. Clone ownership references for history, not whole pixel buffers. Detach only blocks that a committed mutation changes. Reserve bytes before detachment.
2. Give every backing an explicit representation type. Distinguish complete snapshots, patches, and failed or pending transfers. Do not represent all outcomes as a bitmap plus an undocumented boolean.
3. Preserve raster extents as well as dimensions. Include origin, halo, format, precision, and sample layout in patch reconstruction. Test undo after extent growth and origin changes.
4. Separate cancellation from publication. A result must pass a current revision/request check at the actual display or cache insertion boundary. Recheck after uploads or conversions that defer handoff.
5. Keep dependency identity complete. Include document revision, source identity, operation parameters, region, transform, color policy, precision, quality, and device identity where each changes meaning.
6. Retain dirty work while another job runs. Combine new regions without clearing them when older work finishes. Expand effect dependencies before clipping the resulting work.
7. Distinguish exact jobs from viewport previews. An exact export cannot accept degraded work or a display cache merely because the dimensions appear compatible.
8. Keep byte leases alive through worker results, UI ownership, and GPU retirement. Budget shared blocks once, independent outputs separately, and queued staging before dispatch. A job-count limit is not a byte limit.
9. Couple GPU resource retirement to completion fences. Keep cancellation and device unassignment separate from permission to reuse or free buffers.
10. Measure synchronization and transfers before optimizing. Record shared-block counts, copied bytes, uploaded/read-back bytes, queued bytes, peak residency, and cold/warm timings.

Coordinator-owned tests can exercise these contracts with synthetic data. Priority cases are shared-history isolation, CRC-collision rejection, branch/seek cache invalidation, dirty work arriving during cancellation, and delayed GPU publication. Full-resolution cached/uncached agreement must cover extent changes, translucent borders, masks, and quality identity. This research round ran none of those tests.

## Tools, failures, and coverage limits

REA CLI 6.4.0 was available and its version was recorded. This round reused the direct focused Ghidra route because prior whole-library attempts exceeded their bounds. There was no new whole-library import and no native REA Evidence result.

Eight native analysis invocations ran sequentially. Each used a 240-second outer bound and a 25-second per-function decompiler bound. Each exited with status zero. No two heavyweight analyses ran together.

The analysis mapped small original-byte slices at their preferred addresses and disabled whole-program analysis. Final verification matched 143 slices, totaling 68,850 bytes across plans. That total includes overlap between plans and is not unique coverage.

The first slice-discovery attempt linearly decoded inline switch-table bytes as instructions. It generated invalid branch candidates. The corrected script follows reachable instruction flow and rejects non-file-backed ranges. The incorrect initial plan remains restricted and was never submitted to Ghidra.

Supplemental PE import parsing recovered 9,438 symbols for `libpersona.dll` and 846 for `libraster.dll`. The prior parser output omitted later imports needed for this subsystem. Original import descriptors and hint/name locations supplied the additional names [A2-I]. There were no delay-import entries in the bounded check.

Ghidra requires a project path without a dot-prefixed component. A temporary symbolic path points to the persistent owned project directory. Unique evidence, user-home state, and Java temporary files remain under round 2. The project path alias is not the evidence store.

The first evidence-verification script stopped when Ghidra omitted a null unwind field. The corrected verification uses an optional lookup and completed. Both outcomes are recorded.

Important residual limits:

- `ResumeDraw` and `WantsGPUEnabled` contain bad-instruction/truncated-flow warnings. Their full behavior is not claimed.
- Dynamic bitmap deduplication, hardware-buffer destruction, and inflation getters contain unresolved indirect-jump warnings. Typed RGBA8 bodies and history call arguments provide narrower evidence.
- Latest backing assembly contains a type-propagation warning. Assembly observations require the source-bound instructions and surrounding calls, not inferred types alone.
- Large-stack routines contain unresolved compiler stack-probe calls. Several recovered signatures and stack expressions are unreliable. Imported call identities and explicit machine-code gates are stronger evidence.
- Local split/cold branches are included within a bounded range count. Indirect targets, language-specific exception tables, and cleanup funclets are not complete.
- Static normal-path ownership does not prove exception safety, thread safety, atomic publication, or a global memory limit.
- The investigation does not cover every format specialization, kernel implementation, concrete display subclass, or export path.

## Ordered next probes, not executed

1. Identify the concrete view-tile subclass that implements the byte/floating result callbacks. Trace its request validation, texture upload, UI publication, and retirement fence. Keep the task inside rendering and exclude account/license code.
2. Trace edit and history-seek notifications into tile invalidation. Establish the shared lock or revision authority across mutation, cancellation, and the concrete publication boundary.
3. Trace block-owner mutation-time detachment and one region-clone implementation. Prove preservation of samples shared with history, including partial writes, lazy loading, and host/hardware cache transitions.
4. Resolve the copy-entry failure status through its consumers. Establish whether it records pending recovery, a reconstructible empty state, or another policy before using it as a behavior contract.
5. Trace `Prolongation` destruction, hardware-buffer retain/release, and one completion fence. Establish when cancellation permits buffer reuse, device unassignment, and allocation retirement.
6. Trace an exact export request independently from view tiles. Establish requested dimensions, precision/color policy, degraded-mode exclusion, and error propagation.

Later authorized runtime work can test the static hypotheses with original synthetic fixtures. No follow-up probe or runtime experiment was started by this report.
