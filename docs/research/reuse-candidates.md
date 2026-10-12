# Audited reuse candidates for AFFINT

Status: pinned-source triage, 2026-10-12. **No candidate is approved for production integration yet.** This worker did not compile, run tests, benchmark, install dependencies, execute upstream code, or change user configuration. Findings below are static source observations. Existing upstream tests are evidence of intended contracts, not evidence of passing tests here.

## Snapshot/provenance ledger

Snapshots are under `docs/research/affinity-evidence/opensource/<owner>--<repo>/<full-sha>/`. Each has `snapshot.json` with original repository, full SHA, UTC collection time, file path, byte length and SHA-256. `refs.txt` records `git ls-remote` results. GitHub API rate-limiting affected commit endpoints, so refs plus SHA-addressed codeload supplied pins. Source files were downloaded, never built/executed. Only Rust/shader text, Cargo manifests/locks, README, license and notice files were retained; artwork/binaries were not inspected.

| ID | Repository | Exact current source pin | License evidence | Initial role |
|---|---|---|---|---|
| P | https://github.com/storytold/photocraft | `6de37ce7f063e6814903650a24c8dad4936e8136` | Root `Cargo.toml`: MIT OR Apache-2.0; `LICENSE-MIT`, `LICENSE-APACHE` | Tiled raster, composition/masks, color and error-policy reference; not whole-editor adoption |
| V | https://github.com/storytold/vectorcraft | `540505fcad636b7983504ff7a1368a4a0d792803` | Same dual-license manifest/files | Vector geometry/booleans, rendering/export reference |
| L | https://github.com/storytold/lightcraft | `76b5377868d912db69a777bda253a5b1b96b22bd` | Same dual-license manifest/files | RAW/develop CPU/GPU pipeline and working-memory gate reference |
| D | https://github.com/storytold/designcraft | `65ef027f9fea002ac641aaf53d2929e1a6a811d1` | Same dual-license manifest/files | Typography, flow, frame/layout and PDF reference |
| E | https://github.com/storytold/effectcraft | `67434e90388b6dca4417555346fa6850b6f96033` | Same dual-license manifest/files | Dependency-aware effect caching and compositing reference; avoid temporal project architecture initially |
| C | https://github.com/storytold/craft-libs | `1aa81a773aa9198a0e4c0daeda954d622475af38` | Manifest/license files and README: MIT OR Apache-2.0, required `NOTICE`; assets retain separate licenses | Small docking/printing/UI utilities, not a shared imaging engine |
| R | https://github.com/pch/rawmakase | `0f1bc320207658c9778922a4d6cf87bd174be2b2` | Root and selected crate manifests: MIT; `LICENSE` credits RAWmakase contributors | Develop recipe/stage cache/GPU-residency design reference; native RAW boundary audited separately |
| G | https://github.com/GraphiteEditor/Graphite | `b4d07595ca82181297d770f54db941d338ea3af7` | Root manifest: MIT OR Apache-2.0; both license files | Resource/history/graph-storage and heterogeneous-node editor reference |
| Q | **https://github.com/CyberTimon/RapidRAW** | `2534f67ee25c1cd6c36c8e1641c12f32c3263835` | `LICENSE`: GNU Affero General Public License v3; GitHub repo metadata identifies AGPL-3.0 | Verified requested repo identity; behavior/design reference only; no application code adopted |

**License is file/module-specific, not inferred from GitHub's badge.** The Craft GitHub metadata identifies Apache-2.0 while manifests explicitly allow MIT OR Apache-2.0. Preserve actual license/notice files with any permitted copied substantial code. P/V `NOTICE` explicitly exclude ArtCraft name/wordmark/logos from the open-source license; no branding/artwork reuse is approved. Dependency/model/font/icon/sample-file licenses are not inherited from repository license. AFFINT's original source is now MIT licensed; this does not relicense external code. Do not import AGPL application code before a deliberate compatibility/legal decision.

**Version graph:** P/V/D/E/C state Rust 1.95, L states 1.90, R/G/Q state 1.98 in reviewed manifests. P/V/L/D reference `craft-ui` at `5c874d51eeb3de70e735c8ce87a59a7dedd47675`, not current C HEAD. D references `craft-print` at `d991bf6792f8ade99c51358bf3c1d902c8cd96b5`. R's lock pins `crmne/fastframe` tag v0.1.7 to `9cb28309097ef313ca38570e4d9d53faed92bdd1`. Q's lock pins its `rawler` fork `CyberTimon/RapidRAW-DngLab` to `a32bc1ff1f4f3c56d4d3226082f6eb17414f9159`. The top-level pin does not erase these transitive provenance boundaries.

## Decision summary

1. **Prefer independently maintained foundation crates over copying app adapters.** V/D demonstrate actual integration of `kurbo`, `vello_cpu`, `skrifa`, `harfrust`, `usvg`/`resvg`, `krilla`, and geometry libraries. L demonstrates `moxcms` and modern imaging codecs. These dependency choices are source-verified, not a completed crate audit. The owning crate/UI researcher must verify current versions/licenses/platform behavior before adoption.
2. **Audit P's tiled raster and V's geometry first**, but retain AFFINT's own small transaction/identity/error contract. Whole upstream document or session APIs are not interchangeable with AFFINT's invariants.
3. **Use R/L to guide a later RAW module**, after the first raster/vector/layout workflow works. A recipe-driven RAW module is useful; transplanting a catalog/photo application is not.
4. **Use D's text/frame composition as a later deep reference**, not as permission to fake layout or silently substitute fonts.
5. **Keep E and G as focused references for dependencies/resource retention.** Their temporal/collaborative machinery is expensive when the first workflow does not need it.
6. **Q is currently reference-only.** In addition to AGPL decision, a verified source path permits successful export after RAW decode falls back to an embedded preview. AFFINT must reject that hidden-fidelity policy.
7. **C contains only two shared crates** (`print`, `ui`) at this pin. It is not a universal shared Craft image/document engine. Do not plan against a nonexistent shared renderer.

## Detailed candidate review and exact source references

Source links below refer to the ledger SHA; use `https://github.com/<repo>/blob/<sha>/<path>#L<line>` for immutable browser references. Local source preserves identical paths and per-file hashes.

Direct immutable links for the highest-priority findings:

- [PhotoCraft selection owner](https://github.com/storytold/photocraft/blob/6de37ce7f063e6814903650a24c8dad4936e8136/crates/doc/src/lib.rs#L800) and [history-surface traversal](https://github.com/storytold/photocraft/blob/6de37ce7f063e6814903650a24c8dad4936e8136/crates/ops/src/tiles.rs#L14).
- [PhotoCraft effect key/build/cache](https://github.com/storytold/photocraft/blob/6de37ce7f063e6814903650a24c8dad4936e8136/crates/compose/src/lib.rs#L1470).
- [VectorCraft boolean tolerance](https://github.com/storytold/vectorcraft/blob/540505fcad636b7983504ff7a1368a4a0d792803/crates/pathops/src/boolean.rs#L133).
- [LightCraft byte-weighted work gate](https://github.com/storytold/lightcraft/blob/76b5377868d912db69a777bda253a5b1b96b22bd/crates/engine/src/memory.rs#L104).
- [DesignCraft cache retention/clear](https://github.com/storytold/designcraft/blob/65ef027f9fea002ac641aaf53d2929e1a6a811d1/crates/compose/src/cache.rs#L140).
- [EffectCraft retained-byte cache](https://github.com/storytold/effectcraft/blob/67434e90388b6dca4417555346fa6850b6f96033/crates/render/src/cache.rs#L193).
- [craft-libs atomic docking model](https://github.com/storytold/craft-libs/blob/1aa81a773aa9198a0e4c0daeda954d622475af38/crates/ui/src/docking/model.rs#L302).
- [RAWmakase stage cache](https://github.com/pch/rawmakase/blob/0f1bc320207658c9778922a4d6cf87bd174be2b2/crates/rawmakase-engine/src/develop/stage_cache.rs#L20).
- [Graphite texture-cache budget scope](https://github.com/GraphiteEditor/Graphite/blob/b4d07595ca82181297d770f54db941d338ea3af7/node-graph/libraries/wgpu-executor/src/texture_cache.rs#L66).
- [RapidRAW preview fallback](https://github.com/CyberTimon/RapidRAW/blob/2534f67ee25c1cd6c36c8e1641c12f32c3263835/src-tauri/src/image_loader.rs#L159) and [export call path](https://github.com/CyberTimon/RapidRAW/blob/2534f67ee25c1cd6c36c8e1641c12f32c3263835/src-tauri/src/export_processing.rs#L1754).

### P — PhotoCraft

Useful source: `crates/raster/src/` (surfaces and tiles); `crates/compose/src/lib.rs`; `crates/compose/src/masks.rs`; `crates/cms/src/pipeline.rs`; `crates/doc/src/`; `crates/gpu/src/`.

- `crates/ops/src/lib.rs:1–10,31–50,74–92` stores undo states as `Arc<Document>` snapshots and clears redo on new record. Pixel tiles are intended to be COW-shared. This is a **reference pattern**, not a claim that cloning its whole document is cheap enough for AFFINT; layer/metadata/history costs still need measurement.
- **Verified static accounting gap:** `crates/doc/src/lib.rs:800` owns `selection: Option<Surface>`. `crates/ops/src/tiles.rs:14–27` counts layer surfaces, masks, channels and quick-mask but **does not visit the selection surface**. `crates/ops/src/lib.rs:194–262` bases history byte reporting/trimming on that traversal. Selection-only retained tiles can therefore escape this budget. Document patterns at `crates/doc/src/lib.rs:818` are another owner to audit; do not present a partial pixel counter as total resident memory.
- **Budget is intentionally soft:** `crates/ops/src/lib.rs:212–244` always retains the most recent undo state even when it exceeds the byte budget. AFFINT needs an explicit oversized-edit admission/spill/refusal policy, not a falsely advertised hard cap.
- **Cache positive and limit:** `crates/compose/src/lib.rs:1331–1391` hashes COW tile identity plus format/default pixels, child state, masks and effect settings and pins a Layer clone while addresses are used. `:1401–1543` implements a byte-accounted global effect-map cache, but allows concurrent miss builds and retains the current used map even over budget. Count temporary build copies and pinned source owners, not just map arrays.
- The current source includes default-pixel and fill-cache identity inputs. The earlier work-log stale-cache reproduction had no recovered source pin. **Do not reassert that historical bug unchanged at this new pin.** Re-run its minimal reproducer only through the coordinator, and distinguish which defect was actually covered.
- **Unclosed cache dependency question:** `effect_maps` at `crates/compose/src/lib.rs:1470–1485` keys layer identity, region and global light, while map building consumes `cx.patterns` through `texture_ctx`. A pattern collection/source change under an unchanged pattern reference needs a cached-versus-uncached proof. This is a specific audit/proof request, not an executed failure report.

Integration cost: raster/doc/color/compose coupling; large feature surface; masks/effects semantics skew toward PSD. Affinity-named import crates are **excluded from this clean-room behavioral/reuse review** pending a separate approved public-file-spec/provenance assessment. Their presence in the public snapshot is not approval to copy them or inspect proprietary files.

Next proof: selection-only history allocations; mutate default pixel, fill pixels, pattern resource, mask properties, group child scope and working precision; compare cached/uncached; exercise tile sharing with source retained across history and async renders; record actual peak memory.

### V — VectorCraft

Useful source: `crates/pathops/src/boolean.rs`, `crates/geom/src/`, `crates/render/src/`, `crates/text/src/`, `crates/svg/`, `crates/pdf/`.

- `crates/pathops/src/boolean.rs:1–18` uses `linesweeper` topology and refits split curves; default refit precision is 0.01 document points. `:133–146` rejects non-finite coordinates and chooses scale-sensitive tolerance. This is real geometry code, not only menus, but **it is approximate/refitted output**, not an exact symbolic Boolean guarantee.
- `crates/render/src/lib.rs:1–7` integrates `vello_cpu` with view transform, appearances, clips, images and glyph outlines. `:48–71` explicitly validates raster side/pixel limits (32768 per side, 2^28 total pixels). That visible refusal is better than hidden export downsampling; AFFINT should use tiling or truthful refusal at its own measured limit.
- Root manifest pins linesweeper 0.5, kurbo 0.13, vello_cpu 0.2 with f32 pipeline, skrifa 0.47, harfrust 0.13, usvg 0.48, krilla 0.8. Lockfiles provide exact resolved versions, but dependencies remain separately unaudited.

Integration cost: upstream `PathData`/anchor model and style/document representation; boolean result is geometry, whereas a live boolean must retain operands in AFFINT. Renderer path limits and error/string adapters must be adapted to structured errors and byte admission.

Next proof: coincident/tangent/self-intersecting curves, opposite winding/fill rules, huge/tiny coordinates, operand order, output bounds; compare geometry/rendering to external references. Geometry tolerances must be specified in document/output units.

### L — LightCraft

Useful source: `crates/pipeline/src/lib.rs`, `crates/pipeline/src/{geometry,masks,output}.rs`, `crates/raw/src/`, `crates/gpu/src/render.rs`, `crates/engine/src/{memory,export}.rs`.

- `crates/pipeline/src/lib.rs:1–26` explicitly declares scene-linear Rec.2020 input, geometry first, scene operations, tone mapping, color, display transfer. It supports full source or proxy input and output-size-relative spatial parameters. This is a documented source contract, **not proof that every kernel is resolution-faithful**.
- `crates/engine/src/export.rs:1–6` separates encoding bytes from filesystem writes; `:30–35` distinguishes original+XMP from raw DNG edits. Useful deep seam: rendering/encoding is not atomic filesystem persistence.
- `crates/engine/src/memory.rs:1–5` correctly says cache bookkeeping is not process RSS. `:12–20,104–147` uses a byte-weighted work gate and derived budgets, **but allows one oversized job and urgent interactive acquisition without waiting**, plus extra export concurrency. It is a throttling pattern, not a hard global memory guarantee.

Integration cost: photo-oriented develop settings and camera/lens metadata, several coupled internal crates, shader/CPU consistency, changing wgpu/egui versions. AI weights/lens databases/camera data need separate license/download audit; no automatic background installation is authorized.

Next proof: color/alpha/precision contract at full resolution; full-source versus proxy cannot silently share exact-export path; GPU equivalence with declared tolerances; malformed raw/metadata and missing lens profiles; global in-flight/cached/history memory admission.

### D — DesignCraft

Useful source: `crates/compose/src/{lib,shape,breaker,cache}.rs`, `crates/fonts/src/`, `crates/doc/src/`, `crates/pdf/src/`, `crates/render/src/`.

- `crates/compose/src/lib.rs:1–8` specifies styled shaping, threaded frames, Knuth–Plass paragraph or greedy line breaking, columns/baselines/wrap and overset detection. This is real layout code. Font substitution is surfaced in `RunStyle.missing_font` at `:60–62` rather than proving faithful output automatically.
- `crates/compose/src/cache.rs:1–6,18–33` retains story/styles/frame inputs to make pointer identity safe and caches previous composition for changed-frame damage. Useful dependency/pinning pattern.
- **Static memory-policy weakness:** `crates/compose/src/cache.rs:140–151` prunes by entry count/recent stamp, not retained bytes; `:211–215` clears main map/running/xrefs but does not clear `previous`. Large shaped stories/glyph output and previous compositions require retained-byte accounting/lifecycle audit before reuse. This is a source observation, not a measured leak.

Integration cost: text story/style/frame model, font scopes, layout-only `Document`, multiple ownership/coordinate spaces. Do not let AFFINT's first document seam become the union of every upstream field.

Next proof: multilingual/bidi/variable-font shaping, missing-font policy, overset and frame wrap; style/master/cross-reference invalidation; large-story memory accounting; PDF text/font/color external oracle, not only self-round-trip.

### E — EffectCraft

Useful source: `crates/render/src/cache.rs`, `crates/render/src/cache/dependencies.rs`, `crates/render/src/masks.rs`, `crates/gpu/src/`, `crates/effects/src/`.

- `crates/render/src/cache.rs:1–17,484–542` explicitly treats referenced-layer transforms, evaluated properties, source dependencies and render options as semantic cache inputs. It has separate layer/input/footprint keys. This is useful for AFFINT's scoped non-commuting live operations, despite temporal inputs being unnecessary initially.
- `:193–216` rejects single entries above one-quarter cache budget and evicts LRU by recorded pixel bytes. Better bounded entry policy than P's over-budget retained current map, but does not itself account for every render intermediate, active external `Arc`, GPU allocation or pinned source.
- Cached-key comments assert no stale pixels. That assertion is **not** a completed audit of every parameter path. The coordinator must verify dependency changes with fixtures before borrowing it.

Integration cost: animation-time/expression/project coupling and many crates. Prefer narrow algorithm/pattern references; avoid bringing a timeline engine into a static editor solely to gain effect names.

Next proof: masks/effects/source/selected-layer dependency invalidation, stale worker cancellation, cache versus uncached, errors through disk-cache/export, input/render precision at exact dimensions.

### C — craft-libs

Useful source: `crates/ui/src/docking/model.rs`, `crates/ui/tests/docking_workflows.rs`, `crates/ui/src/{tabs,layout,theme}/`, `crates/print/src/lib.rs`.

- Root workspace and README list only `craft-print` and `craft-ui`. The crate's rendering/widgets are egui-oriented. However, the reviewed docking model uses generic panel identity and plain numeric geometry (`[x, y, width, height]`), not egui geometry types. This makes the **model contract** a better portability candidate than adopting the whole UI crate.
- `crates/ui/src/docking/model.rs:302–380` stages restoration in a cloned proposed layout, validates it, and publishes only on success. `apply_with_permissions` begins at `:385`. This is a good small-interface transaction reference.
- Printing submits an already-created PDF; PDF generation stays in the app. The UI crate does not solve canvas rendering, document transactions or export fidelity.

Integration cost: whole-crate egui dependency versus AFFINT's unresolved native UI choice. Evaluate extracting/adapting the small toolkit-independent model contract if useful; don't select egui accidentally via a shared helper. Preserve NOTICE, which records exact upstream derivation pins, and audit attribution separately; no vendor icons/artwork are approved.

Next proof: malformed layouts, duplicate panels, invalid geometry, cancelled move, missing restore anchors, window/dock persistence. The owning UI researcher decides adoption.

### R — RAWmakase

Useful source: `crates/rawmakase-engine/src/develop/stage_cache.rs`, `crates/rawmakase-engine/src/develop/gpu/{resident,develop,present}.rs`, `crates/rawmakase-model/`, `crates/rawmakase-native/{Cargo.toml,build.rs}`.

- `stage_cache.rs:1–8,113–135` uses stage-specific recipe keys and exhaustive field destructuring so adding a recipe field forces consideration. Useful cache-maintenance technique, though a field can still be assigned to the wrong stage.
- `:20–37,49–96` keeps four entries and 512 MiB **per stage**, computes large results without keeping them, and returns shared `Arc` outputs. Seven stage LRUs can collectively retain much more than 512 MiB plus rasters/in-flight data. Treat per-stage retention as one account inside a process budget.
- `gpu/resident.rs:277–308,565–636` checks hardware buffer limits and reuses photo/sample buffers rather than uploading indiscriminately. It still needs actual upload/readback/peak-GPU measurements and cancellation/fidelity proof.
- `crates/rawmakase-engine/Cargo.toml` explicitly separates the CPU/GPU develop engine from native LibRaw/catalog/GUI. `crates/rawmakase-native/Cargo.toml:7–12` is a **C++ bridge to LibRaw and Little CMS** using `cc` and `pkg-config`, not pure Rust. Root MIT does not license all linked native libraries. Native source/provenance/license/version audit is still required; no build scripts were run.

Integration cost: photo-specific model/interop types; independently evaluated native FFIs/dependencies; MSRV 1.98/wgpu 30; allocation and panic/error policy differs from AFFINT. Do not inherit telemetry/updater/catalog functions for a rendering module.

Next proof: exact-resolution CPU/export versus GPU preview; camera color/black-level/orientation/demosaic policies; max buffer overflow path; stage-key perturbation fixtures; byte budget that includes source cache, CPU/GPU stages and queued jobs; native dependency license audit.

### G — Graphite

Useful source: `document/graph-storage/src/{session,history,resources,ids}.rs`, `document/container/src/`, `document/format/src/`, `node-graph/graph-craft/src/document.rs`, `node-graph/libraries/wgpu-executor/src/texture_cache.rs`.

- Current root manifest includes real desktop, document storage/container, graph executor and rendering crates; do not classify the current repo merely as a web-only vector toy based on old descriptions.
- `document/graph-storage/src/session.rs:98–103` stages content-addressed embedded resources independently of bytes; caller persists resource content. `history.rs:1–25` owns append-only delta DAG and indexes revisions/resource hashes. Useful resource retention/history direction, but not a small ready-made single-user transaction/history layer.
- Session/storage includes collaboration/hot-tail/retirement/serialization machinery and runtime conversion. Adopting it wholesale would increase interface/concept count before AFFINT has a useful first workflow. The `default = ["conversion"]` feature in `document/graph-storage/Cargo.toml` adds runtime graph dependencies; it is not standalone by default.
- `node-graph/libraries/wgpu-executor/src/texture_cache.rs:6–17,66–92` budgets **free cached textures**, not all active GPU memory. Borrowing its pooling strategy does not establish a global GPU cap. It also contains unchecked/panicking assumptions (`:27`) that conflict with AFFINT's production contract and require adaptation, not blind copy.

Integration cost: graph/dynamic-node/CRDT schema, runtime conversion, execution/thread/async semantics and evolving desktop/frontend. Favor concepts or narrowly audited types over cloning the full app. No historical bezier-rs path is proposed without finding that module at this actual pin.

Next proof: no ID reuse after branch; transaction failure isolation; external resource retention; malformed storage/replay and atomic save; dependency graph cycle behavior; GPU resource lifetime/active+free budget and device loss.

### Q — RapidRAW (verified repo)

Useful source for reference: `src-tauri/src/{raw_processing,image_loader,gpu_processing,export_processing,mask_generation,cache_utils}.rs`, shaders and locked `rawler` fork.

- Identity is **CyberTimon/RapidRAW**, verified by GitHub repository search metadata, root README, pinned ref and source archive. Do not confuse its separate AI connector/docs/raw-loader repositories with the editor.
- Root app is Tauri/Rust plus web frontend. `src-tauri/Cargo.toml` includes wgpu 29 and the separate rawler fork; it is not a pure native Rust UI implementation to transplant.
- **Verified static preservation/fidelity hazard:** `src-tauri/src/image_loader.rs:69–79` routes `load_and_composite` through `load_base_image_from_bytes`. `:159–194` catches RAW develop error/panic and returns `Ok(linearize_embedded_preview(preview))`, with log warning but no typed degraded-result marker. `src-tauri/src/export_processing.rs:1754–1794` uses the currently loaded original image or this same loader with `use_fast_raw_dev = false`. Thus a supported export path can consume an embedded preview after failed RAW decode rather than returning a faithful-source error. This is a complete static call-path finding; the worker has not run a corrupt/unsupported RAW reproducer.
- AFFINT policy must distinguish `FullSource` from `PreviewOnly` at the type/interface boundary and forbid the latter in exact export unless the user explicitly requests a degraded export. A general `DynamicImage` success return erases that distinction.

Integration cost: AGPL decision, frontend/app-state coupling, native/platform/raw/model dependencies, different error/fallback policies. **No direct application/shader-code reuse approval.** Independently licensed foundation crates/fork components need their own audit.

Next proof: failed RAW with valid embedded thumbnail; exact export either refuses with a precise error or explicitly labels degraded output. Confirm no silent 8-bit/color/size downgrade. Record full-resolution memory and transfer behavior; README adjectives are not performance evidence.

## Adoption gate for any copied module or dependency

Before integration, the coordinator should record:

1. Exact repository/crate version/SHA, file subset and modifications; preserve upstream copyright/license/NOTICE.
2. Dependency features, resolved versions, separate native/data/model licenses, MSRV and platform costs. Avoid unsupported native/config changes.
3. Deep seam: types/input/output/errors/invariants, who owns original bytes, how revisions and cancellation travel, what is cached, and retained-byte admission policy.
4. Fidelity policy: precision, premultiplication, working color/ICC, kernel radius/border/rounding rules, requested-resolution behavior and unsupported-data preservation.
5. Concrete coordinator-owned verification requests: corpus/property fixtures, failed transaction/save, source immutability, history identity, cache perturbations, CPU/GPU tolerances, external format oracles, peak memory and transfer accounting.
6. A measured benefit and bounded integration cost. No whole-suite import or “world fastest” claim follows from menu count, Rust use, GPU use, or upstream marketing.

## Priority proof work

- **Now:** implement AFFINT's own small shared document; evaluate foundational geometry/text/raster crates separately. Complete the synthetic editorial poster workflow in `affinity-behavior.md`.
- **Before P reuse:** close selection/pattern/history budget gaps and re-prove old stale-cache findings at the pinned revision; total owners, not only tile/cache arrays.
- **Before any exact export:** explicit full-source provenance, typed failure/degraded outcome, declared color/precision policy and requested dimensions; no implicit embedded-preview fallback.
- **Before RAW:** compare R's clean engine/native boundary with L's pipeline/working gate; license-audit RAW/native/profile inputs; exact-reference fixtures for representative camera and linear DNG cases.
- **Before layout:** D's font/flow/cache lifecycle, explicit missing-font/overset errors and external PDF rendering/text checks.
- **Before more GPU:** active/free/staging/source/jobs byte accounting, CPU equivalence and observed transfer costs. A bounded free-texture cache is not a bounded rendering system.
