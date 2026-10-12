# ADR 0006: active pages in the native canvas

Status: implemented and coordinator verified, including native page inspection.
Date: 2026-10-12.

The document already stores ordered pages and supports validated `AddPage` and
`SetPage` transactions. The original shell displayed only the first page. The
native editor now exposes existing pages and real blank creation without adding
another document model or changing storage/rendering interfaces.

## Decision and behavior

`Editor` privately owns a typed active `PageId`, checked against the current
snapshot whenever used. The session remains authoritative for names, dimensions,
order, roots and page existence. Native opening initially selects the first stored
page and preserves every page. Active-page preference is transient application
state; native save is an all-page document save, not a UI-preference save.

Previous/Next controls and a bounded name/count label occupy the existing 30px
document strip. File supplies Add blank page; View supplies page navigation.
Ctrl/Command+Page Up/Page Down navigate outside layer-name editing. Controls disable
at the ends of page order and at the document's 64-page limit. Only the current page
name is shaped, shortened to 32 characters; no giant page selector is introduced.
The existing minimum 1024×720 logical size and single DPI conversion remain.

Switching cancels document/pan gestures, commits pending layer-name text, clears
incompatible selection/name state, fits the chosen page and invalidates the exact
request. Navigation itself creates no document mutation/history entry; committing
edited name text is a separate real rename edit. A bounded layout refresh
removes the old page's layers/selection before presentation. Old-page frames are
dropped; the prior GPU texture is not sampled while the new page has no valid frame.
Current-page layer and property scroll IDs include PageId, so another page's long
list offset cannot hide a short or empty list.

Add page issues one atomic `Change::AddPage`, with the current page's validated
dimensions and a concise default name. It selects/fits the new empty page and
creates one undo entry. It never duplicates artwork, source assets or pixels.
Creation refusal preserves committed artwork/history. There is no Page Remove
control because the core has no removal command.

Undo removing the active created page falls back to the first valid page and
clears missing/foreign selection. One bounded identity bookmark lets redo restore
the removed active page. An explicit page choice or new creation clears that
bookmark; redo then preserves the user's later page choice. Branching creation
uses the core allocator and cannot reuse an undone PageId. Selection itself is
transient; redo does not fabricate a selected layer.

## Command and worker scope

Hit testing traverses only active-page roots. Selection, visibility and selected
layer operations validate parent ancestry to the active PageId before editing.
Shapes/masks are inserted into that page. Existing cross-page mask dependencies,
if supplied by a supported document, retain document semantics; page switching is
not permission to alter those dependencies or originals.

PNG import records its target PageId before opening the native dialog. Completion
uses that identity and validates that it still exists; it does not retarget to a
later active page. An import into an inactive page does not replace current-page
selection. Native dialog shielding and the one-file-job policy remain intact.

`Editor::export_request` captures a committed immutable snapshot and validated
whole-pixel full-page `RenderRequest` together. The file job carries both before a
dialog or render runs. Later navigation cannot change the exported page or its
revision. Fractional/unsupported dimensions are refused explicitly. PNG remains
the renderer's current straight RGBA8 sRGB output; the proposed F32 native seam is
a separate increment and is not implemented here.

PageId already participates in exact render-request equality. Revision, view,
worker serial and device generation checks reject other-page/stale results.
Dropping a frame releases its existing renderer lease; no owned pixel escape or
page cache was added. Native saves still capture all pages, and older save receipts
cannot clear newer edits. Dirty close/new/open and native clipboard revision/node
tickets retain their existing policy.

## Verification and limits

New meaningful regressions exercise distinguishable red/blue originals on two
pages; page-scoped select/move and captured import; shared source allocation after
blank creation; creation undo/redo/branch identities; canceled move/shape navigation
without revision/history/dirty changes; explicit choice across redo; atomic limit
refusal; captured-page export colors; and same-revision/view other-page handoff.
The original 13 app tests remain; the source increment adds six, for 19 expected.

The existing `--smoke-test DIRECTORY` now also writes `multipage.affint`,
`multipage-active.png` and `multipage.txt`. It exercises real blank creation and
history, independently authored second-page raster/vector content, all-page native
roundtrip/source preservation, initial first-page policy and captured active-page
exact export after navigation. No worker executed it or claimed a passing result.

Coordinator verification: the combined app/render `scripts/verify` lane, then the
normal built binary's `--smoke-test` under the exclusive coordinator lock. Native
confirmation should add a blank page, navigate back to the original artwork,
undo/redo creation, inspect the compact File/View menus and confirm both required
logical sizes with fractional DPI when available. Tests are not manual GUI evidence.

Page name/dimension editing, page reorder/removal, multiple document tabs, docking,
layout/typography, broad interchange, managed HDR/ICC, GPU document evaluation and
complete accessibility remain outside this bounded increment. Existing original
synthetic study creation is explicitly named; Add page creates a genuinely blank
page. No dependency, core, renderer or storage change is required.

Coordinator outcome: `core-pages-fidelity-03` passes all 19 app tests, the full
workspace, formatting, all-target Clippy and normal app build. Built-binary
`pages-runtime-03` smoke preserves both pages/originals and captured 960×720 export
after navigation; the fresh GPU proof passes within 1/255 on RX 6600/Vulkan.
Owned native captures confirm blank creation, navigation and creation undo/redo
at 1024×720 and 1440×940 logical units with 150% DPI. File/View menus fit; protected
Discard exits 0. Native pointer/dialog/IME composition/accessibility are still
unverified. The source worker did not execute these coordinator checks.
