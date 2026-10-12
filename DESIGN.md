---
name: AFFINT Native Editor
description: Installed unified Affinity reference, implemented with original native MUI chrome.
colors:
  title: "#282828"
  toolbar-top: "#3B3B3B"
  toolbar-bottom: "#333333"
  context: "#3B3B3B"
  tray: "#404040"
  workspace: "#262626"
  tabs: "#1F1F1F"
  selected-tab: "#363636"
  panel: "#3D3D3D"
  panel-header-footer: "#484848"
  layer: "#333333"
  selected-layer: "#666666"
  ink: "#F2F2F2"
  muted: "#ADADAD"
  control: "#4F4F4F"
  field: "#1F1F1F"
  selected-tool: "#1E1E1E"
  selected-tool-border: "#666666"
  focus: "#007AFF"
typography:
  body:
    fontFamily: Adwaita Sans Regular
    fontSize: 10px
  label:
    fontFamily: Adwaita Sans Regular
    fontSize: 9px
  medium:
    fontFamily: Adwaita Sans Regular
    fontSize: 12px
  large:
    fontFamily: Adwaita Sans Regular
    fontSize: 14px
rounded:
  tool: 8px
  studio-tab: 5px
  field: 2px
  document-tab: 0px
spacing:
  tight: 2px
  compact: 6px
  grouped: 8px
  panel: 12px
---

# AFFINT native editor design authority

The owner requires a faithful 1:1 installed unified Affinity interface and workflow.
The earlier generic compact shell is rejected. The verified core checkpoint does
not establish UI acceptance or acceptable interaction performance.

The authority is the owner's installed Affinity 3.2.3.4646, the independently
worded resource/workspace report in `docs/research/affinity-ui-reference.md`, and
the coordinator's actual owned-window captures. Restricted vendor source, markup,
icons and font assets are not implementation inputs. AFFINT keeps its independently
drawn mark and monochrome symbols, authored document and licensed font bytes.

## Evidence and scale

`.research/affinity-ui-reference-runtime-01/settled-0.png` establishes the settled
dark shell at 150% scale, with Slice selected and the Home overlay present.
`settled-1.png` captures the Home popup. Initial animation captures are invalid
layout references. Source inspection establishes the chrome and saved triangle
geometry. Exact Vector profile association, tool ordering, context sequence and
active Color studio mode remain unresolved native descriptor details. Initial
composition is provisional in those areas. The owner stopped further Affinity
launches; use source resources and the existing captures for this increment.

Resource dimensions are WPF DIP. AFFINT uses logical native units and converts to
physical pixels once at the window/canvas seam. Font substitution, rounding,
saved workspace preferences and Wine can change the resolved appearance.

## Shell geometry

The combined title/menu region is 33 DIP, the unified persona toolbar 53 DIP,
the context region 30 DIP plus its 1 DIP bottom divider, and status 25 DIP plus its
1 DIP top divider. Documents and studio groups have 28 DIP tab regions. Documents
sit inside the central docking area, beside studios; the document strip is not
an additional full-width command bar. The one-column tool tray is 40 DIP in the
saved/reference state, with 36 DIP cells and 18 DIP symbols.

The native minimum is 500×640 DIP. The vertical ruler occupies a fixed 20 DIP
width and fills height; it never takes a horizontal flex share from the canvas.
The right studio factory defaults to 265 DIP. Actual Vector composition must be
checked against source profiles and existing captures; factory defaults do not
establish active book heights or persona association. Layer indentation is 8 DIP; 28 DIP thumbnails
currently produce 32 DIP AFFINT rows. Realization stays bounded by resolved
scroll height with small overscan.

## Typography and surfaces

Installed resources declare Segoe UI at 10 DIP normal, 9 small, 12 medium and 14
large. AFFINT embeds unmodified Adwaita Sans Regular, the system Fontconfig
fallback for Segoe UI. This is an audited substitution, not proof that Wine uses
identical glyphs. Its OFL 1.1 notice and provenance are retained in the app.
Existing reference captures and new AFFINT captures support visual comparison.

Reference grays distinguish title, toolbar, tray, selected tabs, panel content,
layer rows and fields. The selected tool has a dark face, gray border and radius
8; studio selection uses a small radius-5 pill; document tabs stay rectangular.
Focus and canvas selection use the recorded blue accent. Artwork colors remain
document content and are unaffected by theme.

## Functional chrome

Menu placement follows File, Edit, Document, Text, Vector, Pixel, Layer, View,
Window and Help. Supported menus have separate title and shortcut columns.
Unsupported commands and tabs remain disabled with truthful reasons. There are
no enabled placeholders for text, strokes, brushes, artboards, styles, accounts,
AI, cloud or slice export.

Vector, Pixel and Layout workspace choices expose supported tool/operation/page
workflows in the same editor. They do not establish commercial persona parity.
File actions, history, pages, selection, shapes, crop, opacity, exposure, mask,
ordering and visibility invoke actual commands. Properties belong to studios,
not a generic “Live properties” form. Rulers and transform readouts derive from
real page/selection/camera state.

Continuous edits are provisional, cancelable and one undo step. Protected
Save/Discard/Cancel handles dirty close/replacement. Failures and pending exact
rendering retain the prior valid canvas. Counters belong in explicit diagnostics.

## Input and acceptance

Consumed input and repaint work have separate paths. Repaint does not consume
canvas input; stationary held responses do not issue another drag. Document
points are compared exactly, including camera changes. Native motion is coalesced
and consumed once before painting.

The camera increment separates requested camera intent, the immutable evaluated
source key, and a receipt recorded after actual presentation. Hit testing, selection
and rulers use that receipt. Starting a new visual canvas edit requires its document
revision to match the committed document; an already active gesture can continue.
Supported translations retain one F32 texel per physical pixel and recompute page
and checker coordinates. Required-density zoom or missing page AA coverage keeps
the displayed camera coherent with a pending status. No old viewport is magnified.

The actual evaluated request includes 64 physical pixels of overscan on each edge
when renderer and GPU admission permit it. A viewport-only fallback removes that
prefetch without reducing visible density. Source cropping remains
`RetainedAtNativeDensity` with the original evaluation key and an exact followup;
it does not assert fresh viewport vector AA equivalence. Camera-only work coalesces
behind active work, and zoom settles for 100 ms. Pixel-wheel calibration is AFFINT's
24 logical pixels per notch; the independently specified signed linear zoom factors
cap each event at three notches. This authored increment awaits root verification.

`--interaction-profile DIRECTORY` requires `AFFINT_DIAGNOSTICS=1` at launch.
It records bounded CPU wall timings, render counters and sampled/accepted serials
in memory, then writes JSON with exclusive creation after the event loop ends.
Surface submission observations are not GPU execution or scanout timings.
Window physical/logical sizes, actual scale factor, source dimensions, displayed
mapping and retained-renderer counters are recorded directly, without DPI inference.

Inspect actual native captures at 1440×940 and 1024×720 logical sizes and 150% DPI.
Verify drag, hold, pan, cursor-centered zoom, color/opacity, undo/cancel, menus,
clipboard and protected close. Compare verified resource geometry and existing
reference before claiming fidelity. The replacement awaits coordinator review
and is not accepted.
