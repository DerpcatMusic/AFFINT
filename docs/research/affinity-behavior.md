# Unified Affinity behavior research for AFFINT

Status: first public-source report, 2026-10-12. Clean-room research only. No proprietary implementation/resource inspection, application launch, build, test, or benchmark performed by this worker. This report is not a claim of verified installed-version parity.

## Evidence rules and important corrections

- **Current documented** means the current unified Affinity site explicitly describes the feature. This is not a black-box reproduction.
- **V2 documented** means official Photo/Designer/Publisher 2 help supplies detailed behavior. It is a useful prior, not proof that the installed unified app has identical behavior.
- **Observed** requires an original synthetic fixture, action trace, installed version, and result. There are no such observations in this report yet.
- **AFFINT requirement** is our chosen behavior. It must not be presented as an Affinity fact.
- **Unknown** means the evidence does not establish the behavior. Missing marketing text is not proof of absence.

The current public site differs substantially from old Affinity feature-gap lists. Its 15 September 2026 first-party update explicitly announces **Blend Tool, in-app scripting across vector/pixel/layout, a new Mask Tool, curvature-comb/G2 tools, native INDD import, live booleans by default, and custom Preflight rules**. The current vector product page also explicitly lists **mesh gradients**, symbols and constraints. Do not put those features into an “Affinity lacks” column. The installed app's build and entitlement still need proof. The current layout marketing FAQ mentions EPUB export. Do not promote any format claim to demonstrated format fidelity.

Public evidence is preserved in `affinity-evidence/public/`. `affinity-evidence/retrievals.jsonl` records URL, UTC retrieval time, content hash, status, and redirects. Current help/search/release-note pages return a document shell to the non-JavaScript collector, not the actual knowledge-base article. This limitation is recorded rather than filled with guesses. The initial generic user agent returned an “unsupported browser” page; a normal Chromium user agent returned the public product pages. No account/login requests were made.

## Source register

| ID | Primary URL | Local text snapshot | Authority / scope |
|---|---|---|---|
| U1 | https://www.affinity.studio/ | `affinity-browser-0.text.txt` | Current public product statement: one `.af` file, Vector/Pixel/Layout, saved undo history |
| U2 | https://www.affinity.studio/photo-editing-software | `current-topics-2.text.txt` | Current photo feature/FAQ statements, including explicit PSD caveat |
| U3 | https://www.affinity.studio/page-layout-software | `current-topics-3.text.txt` | Current layout feature/FAQ statements |
| U4 | https://www.affinity.studio/blog/affinity-update-september-2026 | `current-feature-blog-1.text.txt` | First-party release overview, published 15 September 2026 |
| U5 | https://www.affinity.studio/blog/blend-tool-mask-tool-3d-model-support | `current-feature-blog-0.text.txt` | First-party Blend/Mask/G2/3D overview, same date |
| U6 | https://www.affinity.studio/graphic-design-software | `current-feature-blog-2.text.txt` | Current vector feature claims |
| U7 | https://www.affinity.studio/features/raw-photo-editing | `current-more-2.text.txt` | Current same-document non-destructive RAW workflow and macro/batch statements |
| U8 | https://www.affinity.studio/blog/raw-development-mixed-light-motion-blur-reduction | `current-more-0.text.txt` | First-party RAW update / presets / nine adjustments; AI entitlement caveat |
| U9 | https://www.affinity.studio/blog/affinity-automation-scripting-claude | `current-more-1.text.txt` | First-party automation update |
| H1 | https://affinity.help/photo2/en-US.lproj/pages/Layers/livefilters.html | `v2-layer-semantics-2.text.txt` | V2 live-filter scope, edit, move, mask behavior |
| H2 | https://affinity.help/photo2/en-US.lproj/pages/Layers/adjustmentLayers.html | `v2-layer-semantics-8.text.txt` | V2 adjustment scope, mask, placement behavior |
| H3 | https://affinity.help/photo2/en-US.lproj/pages/LayerOperations/clipping.html | `v2-layer-semantics-5.text.txt` | V2 parent/child clipping and Lock Children |
| H4 | https://affinity.help/photo2/en-US.lproj/pages/Layers/layerDropZones.html | `v2-layer-semantics-6.text.txt` | V2 row-name / thumbnail / insertion-line distinction |
| H5 | https://affinity.help/photo2/en-US.lproj/pages/Layers/LayerMasks.html | `v2-layer-semantics-1.text.txt` | V2 raster/vector masks |
| H6 | https://affinity.help/photo2/en-US.lproj/pages/Layers/compoundMasks.html | `v2-layer-semantics-3.text.txt` | V2 editable mask booleans |
| H7 | https://affinity.help/photo2/en-US.lproj/pages/LiveMasks/liveLayerMasks.html | `v2-layer-semantics-4.text.txt` | V2 hue/luminosity/frequency masks |
| H8 | https://affinity.help/photo2/en-US.lproj/pages/SizeTransform/cropping.html | `v2-edit-semantics-0.text.txt` | V2 canvas crop/uncrop/straighten; separate resample mode |
| H9 | https://affinity.help/photo2/en-US.lproj/pages/Raw/raw.html | `v2-edit-semantics-2.text.txt` | V2 embedded/linked RAW and engine/output restrictions |
| H10 | https://affinity.help/photo2/en-US.lproj/pages/Panels/historyPanel.html | `v2-edit-semantics-3.text.txt` | V2 persistent history and alternate futures |
| H11 | https://affinity.help/designer2/en-US.lproj/pages/ObjectControl/compound.html | `v2-vector-layout-0.text.txt` | V2 editable boolean operands and release |
| H12 | https://affinity.help/designer2/en-US.lproj/pages/Tools/tools_crop.html | `v2-vector-layout-1.text.txt` | V2 object-level Vector Crop Tool |
| H13 | https://affinity.help/publisher2/en-US.lproj/pages/Media/pictureFrames.html | `v2-vector-layout-2.text.txt` | V2 frame versus contained-image controls |
| H14 | https://affinity.help/publisher2/en-US.lproj/pages/Pages/masterPages.html | `v2-vector-layout-3.text.txt` | V2 shared page elements |
| H15 | https://affinity.help/photo2/en-US.lproj/pages/Filters/filters_applying.html | `v2-edit-semantics-5.text.txt` | V2 distinction between destructive and live filters |
| H16 | https://affinity.help/photo2/en-US.lproj/pages/Sharing/export.html | `v2-edit-semantics-4.text.txt` | V2 export area/settings |
| H17 | https://affinity.help/publisher2/en-US.lproj/pages/Publishing/export.html | `v2-vector-layout-4.text.txt` | V2 page/spread export |

All snapshot names in this table are relative to `docs/research/affinity-evidence/public/`.

## Feature and uncertainty matrix

| Workflow / feature | What Affinity already documents | Reference-app addition or distinction | AFFINT action / uncertainty |
|---|---|---|---|
| Shared vector/pixel/layout document | U1: all three studios edit the same `.af` file. U3: edit images and graphics inside layout; customize studios with tools from other disciplines. | Separate Adobe application workflows are not automatically superior. | First milestone must retain one document/revision across mode switches; UI tools are views onto that model, not different formats. |
| Heterogeneous layer tree | U2: raster, vector, text, groups, clipping masks, linked layers. H3: pixel and vector can be parent or child, and child can be edited independently. | Photoshop clipping semantics need separate Adobe primary verification; do not equate the term “clipping mask” across apps. | Represent containment, clipping, mask attachment, and effect scope explicitly. A generic `parent_id` alone can lose semantics. |
| Layer drop/reorder | H4: between rows changes stacking; row name makes child; thumbnail attaches mask; parent bars permit exit to ancestors. H3: Move Inside/Outside. | No verified competitor gap yet. | Show destination relation before drop. Commit reparent+order+transform compensation atomically. Reject cycles. Current hit zones remain unobserved. |
| Masking | U2: channel/luminosity/depth masks, refined selection workspace. H5–H7: raster/vector/compound and live hue/luminosity/frequency masks. U5: dedicated Mask Tool with editable gradients and detection workflows. | Adobe Select and Mask / reusable smart-object masking need direct docs and fixture comparison, not feature-name counts. | Start editable coverage masks, then edge-refinement workflow. Do not claim Affinity lacks frequency/compound/live masks. |
| Live filters / adjustments | U2 explicitly supports non-destructive adjustment and live-filter layers. H1/H2: edit later, reorder, scope to stack/layer/group, mask effect. | Photoshop smart filters may add packaging/interchange semantics, not proof of missing Affinity live effects. | Separate effect inputs from scoped compositing result. Moving the same filter must change scope without baking source. |
| Boolean geometry | H11: compounds retain editable operands, individual modes, later insert/remove/release. U4: booleans now live/editable by default. | Illustrator compound shapes and Pathfinder workflow need direct docs. | Keep operand identities/transforms and operation order. Explicit flatten/expand is a separate operation, never implicit. |
| Crop / straighten | U2 lists non-destructive crop. H8 hides regions rather than deleting them; Unclip Canvas restores bounds; Resample is a distinct mode. H12 crops objects/groups/images non-destructively. | Photoshop's Delete Cropped Pixels toggle is a candidate interaction comparison, not an established Affinity gap. | Distinguish page/canvas bounds, image aperture/frame, and intentional resample. Crop must not replace original asset bytes. |
| RAW | U2: non-destructive RAW development. H9: original RAW retained as embedded or linked RAW layer; Pixel output is destructive; Serif Labs engine required for V2 non-destructive path. U4: deeper live RAW control/presets. | Camera Raw workflow/interoperability requires Adobe docs and camera-specific oracle evidence. | Later embedded-original RAW asset + versioned development recipe. Do not promise universal camera support or exact vendor matching. |
| Undo / history | U1/U2: history can be saved. H10: timeline seek, persistent history option, **Cycle future** retains alternate redo branches. | No demonstrated gap in Affinity branch history. | Single-edit undo/redo first; branch identity must be correct now. Optional retained alternate futures is a later UI feature, not a reason to reuse IDs. |
| Layout | U3: shared styles, Smart Master Pages, text flow, variable/OpenType typography, Data Merge. H13: max-fit/min-fit/stretch/manual fitting, one specially designated content child among other frame children. H14: shared page elements. U4: custom Preflight rules. | Advanced InDesign fidelity needs a separate import corpus; tables/anchored/GREP limitations in U3 are qualified import statements, not absence of native layout tools. | One-page framed photo/vector/title composition first. Multi-page flow, masters, preflight and data merge later. |
| Blend / smooth curves | U5: live editable blends, true-distance curve distribution, bias/easing, mapping, two-path control; curvature comb/snapping. | Illustrator blend is not a safe “missing from Affinity” claim anymore. | Later workflow backlog; validate sampling/arc-length semantics and editability with synthetic curves. |
| Scripting | U4 explicitly: in-app scripting across vector, pixel, layout. | Adobe automation not a proven unique addition. | Later transaction-based automation; no direct mutable graph escape hatch. |
| Import/export | U1 markets broad preservation. U2 more specifically warns PSD smart objects/advanced filters may flatten. U3 warns IDML advanced formatting may need correction; says no IDML/INDD export. U4 announces native INDD import. | Broad “all files compatible” statements are not oracle evidence. | Native save first; explicit preservation/refusal policy for unsupported data. Export full requested dimensions with clear rasterization/color/font policy. |
| Performance | U1/U2 claim speed/GPU behavior without a disclosed comparable benchmark. | None establishes world leadership. | Treat as marketing only. Measure representative documents, fidelity, p50/p95, peak memory and transfer traffic before comparative claims. |

## Photoshop / Illustrator comparison frontier

The Adobe primary-doc collection returned **HTTP 403**, including these exact official pages:

- `https://helpx.adobe.com/photoshop/using/nondestructive-editing.html`
- `https://helpx.adobe.com/photoshop/using/create-smart-objects.html`
- `https://helpx.adobe.com/photoshop/using/crop-straighten-photos.html`
- `https://helpx.adobe.com/photoshop/using/select-mask.html`
- `https://helpx.adobe.com/illustrator/using/combining-objects.html`
- `https://helpx.adobe.com/illustrator/using/blending-objects.html`

The retrieval ledger records failures; no article content was silently substituted from memory or a secondary post. Consequently, this report does **not** establish a current Adobe-exclusive feature. These are actionable comparison candidates, not verified Adobe/Affinity differences:

| Candidate reference workflow | Next primary/black-box proof | What AFFINT can improve independently |
|---|---|---|
| Photoshop packaged reusable source + editable filter/mask stack | Verify current Smart Object/filter-mask behavior, then compare Affinity's embedded/linked document/image and live-effect paths using the same original fixture. | Clear full-source provenance, effect scope, replace-source contract and unchanged original bytes. |
| Photoshop fine-edge selection/mask refinement | Verify current Select and Mask controls, output options and color-decontamination editability. Compare Affinity's documented refine workspace/new Mask Tool, rather than assuming a gap. | Fast original-fixture edge cleanup, visible mask preview and recoverable prior coverage. |
| Photoshop crop/history workflow | Verify pixel-deletion toggle, persistent history limits and branch behavior. Affinity already documents uncrop and Cycle future. | Reversible aperture by default; explicit resample; atomic gestures and identity-safe history. |
| Illustrator Pathfinder / live compounds | Verify operand retention, expand/release, stacking/style owner and multi-operand semantics. Compare current Affinity live-by-default booleans, not only V2 shortcuts. | Stable operands, declared tolerances and a truthful flatten/expand boundary. |
| Illustrator Blend / Appearance / pattern authoring | Verify editable endpoints, path spacing and multi-fill/stroke/pattern controls against current docs or same-fixture UI. Affinity already documents Blend Tool, mesh gradients and symbols. | Complete useful vector workflows with parameters that remain editable and output that scales faithfully. |

Reference open-source implementations add **inspectable engineering evidence**: V's curve-preserving boolean adapters; D's threaded text/overset composition; R/L's staged develop pipelines; E's dependency-aware caches; G's resource/history representation. `reuse-candidates.md` distinguishes actual audited source from performance or product-parity claims. Those capabilities are not evidence that Affinity lacks them.

## Implementor-facing behavior requirements

These are **AFFINT requirements**, informed by the sources above, not undocumented Affinity claims.

### B1 — One document, multiple workspaces

1. Opening or creating a document produces one document identity and one revision sequence.
2. Vector, Pixel and Layout tool workspaces do not copy, flatten, export, or reinterpret the document on switch.
3. Selection and current page survive a workspace switch where their identities remain valid.
4. Each enabled tool commits real behavior through the document transaction interface. Unsupported actions are absent or truthfully unavailable.

### B2 — Relationships and layer moves

1. A relationship specifies its semantic role: ordered composition child, clipped content, mask, or scoped operation. The implementation can choose another internal shape if it preserves these distinctions.
2. A move request specifies destination, relationship role, insertion position, and coordinate policy.
3. A drag/drop preview displays that destination before release. Invalid destinations are not offered.
4. Reparenting is one edit. It preserves stable layer/asset identity. On failure, document and history are unchanged.
5. Default AFFINT reparent policy: preserve world-space appearance by compensating the local transform. Verify the installed Affinity behavior separately before calling this parity.
6. A parent transform affects contained children. An explicit Lock Children mode compensates child transforms when resizing the parent. The whole gesture remains one edit.
7. Reject self-parenting, descendant-parenting, duplicate ownership and invalid mask/effect relationships before publication.
8. A picture frame is not merely a group with a rectangle. H13 documents one designated framed-content child plus arbitrary regular children, with different resize behavior. AFFINT must model this designation if it exposes picture-frame tools. Replacing designated content is one atomic edit; undo restores the previous content/source identity. Do not copy an ambiguous row-name drop policy until its replacement versus add-child consequence is shown clearly.
9. A frame's fit mode is explicit: cover/max-fit, contain/min-fit, stretch, or independent/manual. Manual image manipulation changes the mode explicitly; frame resizing must not unexpectedly resume automatic fitting. These names summarize H13's V2 behavior; current unified controls remain unobserved.

### B3 — Masks, live operations and compositing scope

1. Original assets are immutable. A mask changes coverage, not source pixels.
2. Mask transform and owner transform are distinct. Expose whether they are linked; do not silently assume a link after reparenting.
3. An adjustment/filter retains type, versioned parameters, coverage and source scope. Reopening it edits that operation.
4. Operation order is observable. Reordering two non-commuting operations must recompute the result.
5. Attaching an operation to one layer/group limits scope; a stack operation applies to the declared below-stack input. Do not use one cached “whole-document bitmap” for both.
6. Cache identity includes upstream asset/content revisions, relationship scope, ordered inputs, parameters, masks/transforms, precision/color policy and output geometry. Renderer developers choose exact key representation.
7. Removing, hiding or moving an operation restores/recomputes output from retained source, not an inverse approximation.
8. Pixel brushes/retouching are not automatically reversible live recipes. Until implemented safely, offer a separate editable paint/retouch layer and retain original source; do not advertise every vendor destructive action as live.
9. H5 documents both attached masks and independent masks that affect the below-stack scope. These are not interchangeable. AFFINT may initially expose attached masks only, but its domain model must not mislabel a generic composition child as a mask or imply that a mask can only affect one layer.
10. Coverage gradients and mask modifiers retain their parameters. A later edge-refinement operation must specify whether it modifies editable coverage or creates a baked replacement; retain recoverable prior coverage under either policy.

### B4 — Live booleans and crops

1. Boolean operands remain addressable and editable after the operation. Releasing a boolean restores its operands with identities and transforms.
2. Validate degenerate geometry and define operand order. Use an explicit color/style owner policy; H11 describes lowest-object style but AFFINT may choose a documented alternative.
3. Page/canvas crop changes the output aperture. Object crop changes that object's aperture. Image frame fitting changes view onto an image. None replaces original source pixels.
4. Straightening retains original asset and affine transform. Intentional resampling is a separate named operation with filter/dimension/rounding policy.
5. Removing crop reveals original extents, subject only to other explicitly retained masks/operations.

### B5 — Edit, history, save and exact export

1. A continuous move/resize/slider gesture previews transient state, then commits one undoable edit. Cancel commits nothing.
2. Undo/redo preserves layer/asset identity and content. A new edit after undo never reuses identifiers issued on the abandoned future.
3. Native save atomically replaces the destination only after complete serialization/validation. Failure preserves the original and reports the error.
4. Native reload restores operation parameters, nesting, source assets and relevant coordinate/color policies, not a flattened preview.
5. Export binds to a committed revision and requested dimensions. Later edits do not change an already-bound export silently.
6. Display mipmaps/proxies must not become exact export inputs. Document all intentional resampling and quantization.
7. Unsupported imported content is retained opaquely, refused, or subject to explicit restricted-save rules. Do not silently save a damaged subset.

## Recommended first end-to-end workflow

**Original synthetic one-page editorial poster.** This demonstrates all three disciplines without starting with advanced RAW or book publishing.

1. Create a page with explicit pixel dimensions/color policy.
2. Import a generated asymmetric checker/ramp image as an immutable original asset. Preserve source dimensions/hash.
3. Place it inside a vector shape or image aperture. Move the image independently inside the frame; resize parent with and without Lock Children.
4. Add one genuinely implemented live adjustment and a coverage mask. Edit parameters after applying.
5. Add two vector shapes as a live boolean and a title/text element. If shaped text is not implemented yet, defer the title visibly; do not use a fake text control.
6. Reorder and reparent image/vector/effect elements. Verify scope change and stable identities.
7. Undo/redo every meaningful gesture, then branch history after an undo.
8. Save/reopen editable native document; restore crop and change effect/boolean operands.
9. Export at both original page dimensions and a larger requested size; compare with the declared CPU reference and confirm no viewport proxy was substituted.

**Coordinator proof requests:** test transactional cycle rejection with unchanged history; cancellation; immutable asset hash before/after edits; identity preservation through undo/branch; cached/uncached agreement after scope/reorder/mask changes; native save failure preserving prior file; exact export dimensions and reference tolerances. This worker has not run any of those tests.

## Black-box observation queue for the coordinator

Only the coordinator launches the blank app. Use original synthetic documents; do not capture home/account/recent files or open personal documents. Persist observations in `.research/observations` as requested, then provide their paths to this worker for a behavior-only interpretation.

Original optional fixtures are now available in `docs/research/affinity-evidence/fixtures/`: `original-asymmetric-ramp.png`, `coverage-ramp.png`, `alpha-coverage-ramp.png`, and `original-vector-operands.svg`. `manifest.json` records dimensions, hashes and origin; `make_synthetic_fixtures.py` generates them without external libraries or vendor assets. The main PNG has one-pixel bars, asymmetric corners, opaque color ramps and populated RGB under an alpha ramp. The two mask PNGs deliberately distinguish **opaque grayscale** from **white with ramped alpha**; luminance-to-mask conversion must be explicit and observed. These distinguish orientation, crop, sampling and premultiplication mistakes. The fixtures are **inputs**, not render goldens or evidence that any application passed a test.

- O1: Record installed app version safely, document title, page geometry and working color format. The current site may be newer than the installation.
- O2: One image + rectangle + ellipse + paint layer. Drag image to row name versus thumbnail versus insertion line; record parent/role/bounds before and after. Undo once per gesture.
- O3: Create one scoped adjustment and live blur. Move to stack top, into group, onto image; record which underlying elements change. Move two non-commuting operations past each other.
- O4: Raster mask + vector mask + gradient; move owner and mask separately, toggle transform link if offered. Preserve before/after synthetic renders.
- O5: Create live add/subtract boolean, edit one operand, release. Determine current default live/flattened mode rather than assuming V2 shortcut behavior.
- O6: Crop/uncrop canvas; object crop; parent frame fitting; straighten. Reopen native save and reveal hidden original regions.
- O7: Undo move, create new edit, inspect Cycle future behavior if present; save with and without history.
- O8: Export two sizes from same committed content; include fine lines, alpha ramps, boundaries and text. Record color/profile/resampling settings. Do not infer full-resolution internals merely from an output dimension field.
- O9: Later: own/licensed RAW fixture with embedded original, redevelop after adding layers, reopen. Record supported output modes and linked-asset missing-file behavior.

## Later backlog by useful completed workflow

1. **Reliable compositing:** selections and edge refinement, editable gradients/compound masks, blend ranges, linked mask transforms, multiple scoped live effects.
2. **Vector editing:** pen/node tools, robust booleans, stroke/fill geometry, live warp and reusable symbols; later blends/G2/arc-length distribution.
3. **Photography:** audited RAW decoding/color/lens recipe, before/after comparison, re-development in layout, preset application, non-destructive retouch layers.
4. **Layout:** shaped text, image frames, linked text flow, styles, multiple pages/master overrides, missing-font/asset handling, preflight and print PDF fidelity.
5. **Automation/interchange:** transaction-based scripts/macros, batch/slices export, external PSD/SVG/PDF/IDML/INDD oracle corpus and explicit preservation limits.

Unknowns must be closed by primary documents or controlled observations. No feature absence or benchmark leadership is inferred from this report.
