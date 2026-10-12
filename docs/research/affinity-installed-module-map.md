# Installed Affinity module and research coverage map

Status: whole-install inventory plus bounded deep reads, 2026-10-12. This report
maps the owner's installed unified Affinity 3.2.3.4646 and the actual research
coverage. It does not claim that its entire implementation has been read or that
decompiled code is original source.

## Inventory scope

The complete installation at
`<local Affinity installation>` was
re-enumerated: **1,268 regular files**, **122 MZ/PE images**, with no additions or
missing PE candidates relative to the prior complete map. All 122 images were
rehashed. Existing PE layout/import/export classification is reused from the
source-bound round-01 inventory; it identifies **69 native**, **51 IL-only**, and
**two mixed managed/native** images.

ILSpy independently listed resource names and TypeDef metadata in all **53**
managed/mixed images, including the older UI `.bak` and localized satellites.
Every invocation returned success. The TypeDef tables contain **54,788 rows**,
including module and compiler-generated definitions. This is type inventory, not
54,788 inspected implementations. The resource listing has 327,989 text lines;
embedded names can contain newlines, so that line count is not advertised as an
exact resource-entry count.

The current UI assembly contains **453 compiled BAML entries**; **15** were selected
and converted for actual interface/layout research. No vendor icons or branding
were extracted for AFFINT. The mixed persona bridge's **77,390 MethodDef rows** and
the current UI assembly's method declarations were inventoried for targeted
dispatch/input/startup selection. Most method bodies remain unread.

Raw extracted source, markup, metadata and disassembly remain exclusively below
ignored `.research/vendor-affinity/ui-reference-r1/`. Implementors consume this
report and the independently worded specs, not raw vendor output.

## Principal installed module boundaries

The counts below are exported-name inventory counts from the retained PE maps.
An exported name identifies a research seam; it is not proof that every operation
works or that its algorithm has been inspected. Module descriptions combine prior
source-bound findings and symbol/import classification, with inference called out.

| Module | Installed bytes | Export count | Mapped responsibility / actual read status |
|---|---:|---:|---|
| `Affinity.exe` | 11,446,280 | 0 | Managed application launcher and resources. Startup class read; artwork kernels are elsewhere. |
| `Serif.Affinity.dll` | 15,636,992 | 0 | WPF shell, controls, docking, studio pages, commands and theme. Selected resources/control/startup sections read; most UI implementations unread. |
| `Serif.Windows.dll` | 467,976 | 0 | Managed WPF support/docking types and services. Type/resource inventory; selected templates reside in the main UI assembly. |
| `Serif.Interop.Persona.dll` | 59,277,832 | 0 | Mixed managed/native document/UI bridge. Targeted wrappers and native input/camera dispatch read; most metadata-listed methods unread. |
| `libaffinity.dll` | 24,856,072 | 13,785 | Native application/workspace/interface/controller organization. Pan/zoom and ruler roots read this round; broad persona/workspace declarations inventoried. |
| `libpersona.dll` | 359,297,544 | 50,359 | Native document/tool/view/history and tiled-view behavior. Earlier history/raster/mask rounds plus targeted view/camera/cache/dirty-region reads; most procedures unread. |
| `libcommands.dll` | 5,140,488 | 4,633 | Document command/controller seams inferred from symbols/imports. Inventory is not a whole-command implementation audit. |
| `libkernel.dll` | 7,015,944 | 5,271 | Shared lifetime, locking, collections, reflection and infrastructure. Targeted counted-owner/lock calls reached by earlier and current probes; broad body coverage absent. |
| `libraster.dll` | 71,407,112 | 37,923 | Bitmap/buffer/sampling/color and raster primitives. Earlier bounded buffer/mask work; conversion/filter coverage remains partial. |
| `librastertools.dll` | 163,573,256 | 3,394 | Raster/photo tool operations and reflected tool seams inferred from symbols/imports. Inventory; detailed per-filter kernels remain followup. |
| `librenderer.dll` | 81,864,712 | 5,112 | Renderable/texture/display-rendering infrastructure. Earlier bounded retained-result/lifetime probes; no whole-renderer algorithm coverage. |
| `libgeometry.dll` | 3,988,488 | 4,053 | Geometry/curve primitives inferred from export families. Inventory; per-boolean/offset/stroke kernels not read this round. |
| `libStory.dll` | 18,865,160 | 6,654 | Text/story/layout families inferred from symbols/imports. Inventory, not typography parity evidence. |
| `libbook.dll` | 2,504,200 | 1,196 | Book/page/document organization seams. Inventory; no whole-layout read. |
| `Serif.DirectX.dll` | 1,314,312 | 364 | Windows GPU bridge. Prior targeted texture/upload/lifetime traces; API presence does not establish equivalent GPU processing. |
| `libdxcore.dll` | 50,184 | 45 | DirectX helper boundary inferred from linkage; no broad body read. |
| `libocio.dll` | 4,340,232 | 980 | Color-display conversion boundary inferred from linkage. Inventory; actual color-transform internals not read this round. |
| `libpsd.dll` | 3,132,424 | 985 | PSD interchange seams inferred from linkage. Inventory is not a demonstrated compatibility oracle. |

The retained complete map also covers import/export, metadata, scripting,
networking, text/font/dictionaries, image codecs, PDF/DWG/IDML/EPUB import,
crash/IPC and installed helper dependencies. These modules remain indexed unless
a separate report identifies concrete read coverage. Installation-specific
`AffinityHook.exe` is a third-party launch wrapper: its startup argument forwarding
was read only to support the coordinator's own reference capture. No unrelated
software/configuration was changed.

## Deep-read coverage in this round

Twenty-seven distinct native functions completed Ghidra decompilation across five
sequential source-bound analyses. One additional local comparator has complete
reachable assembly coverage. [The view pipeline spec](affinity-view-pipeline-spec.md)
identifies concrete camera transforms, 100 ms zoom completion, view-center tile
prioritization, residual-scale gating, pixel alignment, dirty intersections and
retained removal collections. Its source slices total 41,973 bytes, plus the
59-byte comparator.

[The UI reference](affinity-ui-reference.md) records dimensions, typography,
decoded theme states, layer-tree geometry, tool flyout interactions, saved UI
preferences and initial root-owned screenshot measurements. Selected managed
classes were reconstructed to reach those seams. Large classes such as workspace
and layer view were read only at specifically relevant sections; decompilation of
a class is not full review of it.

Existing separately scoped reports cover
[history/interface](affinity-binary-spec.md),
[raster/tile ownership](affinity-raster-tile-spec.md),
[display result and mask consumer](affinity-result-mask-spec.md), and
[editable-mask sampling](affinity-editable-mask-spec.md).
Their evidence and unknowns remain separate. This map does not combine successful
tool runs into an unsupported complete-algorithm or application-parity claim.

## Retained inventory and provenance

Records below `.research/vendor-affinity/ui-reference-r1/`:

| Record | Authority |
|---|---|
| `inventory/current-files.json` | Every installed regular path, byte count and MZ candidate rechecked |
| `inventory/whole-install-pe-map.json` | SHA-256 for all 122 installed PE images; prior static classifications/layout/linkage bound by path |
| `inventory/all-managed-resource-index.json`, `inventory/all-resources-*.txt` | All 53 managed/mixed resource-name listings, result codes and raw listing coverage |
| `inventory/all-managed-type-index.json`, `inventory/all-types-*.json` | All 53 complete TypeDef tables and row counts |
| `inventory/{Serif.Interop.Persona,Serif.Affinity}.methoddefs.json` | Method declaration inventory, not body coverage |
| `native/*.plan.json`, `native/*/functions.json` | Exactly selected native roots, addresses, source slices, branch coverage and decompilation outcomes |
| `coverage.json`, `provenance.json`, `artifact-digests.json` | Stable round boundaries, read versus inventory limits and hashes |

## Next specific algorithm questions

The strongest open view question is whether obsolete dirty work can ever publish
after a newer camera/document revision, and what releases the retained removed
tiles after GPU submissions. Neither a lock nor an abort call proves that contract.
The direct tile rectangle-invalidating consumer and removal/purge completion path
are narrow followup roots.

Display cache conversion helpers are another concrete next seam: their caller
selects primary/secondary spread caches, dimensions, format/profile carriers and
format-dependent conversion helpers, but those callees' math remains unread.
Editable-mask coordinate/alpha-versus-luminance and byte/float equivalence unknowns
remain in the earlier mask report. Geometry booleans, stroke expansion and photo
filter kernels should each start from actual exported operation/control seams,
not from a giant undifferentiated source dump.

The Vector workspace's exact tool order, context controls and studio association
remain unresolved static descriptor questions. The owner has stopped all new
Affinity window launching/input/capture work; compiled resources and selected UI
preferences are the reference authority. Existing Slice/no-document imagery is
historical corroboration only. AFFINT's own new UI and responsiveness implementation
must still be tested against its independently recorded numerical/runtime contracts.
