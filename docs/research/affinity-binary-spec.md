# Installed Affinity: binary interface and history findings

Status: bounded round 1, 2026-10-12. The owner authorized static reverse engineering of the installed application. This report contains independently worded observations, not vendor source or decompiler output. AFFINT implementors consume this report, not the restricted raw evidence.

## Scope and evidence rules

This round maps the installed binaries and traces document history through the managed interface, native command queue, undo/redo, branch retention, and serialization. It also identifies targets for document structure, masks, rendering, persistence, and GPU work. It does not establish complete format compatibility or application parity.

A managed assembly contains .NET metadata and executable intermediate instructions, called CIL. A native library contains machine instructions. A mixed assembly contains both. A dispatch table selects a native method through an object pointer.

All findings here are static. No Affinity process, test executable, installed helper, or GPU probe ran for this investigation. No AFFINT build, check, test, or benchmark ran. License, account, credential, token, and Canva systems were outside the investigation. Vendor artwork and resources were not extracted for product use.

Evidence levels used below:

- Observation: A file field, metadata declaration, instruction, or source-bound data value was read.
- Interpretation: Several observations support the stated relationship, but execution did not demonstrate it.
- Discovery: An exported name or import identifies a useful next target. The method body was not examined.
- Unknown: The available evidence does not establish the claim.

Native decompiler output is inferred pseudocode, not original source. Parameter types, implicit object adjustments, and exception behavior remain incomplete. A reported successful decompilation is not a correctness proof.

## Evidence register

Restricted records are under `.research/vendor-affinity/round-01/`. Raw records stay ignored by Git. These paths identify evidence for assigned research workers only.

| ID | Local record | Authority and coverage |
|---|---|---|
| I1 | `inventory/files.json`, `inventory/pe-map.json`, `inventory/*.linkage.json` | Read-only file inventory and PE parsing with pefile. Selected hashes, sections, imports, exports, and version resources. Export parsing was expanded beyond the parser's default limit. |
| R1 | `managed/Affinity.exe.identity.json` | REA 6.4.0, `inspect_managed_artifact`, complete launcher identity. Evidence `ev_f56a429e12bc3ba979960c13d80620736b8f3e0ffd418e7a60eee7b4072e3207`. |
| R2 | `managed/Serif.Interop.Persona.dll.identity.json` | REA static CLI header, partial metadata. Evidence `ev_0f7bb0a2a13349ea93eed9419612ce938dca68c08bea66dd8b8e130ab8cd28fc`. |
| R3 | `managed/Serif.Affinity.dll.members.json`, `managed/Serif.Interop.Persona.dll.members.json`, `managed/Serif.Interop.Persona.dll.boundaries.json` | REA representation-budget failures. Member and native-boundary declarations are unavailable, not absent. |
| M1 | `managed/*.typedefs.ilspy.json`, `managed/*.assembly.ilspy.json` | Separate ILSpy 11.1.0.9782 metadata inventory. This is not canonical REA member evidence. |
| M2 | `managed/HistoryDataSource.ilspy.cs`, `managed/HistoryItem.ilspy.cs`, `managed/HistoryPage.ilspy.cs` | Focused ILSpy reconstruction of the current managed history interface. Windows framework dependencies were not fully resolved. |
| M3 | `managed/Tool.ilspy.cs`, `managed/*.focused.ilspy.il` | Mixed-assembly wrapper reconstruction and selected CIL methods. CIL confirms indirect native dispatch and managed lifetime handling. |
| N1 | `native/libpersona-history-r01/functions.json`, corresponding assembly and pseudocode files | Focused Ghidra 12.1.4 analysis of 45 native entry points. Each record identifies its original address, file offset, and byte digest. |
| N2 | `native/libpersona.slices.plan.json`, `native/libpersona-slices/`, `scripts/slice_history.py` | Derived, source-anchored byte slices. The final plan maps 37,752 bytes in 71 code/data slices. Local split-body branches and fallthroughs are included. |
| N3 | `native/history-vtables.json` | Source-bound dispatch-table entries. Selected primary Tool and secondary DocumentController entries corroborate the interface. Early fixed-length probes include neighbors beyond some secondary tables and are not complete table layouts. |
| Q1 | `native/next-probe-entrypoints.json`, `next-round.json` | Symbol discovery and the ordered next investigation. No behavior claims arise from these records alone. |
| P1 | `provenance.json`, `artifact-digests.json` | Tool versions, analysis limits, source identities, and retained artifact hashes. |

N1–N3 are local Ghidra/PE evidence. They do not have REA Evidence IDs. REA native analysis did not finish within its time bound.

The native route uses original bytes at the preferred PE addresses, not a live relocated image. Full-library import and whole-library analysis exceeded separate 240-second bounds. The successful route used small derived slices and disabled whole-program analysis. It retained the Ghidra project in the restricted evidence directory. A temporary path alias was necessary because Ghidra rejects project paths containing a dot-prefixed component. No unique evidence resides only at that alias.

An early slice pass omitted split-function fallthroughs. The corrected pass follows those ranges and has no bad-instruction truncation warnings for the selected 45 entry points. This fixes that coverage defect, not unknown callees, types, indirect targets, or exception paths.

## Installed artifact identity

The installation root is `<local Affinity installation>`.

I1 enumerates 1,268 regular files and 122 PE images. The PE images comprise 69 native images, 51 IL-only assemblies, and two mixed assemblies. There are 85 PE images at the root. The remaining 37 are below it. Classification uses the PE CLI header and IL-only flag, not filename suffixes.

The launcher, current managed UI assembly, and mixed bridge report assembly version `3.2.3.4646` [R1, M1]. The launcher declares .NET Framework 4.8 and x86-64. Its 11,446,280-byte file has four metadata TypeDef rows and 58 MethodDef rows. Most of its bytes are embedded resources. It is not the main native document implementation [R1].

The nearby `Serif.Affinity.dll.bak` reports `3.1.0.4231`. It is not a matching backup for the current assembly. Its identity was inventoried, but its implementation was not used for the history findings. The directory also contains installation-specific helper files. Hashes bind this installed set, not an authenticated pristine vendor distribution [I1].

### Selected target hashes

| Target | Bytes | SHA-256 |
|---|---:|---|
| `Affinity.exe` | 11,446,280 | `0b8f42e7d27eb2b50dff4ca9e357b7fc1749cf6e871a453f7473a8fd685bf68d` |
| `Serif.Affinity.dll` | 15,636,992 | `840c1d976699505f506d3b095b8e13e32858ef34492e8728b1476b32fd7ca312` |
| `Serif.Interop.Persona.dll` | 59,277,832 | `dcf43441e01dbf3fbe83ccb3e3b3279929d89ad8ea9f10a76745aea6a16219dc` |
| `libpersona.dll` | 359,297,544 | `285b2b2634029ddb05e7ef436a4da5aefec973362187a88e848b361d22ee1678` |
| `libaffinity.dll` | 24,856,072 | `b6e38da9de61c6a395458710a4bc27b6028b6e91498dc12c2d30eec96b414376` |
| `libkernel.dll` | 7,015,944 | `0caeba0ee08f23764d26745c3c0a3b1cf086a7596c0620486bd1b965e9a3182e` |
| `libraster.dll` | 71,407,112 | `ee3904ba5b05010bbf7d763ae407f594c8bf0245f0106a9b015c5e6bf6dc4d84` |
| `librenderer.dll` | 81,864,712 | `4d087c59b50c1213597bc78cf9b4e24ff8725a176bfed4f0d51828378e617bb6` |
| `Serif.DirectX.dll` | 1,314,312 | `fe149204fb3f0ea4b92924b5b0296a7466c937c909968ffcaaf71cabeef6802f` |
| `libdxcore.dll` | 50,184 | `71fd5afb2ce912e6d6346df1a8454916167e58a109a4b4cc27441552f4a3d065` |

I1 contains additional selected hashes, including command, raster-tool, geometry, text, book, and scripting libraries. PE timestamps were not treated as trustworthy release dates.

### Concrete binary map

| Boundary | Evidence-backed role | Examination level |
|---|---|---|
| `Affinity.exe` | Managed launcher/resource container that references `Serif.Affinity` and `Serif.Interop.Persona`. | Metadata. |
| `Serif.Affinity.dll` | Managed Windows UI. History rows and page controls use a current-view Tool interface. ILSpy inventories 1,560 TypeDef rows. | Focused history reconstruction and CIL. |
| `Serif.Interop.Persona.dll` | Mixed CLR/native bridge. IL-only is unset and its CLI entry is native. ILSpy inventories 45,673 TypeDef rows, including compiler/native proxy types. | Metadata and selected Tool wrappers. |
| `libaffinity.dll` | Native application/controller boundary. It imports `libpersona`, `libcommands`, `libbook`, and other shared libraries. It exports a history-panel controller. | Linkage only. Its history-panel controller was not the managed history route traced here. |
| `libpersona.dll` | Native document controllers, command lifecycle, history, node interfaces, and view-tile operations. It imports raster, geometry, kernel, and several import libraries. | Forty-five history/queue functions plus linkage. |
| `libcommands.dll` | Additional document-edit commands, including selection-oriented undo/redo interfaces. | Linkage only. |
| `libkernel.dll` | Serialization, archive access, counted ownership, dispatch, locks, timers, and other shared facilities. | Imported calls and exported interfaces. Their internal implementations remain untraced. |
| `libraster.dll` | Dynamic bitmap/backing interfaces, tile operations, block deduplication, hardware-device access, and OpenCL discovery interfaces. | Linkage and calls from document history. |
| `librenderer.dll` | Rendering targets and effects, including mask-buffer interfaces. It depends heavily on raster and geometry interfaces. | Linkage only. |
| `librastertools.dll` | Raster editing tools and tool-specific history interfaces. | Linkage only. |
| `Serif.DirectX.dll`, `libdxcore.dll` | Native Windows graphics/device boundaries with Direct3D-related imports. | Linkage only. |
| `libgeometry.dll`, `libStory.dll`, `libbook.dll` | Geometry, text/story, and book-oriented interfaces. | Linkage only. |
| `libscripting.dll`, `libscriptingjs.dll` | Installed native scripting boundaries. | Identity and linkage only. No scripts ran. |

Export counts are not function counts. They include data, dispatch tables, aliases, and compiler-generated entities. The complete selected export inventories contain 50,359 entries for `libpersona`, 37,923 for `libraster`, and 5,112 for `librenderer` [I1].

## Deep trace: document history

### Managed history is a projection of the active tool

The managed history data source does not own an independent undo stack. It obtains the active Tool from the current document view. Row creation asks that Tool for a description, time, optional thumbnail, and alternate-future availability [M2].

The UI collection is virtualized. A full history notification clears derived rows and obtains the new count. An index notification updates the selected position without rebuilding all history state. A local update lock prevents those refreshes from submitting another seek. The history page installs its notifications while visible [M2].

Three positions must not be confused [M2, N1]:

- The native cursor is the number of applied history items.
- The managed selected row is the native cursor minus one. Row `-1` represents the position before the first item.
- The history slider uses selected row plus one.

A preview flag changes slider synchronization. It does not prove a particular hover-preview rendering policy. Snapshot markers are a separate interface, with capability and current-marker queries [M2, M3].

Native timestamps cross the wrapper as an unsigned 64-bit value. The wrapper interprets them as seconds after 1970-01-01 UTC and converts them to local time for display. This is a wrapper contract, not proof of the clock source [M3].

### Managed-to-native dispatch

The focused CIL for `Tool.SetHistoryIndex` has 38 instruction bytes at RVA `0x013a09c0`. It obtains the counted native Tool pointer, reads a table entry at byte offset `0x650`, and performs an unmanaged call. It then retains the managed wrapper through the call with `GC.KeepAlive` [M3].

The primary native Tool table belongs to its `Schedulable` base. Its source-bound entries corroborate the wrapper offsets [N3]:

| Table offset | Interface |
|---:|---|
| `0x628` | Number of history states |
| `0x630` | Current history cursor |
| `0x638` | Description |
| `0x640` | Time |
| `0x648` | Thumbnail |
| `0x650` | Set history row |
| `0x658` | Snapshot capability |
| `0x660` / `0x668` | Get / set snapshot row |
| `0x670` / `0x678` | Alternate-future availability / cycle request |

The native default Tool first asks for a tool-history provider. With a provider, it delegates. Without one, it reads document history or submits a document-history command [N1]. `ToolWithHistory` also has a separate, locked seek interface. A tool-local history is therefore not interchangeable with document history.

This identifies supported static routes. It does not identify the concrete Tool object active in a particular live session.

### Seek request to document command queue

The default Tool seek route constructs a `SetHistoryIndexCommand`. It submits that command through `PersonaController`, which obtains a document controller. The document controller forwards to its `DomainController` queue [N1].

The enqueue interface returns an acceptance result. Acceptance is not completion. The queue also inspects command capabilities and supports a deduplication check against queued commands. When the first command enters an empty queue, it schedules queue processing through the kernel dispatch facility [N1].

The queue processor separates its queue lock from another lock around mutation work. It calls document-specific pre-processing, waits through a reader-unlock path, processes the command, simplifies notifications, and delivers notifications. It signals queue progress before continuing. Native table entries identify the document-specific callbacks [N1, N3].

Document pre-processing creates an archive access block when an archive exists. Post-processing retires and releases that block. This links command processing to backing-store access lifetime. It does not establish filesystem durability or crash recovery [N1].

The examined queue contains blocking synchronization. Its exact thread assignment and contention costs were not observed. The numeric dispatch-purpose and priority arguments do not establish those costs.

### Seeking replays transitions

For a valid history row, `SetHistoryIndexCommand` compares the request with the current native cursor minus one. It traverses undo or redo items until it reaches the requested position. Each transition invokes the command's cache-aware undo/redo interface and obtains its notifications [N1, N3].

The inspected seek path does not replace a whole document bitmap at each step. It replays command transitions. Commands can still own large snapshots or raster backings. Their individual storage costs remain unknown.

Undo retrieves the item immediately before the cursor and moves the cursor backward when requested. Redo retrieves the item at the cursor and moves it forward when requested. The history object supports retrieval without advancing the cursor. The undo-item interface also has a removal mode [N1].

The cache-aware command wrappers combine the primary command with ancillary commands and text-composition state. Undo and redo have different ordering. They also conditionally update the current spread's artboard cache. Selection restoration remains untraced. These dependencies must not be reduced to a generic cursor increment [N1].

### History insertion, merging, and alternate futures

History insertion tests whether a command belongs on the undo stack. For a stackable command, it first separates the active future from the current path. It then attempts to merge with the previous item, or creates another item. Successful insertion or merging updates history thumbnail/time information and enforces the stack limit [N1, N3].

The merge interface is named `MergeWithTimeoutCheck`. This round confirms the call, not its timeout value or all merge eligibility rules. It is not proof that each user gesture always produces exactly one item.

A history item retains alternate futures. When a nonempty future is detached after a nonzero cursor, it becomes an alternate continuation of the preceding item. Branch records hold references to history items rather than a separate flat redo stack [N1].

Cycling the current future first retains the current continuation, then selects a stored alternate continuation and appends its items to the active path. The exact UI ordering of repeated branch cycles was not observed [N1].

The stack limit counts one unit per history item and recursively counts items in retained alternate futures. It obtains the limit from `PersonaController.GetStackLimit`. If the total exceeds that limit, it removes oldest items and adjusts the cursor and snapshot marker [N1].

This local limit is a command-count limit, not a byte-residency limit. Other memory controls elsewhere in the application remain unexamined. The finding is not a claim that Affinity lacks a memory budget.

## History structure and serialization

### Selected in-memory facts

These offsets are relative to the analyzed native history object or item pointer. They describe this installed build only. They are not an AFFINT representation or a complete C++ class definition [N1].

| Object | Relative location | Supported interpretation |
|---|---:|---|
| History | `0x10` onward | Active sequence of counted history-item references. The export signature identifies a deque. |
| History | `0x30` | Active sequence length, read by cursor checks. |
| History | `0x38` onward | Raster-backing association structure. Traversal and keyed lookup occur here. Its complete type remains unknown. |
| History | `0x48` | 32-bit applied-item cursor. |
| History | `0x4c` | 32-bit snapshot-history marker. `-1` is used as an invalid marker. |
| History item | `0x10` | Counted document-command reference. |
| History item | `0x18` | Counted thumbnail-buffer reference. |
| History item | `0x20` onward | Alternate-future sequence. Its exported access interface identifies a deque. |
| History item | `0x48` | 64-bit time value. |

Counted references acquire and release native ownership through `libkernel`. This establishes shared ownership at these boundaries. It does not prove that all pixels use copy-on-write or that all history payloads are immutable.

### Tagged object records

The serializers expose typed values, object arrays, class arrays, raw data, and path-enumeration modes. History save/load uses tagged fields, not only opaque whole-document snapshots [N1].

The following spellings describe logical numeric tags. The actual wire byte order and enclosing file format were not established.

| Logical tag | Numeric value | Observation |
|---|---:|---|
| `Posi` | `0x506f7369` | Four-byte history cursor value. |
| `Stac` | `0x53746163` | Active history-item sequence. |
| `SSHi` | `0x53534869` | Four-byte snapshot-history marker. |
| `Cmnd` | `0x436d6e64` | Serialized command for a history item. |
| `Tim2` | `0x54696d32` | Eight-byte history time value. |
| `Futu` | `0x46757475` | Alternate-future class array. |
| `Futr` | `0x46757472` | Alternate-future class identifier. The save call supplies schema value `1`. |
| `Itms` | `0x49746d73` | Item collection within a loaded alternate future. |
| `Thum` | `0x5468756d` | Optional thumbnail raw-data field. |
| `RBa1` / `RBa2` / `RBa3` | `0x52426131` / `0x52426132` / `0x52426133` | Distinct raster-backing representations accepted by the history loader. |
| `RBMI` | `0x52424d49` | Class identifier for one backing-entry representation. The save call supplies schema value `1`. |
| `RBaC` | `0x52426143` | Four-byte counts associated with raster-backing groups. |

One backing-entry loader reads bitmap, description, a boolean, and two four-byte offset values. The exact meanings of the boolean and offsets remain unresolved. Different backing representations are observable compatibility paths. Their introduction versions and migration guarantees are unknown [N1].

The item loader constructs a 64-by-64 buffer with 8-bit red, green, blue, and alpha components. The thumbnail save path writes `0x4000` raw bytes. Color space, alpha convention, and validation of malformed payload length remain unknown. This thumbnail does not imply any reduction of document source pixels [N1].

Full-save and path-enumeration modes are distinct. Some save branches walk referenced objects without writing their full payloads. This is a serializer contract, not proof that a user save operation uses incremental storage [N1].

Post-load processing associates loaded history entries with a document controller. Separate pruning interfaces remove unsupported history and alternate continuations. That is not evidence that unknown document layers or original assets are discarded [N1].

The outer `.af` archive layout, compression, object identity encoding, version negotiation, atomic replacement, and write-failure behavior remain unknown. No synthetic `.af` file was parsed or round-tripped in this round.

## Raster memory and GPU boundaries

History has dedicated interfaces for mutable raster backing: clone, copy, empty backing, indexed lookup, and assembly [I1, Q1]. Their names and signatures identify different policies. Their bodies were not traced, so the actual copy costs remain unknown.

The traced history deduplication method visits retained raster backings and history items. It passes a block-deduplication context to `DynamicBitmap.Deduplicate`. It also checks a cancellation flag between items. This establishes a cancellable deduplication path, not the block size, collision policy, savings, or thread safety [N1].

Raster and renderer linkage provide concrete next boundaries [I1]:

- `libraster.dll` exports OpenCL platform/device queries, GPU-enabled session types, hardware-device access, and tile operations.
- `Serif.DirectX.dll` imports `D3D11CreateDevice`, `CreateDXGIFactory1`, `D3DCompile2`, and `Direct3DCreate9Ex`. Its exports include typed texture-creation interfaces.
- `libdxcore.dll` imports `D3D11CreateDevice` and ordinal `101` from `d3d12.dll`.
- `libpersona.dll` exports view-tile draw, abort, GPU-assignment, and timing-query interfaces.

These are shipped static boundaries. They do not prove the active backend under Wine, GPU execution of a given filter, a residency policy, absence of readbacks, or CPU/GPU agreement. No installed `cltest`, `aftest`, or regression helper ran.

## Consequences for independent AFFINT design

These are AFFINT design opportunities, not vendor defects or instructions to reproduce vendor implementation.

1. Keep the committed history model outside UI rows. Project rows, descriptions, thumbnails, and selected positions from a document/history interface. Keep tool-local previews explicit.
2. Give seek requests a typed history position or revision. Convert UI row coordinates at the UI boundary. Do not spread the observed three-position convention through the document model.
3. Distinguish accepted work from completed edits. Publish revision-bearing results and errors. Reject obsolete worker results, as required by `AGENTS.md`.
4. Count retained branch and raster payloads in a byte budget. A command-count cap alone does not bound resident memory. Account for shared blocks once and for independent owners separately.
5. Make gesture grouping explicit. A timeout-dependent merge is not evidence that gesture boundaries are reliable. AFFINT can use its transaction interface to define one committed gesture.
6. Treat undo, redo, seek, and branch changes as render-input changes. Invalidate dependent caches and selection state through the same public mutation seam. Verify cached and uncached agreement.
7. Version operation and history records independently from the outer save container. Preserve or refuse unsupported content explicitly. Do not adopt the discovered proprietary tags as AFFINT's format.
8. Measure lock contention, history-seek work, backing copies, and transfer traffic before choosing a scheduler. Immutable committed snapshots are an AFFINT design choice, not a discovered vendor invariant.

No speedup, complete memory bound, transaction rollback guarantee, or exact rendering guarantee follows from this analysis.

## Open questions and objections

| Question | Status and objection |
|---|---|
| Concrete binary map and installed version | Answered for the inspected directory. Authenticity and a pristine-distribution comparison are not established. |
| One deep document/history subsystem | Answered as a static interface and data-flow trace. Live behavior and exception/failure paths remain unverified. |
| Document graph and identity invariants | Partial. Native `DocumentNode`, physical-root, counted/jail-counted, and controller interfaces are discovered. Cycle checks, identifier lifetime, and asset ownership are untraced. |
| Mask/filter/render dependencies | Discovery only. No mask/filter evaluation or dirty-region propagation was decompiled. |
| History branch persistence | Tagged serialization and branch structures are observed. A reopen oracle and cross-version compatibility remain unverified. |
| Mutation atomicity | Unknown. Queue locking and command success gates do not prove staged validation, rollback, or cancellation safety. |
| Raster sharing and residency | Partial. Shared ownership and deduplication calls are observed. Copy-on-write details, tile size, residency limits, and eviction are unknown. |
| GPU boundary and fidelity | Linkage only. Active devices, backend selection, transfers, precision, and full-resolution output are unknown. |
| Persistence durability | Unknown. Archive access blocks do not prove atomic replacement or failure-safe saving. |
| Current Blend/Mask Tool and scripting behavior | Not reassessed. Do not infer feature absence from this bounded history investigation. Preserve the public report's corrections. |

## Ordered next probes

The parent owns scheduling. `next-round.json` queues research questions only and starts no process or child agent.

### Next bounded round: mutable raster history and view-tile evaluation

Trace `DocumentHistory.PushMutableRasterBackingCloned`, `PushMutableRasterBackingCopied`, and `AssembleMutableRasterBacking`. Compare their references to `DynamicBitmap` block storage and deduplication. Determine which operations share blocks and which copy bytes. Record cancellation boundaries and ownership transitions without exposing vendor code.

Then trace `ViewTile.BeginDraw`, `Draw`, `AbortDraw`, and `ClearGPUAssignment`. Establish the request inputs, dependency invalidation, completion/cancellation contract, and hardware-buffer handoff. Start from source-bound slices, not another whole-library import.

The next round must answer whether a document edit or history seek can leave an obsolete tile eligible for publication. Static evidence must distinguish a dirty-region request from a full-resolution export. GPU residency claims require device/buffer transfer evidence, not method names.

### Subsequent probes

Trace `DocumentController.GetDocument` and `GetCurrentPhysicalRootNode`, then a simple layer-property command. Establish node identity, relationship roles, staged changes, and cycle rejection. Do not infer a model from UI terminology alone.

Trace `SaveDocumentCommand.Do` and the called archive boundary. Establish save-with-history selection, serializer version gates, destination replacement, and recovery after a failed write. Use original synthetic fixtures for later interoperability or runtime checks.

Trace one editable mask/filter node through the discovered renderer/raster interfaces. Establish coverage scope, ordered inputs, color/precision policy, and cache identity. Keep render-resolution claims unresolved until output evidence supports them.

Trace one DirectX texture upload/display path and one raster hardware-device operation. Map ownership, synchronization, upload/readback, and CPU fallback. Keep Wine backend attribution separate from shipped import evidence.
