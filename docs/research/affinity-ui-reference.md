# Installed unified Affinity UI reference

Status: resource/input round 1 with initial runtime capture, 2026-10-12. This independently worded
specification describes the owner's installed unified Affinity 3.2.3.4646 Windows
UI. It is for AFFINT implementors. Extracted vendor markup and reconstructed source
are restricted to ignored `.research/vendor-affinity/ui-reference-r1/`.

## Evidence and measurement scope

The observed installation is
`<local Affinity installation>`.
The current UI assembly, `Serif.Affinity.dll`, is 15,636,992 bytes and has SHA-256
`840c1d976699505f506d3b095b8e13e32858ef34492e8728b1476b32fd7ca312`.
The inspected launcher is the current installed 11,446,280-byte `Affinity.exe`,
SHA-256 `0b8f42e7d27eb2b50dff4ca9e357b7fc1749cf6e871a453f7473a8fd685bf68d`.
The older `.bak` assembly is outside this investigation.

ILSpy CLI 11.1.0.9782 listed embedded resources, converted fifteen specifically
selected compiled UI resources to local markup, and reconstructed selected UI/theme
types. No whole-assembly source export, application launch, desktop manipulation,
product changes, build, test, or benchmark was performed by this worker. Root-owned
screenshots were subsequently read and selected UI-only saved preferences inspected.
Missing
Windows framework references cause some decompiler type warnings; no successful
conversion is a guarantee of perfect reconstruction. The root coordinator owns
live screenshots and runtime verification.

All dimensions below are **WPF device-independent pixels (DIP)** from resources.
Multiply by the app's actual DPI scale to predict screenshot pixels. These are
static declarations, not measured runtime rectangles. Font fallback, layout
rounding, user customization, and Wine can affect the displayed result.

## Framework and shell geometry

The Windows interface uses WPF, with compiled BAML resource dictionaries in the
managed UI assembly and custom docking controls supplied by `Serif.Windows`.
The native artwork view is a separate rendering host inside the managed window.
This establishes the original's presentation organization, not a requirement for
AFFINT to adopt WPF or its internal object model.

The shell orders regions vertically: combined title/menu strip; unified
persona/home/command toolbar; tool-dependent context strip; central tool tray and
document/studio docking area; bottom status strip. The central docking area owns
document tabs and studio shelves. Studio placement is a saved workspace preference,
so the static shell alone does not establish the owner's exact studio widths/order.

| Region or component | Resource dimension or relationship |
|---|---|
| Combined title/menu strip | 33 DIP high; menu shares the title region |
| Unified persona toolbar | 53 DIP high |
| Home trigger | 48 DIP wide; image area 25 × 23 DIP |
| Persona buttons | 31 DIP high |
| Docked context panel | 30 DIP high; enclosing strip adds a 1 DIP bottom divider |
| Context panel padding | Left/right 5, top 2, bottom 1 DIP |
| Status strip | 25 DIP high; enclosing region has a 1 DIP top divider |
| Document tabs | Minimum 28 DIP high |
| Studio tabs | Minimum 28 DIP high; group margin has 2 DIP above |
| Separate studio page header | Minimum 22 DIP high |
| Main window minimum | 500 × 640 DIP |
| Main window border | 1 DIP; removed when maximized |
| Main resize grip | 18 × 18 DIP; hidden when maximized |

The toolbar has distinct persona, view-mode, fixed-command, customizable-command,
and optional app-mode sections. The app-mode section starts collapsed in this
template. The toolbar can be hidden through a persistent setting. Context controls
can overflow into a popup and the context strip can be undocked.

## Typography and spacing

| Role | Declared family/size |
|---|---|
| Normal UI | Segoe UI, 10 DIP |
| Small UI | Segoe UI, 9 DIP |
| Medium UI | Segoe UI, 12 DIP |
| Large UI | Segoe UI, 14 DIP |
| Message/system-label text | 11 DIP, normal weight |
| Caption text | 12 DIP, bold |
| Icon-font reference | 10 DIP, normal weight |

Shared positive spacing values are 2, 6, 8, 12, and 16 DIP. Do not substitute
large web-dashboard padding for these compact editor measurements. Resources
explicitly enable layout rounding at the main window and several toolbar seams.
This report does not authorize copying bundled font files or vendor icons into
AFFINT; AFFINT retains original artwork and audited/licensed fonts.

## Default dark theme

The theme distinguishes chrome backgrounds from the document pasteboard. The
values below are decoded display RGB at the declared default UI/text gamma of 1.0.
The packed color constants use red in the low byte; interpreting them as ordinary
ARGB would swap red and blue. The selected accent seed is **#007AFF** and may be
replaced by user/system accent settings. Text and chrome gamma are configurable,
so screenshot sampling must also establish the active settings.

| Role | RGB |
|---|---|
| Title/menu background | #282828 |
| Title/menu bottom divider | #222222 |
| Toolbar gradient color resources | #3B3B3B → #333333; the captured main toolbar is #404040 |
| Base/status background | #333333 |
| Context strip | #3B3B3B |
| Tool-tray/dialog background | #404040 |
| Docking/workspace background | #262626 |
| Studio divider background | #0D0D0D |
| Ordinary studio/document tab | #1F1F1F |
| Selected studio/document tab | #363636 |
| Panel content | #3D3D3D |
| Panel header/footer | #484848 |
| Layer row | #333333 |
| Layer row border | #1F1F1F |
| Primary text | #F2F2F2 |
| Control/button text | #F0F0F0 |
| Ordinary label text | #ADADAD |
| Disabled text | #939393 |
| Button/control surface | #4F4F4F |
| Editable combo/slider surface | #1F1F1F |
| Button pressed surface | #282828 |
| Toolbar hover surface | #2E2E2E |
| Toolbar selected/pressed surface | #1E1E1E |
| Toolbar selected border | #666666 |
| Toolbar pressed border | #B2B2B2 |
| Menu popup | #1C1B1B |
| Menu popup border | #525252 |
| Highlight background | #666666 |

Ruler resources declare a #454545 background with alpha 0xCC, #BFBFBF text, and
#CCCCCC ticks with alpha 0xCC. The pasteboard/background and artboard-grey scalar
defaults are 0.135 with [0,1] ranges. Those scalar values do not establish the
final screen RGB conversion; runtime capture is still required.

## Tool tray and control states

Ordinary tool icons are 18 × 18 DIP. The button template adds two nested 3 DIP
content margins, a 1 DIP border, and 2 DIP exterior margin, yielding a 36 × 36 DIP
ordinary icon cell by arithmetic. The panel adds 2 DIP to its computed width;
its one-column declared minimum is 38 DIP. The enclosing tray also contains a
40 × 18 DIP customize-tools trigger and bottom fill controls, so **38 DIP is not
a verified total exterior tool-strip width**. Live rectangles should settle that.

Tool buttons use an 8 DIP corner radius. Selection fills the cell #1E1E1E and
uses #666666 border; pressing also fills #1E1E1E but uses #B2B2B2 border. Disabled
tools apply opacity 0.5. A lower-right grouped-tool disclosure appears when the
tool item contains at least two children. A separate larger hit region accompanies
the disclosure. Missing icon data switches to text rather than an empty cell.

The tray derives its contents from the current workspace, supports a configurable
column count, has explicit separator layout, and collects unavailable-height tools
into overflow. Editing the tool arrangement enables drag/drop; ordinary tool use
does not depend on customization drag/drop. Floating tool groups have draggable
grips and original compact icon sizing.

Toolbar command-icon reference rectangles are 28 × 26 DIP. Flat studio action
buttons are 16 × 16 DIP with 2 DIP exterior margin. Toolbar separators are 13 DIP
wide. Menu icon/glyph columns reserve 20 DIP, regular menu icons are 16 × 16 DIP,
and the template reserves separate shortcut and submenu-arrow columns.

## Layers studio

The layers page has three vertical regions: a selection-properties strip, a
stretching editable layer tree, and an action footer. Its property strip places
an opacity label and 55 DIP numeric control before a stretching blend-mode
selector, blend-options trigger, and lock toggle. Opacity admits 0–100 and exposes
slider ticks at 0/25/50/75/100. The strip uses the shared 6 DIP margin and a 1 DIP
bottom border. Selected blend-mode and transient preview blend-mode are separate
bindings; only the displayed control contract is established here.

The footer contains edit-all-layers, layer effects, mask, live filter, adjustment,
group, pixel-layer and delete affordances. These are resource declarations and
command bindings, **not evidence that AFFINT currently implements each command**.
The layer tree uses 8 DIP per depth level and a 28 DIP row-content minimum.
Thumbnail settings select none, 28 × 28, 32 × 32, or 44 × 44 DIP; larger thumbnails
increase intrinsic row height. Item padding is left -1, top 1, right 1, bottom 0
DIP. Thumbnail and row selection can receive different accent treatment, and
ancestor/child rows have separately declared states. The hierarchy is a custom
virtualized tree; it explicitly realizes/unrealizes rows and tracks selected
indices. Exact resolved row height should therefore be measured for the active
thumbnail preference, rather than inferred as always 28 DIP.

The managed studio factory declares 265 DIP standard minimum width and 200 DIP
narrow width. Its default window-profile builder starts left studio shelves at
200 DIP and right shelves at 265 DIP. Page studios declare a 200 DIP minimum;
color studios have a 265 DIP fixed docked-height default, transform studios 100 DIP
automatic height, and history studios 100 DIP stretching height. These are factory
defaults: the active unified workspace also supplies page descriptors with their
own minimum/docked/floating dimensions. Saved preferences can override the visible
result, so these constants are not evidence of the owner's actual shelf geometry.

## Tool group interactions

The ordinary tool cell selects its tool on left-button release inside the cell.
Grouped tools with more than one child start a **200 ms** press-hold timer; the
timer opens a flyout. Clicking the disclosure opens the flyout immediately.
Right-button release opens a flyout with additional tool-tray commands. A cell
double-click invokes its optional double-click command. Selecting a child assigns
that child's presentation to the parent group, so the grouped cell remembers the
chosen tool. These are reconstructed control paths and still require live checks.

## Main canvas input, zoom and retile

The main artwork host is a native child window embedded in WPF. The managed
document view delegates its camera commands to that host. Native window messages
cross a native dispatch table; certain pointer messages defer to WPF when another
managed control holds mouse capture. The managed preview zoom/pan control is a
separate component, and its behavior is not proof of the main editor's gestures.

Source-bound Ghidra analysis of six main-view native functions establishes:

- Wheel zoom updates the existing tiled-view affine transform about the cursor
  location. It marks followup work and emits a zoom callback after the transform.
- In the zoom path, wheel delta is limited to [-360, 360] and divided by 120. A
  positive single-notch event gives scale 1.25; a negative single-notch event gives
  0.8. Multi-notch events use the read event's linear factor, so three positive
  notches in one event give 1.75, not 1.25 cubed. Fractional event arithmetic has
  intermediate float precision. Zoom-versus-scroll and alternate rotation depend
  on active preferences and modifier keys; the actual selected defaults need a
  settings/runtime check.
- The alternate scroll path divides the signed wheel delta by four with integer
  truncation toward zero, selects an axis from modifiers/horizontal-wheel state,
  and forwards the displacement to the view controller. A 120-unit event therefore
  supplies 30 displacement units; the complete device/DPI interpretation remains
  a later seam to verify.
- The managed zoom-start callback restarts a 100 ms dispatcher timer and updates
  scrollbar, cursor, and zoom notifications. The timer calls the native zoom
  completion routine, which invokes transform rationalization and retiling. A
  separate explicit refresh path requests forced rationalization/retiling.
- The scroll wrapper forwards the displacement to the document-view controller,
  then updates the tool's device viewport, logical/device transform, scale and
  visibility options. Ruler visibility changes the viewport's origin adjustment.

This supports an interactive camera stage followed by later exact tile work.
It does **not** establish when every tile becomes accurate, the frame scheduling
cadence, cache-key completeness, stale-job rejection, a GPU fence, or any measured
latency. AFFINT can implement immediate presentation transforms while exact work
is pending, provided source/output fidelity and truthful pending state are retained.
Deferred retiling is a scheduling policy, not permission to reduce saved/exported
resolution or discard editable operations.

## Restricted evidence register

| ID | Restricted path below `.research/vendor-affinity/ui-reference-r1/` | Coverage |
|---|---|---|
| U1 | `inventory/Serif.Affinity.resources.txt` | Exact resource-name inventory |
| U2 | `inventory/Serif.Windows.resources.txt` | Windows support resource inventory |
| U3 | `resources/core.xaml` | Fonts, spacing, shell/toolbar/tool control templates |
| U4 | `resources/personastyles.xaml` | Tool tray and context strip templates |
| U5 | `resources/studiostyles.xaml` | Docking, document tabs and studio tabs |
| U6 | `resources/{layerpage,colourpage,transformpage,historypage,pagenavigationbar}.xaml` | Selected studio/page controls |
| U7 | `managed/{DarkScheme,SchemeColourEntry,SystemAccentSchemeColourEntry,SchemeColorsDeferredResourceDictionary,ExtensionMethods}.cs` | Default color values, color-channel interpretation and configurable gamma/accent seams |
| U8 | `managed/{AffinityWindow,ToolTrayPanel,ToolTrayButton,ToolTray,ToolPanel,FrameworkToolBarPanel}.cs` | Selected layout/control relationships |
| U9 | `inventory/{extraction-log,types-log,dark-palette}.json` | Extraction results and source-derived palette register |
| U10 | `resources/layerview.xaml`, `managed/{LayerView,Workspace,WorkspacePageFactory}.cs` | Layer-tree layout/virtualization and managed studio defaults |
| U11 | `managed/DocumentView.cs`, prior `../round-03/managed/DocumentRenderControl.ilspy.cs` | Main artwork host, camera/callback bridge and 100 ms completion timer |
| U12 | `inventory/Serif.Interop.Persona.methoddefs.json`, `inventory/native-input-methods.json` | All 77,390 mixed-bridge method declarations and focused input candidates; names are inventory, not all-method behavior coverage |
| U13 | `native/input-main.plan.json`, `native/input-main/{functions.json,*.asm.txt,*.pseudo.c}` | Six source-bound native input/camera functions, all six decompilations completed |
| U14 | `preferences/manifest.json`, `preferences/Settings/`, `preferences/Workspaces/` | Twenty-one UI-only settings/geometry snapshots; account, recent-document and service files excluded |
| U15 | `native/pan-camera.plan.json`, `native/pan-camera/180daa6b0.pseudo.c` | Source-bound 20 DIP ruler scalar |
| U16 | root-owned `../../affinity-ui-reference-runtime-01/settled-0.png` | Actual initial Slice/no-document/Home state at 150% scale; not Vector parity |

Raw evidence is for authorized researchers only. Implementors should use this
independently worded specification and root-owned screenshots. The stable
`coverage.json`, `provenance.json`, and `artifact-digests.json` bind this round and
distinguish inventory, reconstruction and actual focused reads.

## Saved preferences and initial live geometry

Twenty-one selected UI-only settings/workspace files were read and retained under
`preferences/`; the manifest binds their source bytes. At that snapshot the owner
uses Dark with UI/text gamma 1.0 and the default blue accent seed, monochromatic
icons, a docked one-column tool tray, a docked visible context strip and visible
status strip. Rich tooltips are enabled with a 400 ms tooltip delay. Ordinary wheel
events scroll because wheel-zoom preference is false; Ctrl reverses that gate and
zooms. Scrubby zoom is enabled, Alt-wheel rotation disabled, and tool switching
does not require Shift. These are saved settings, not a live exhaustive input test.

Root's valid `.research/affinity-ui-reference-runtime-01/settled-0.png` is
2869 × 2085 physical pixels. Saved window dimensions are approximately
1913.333 × 1390 DIP, consistent with **150%** scaling. The capture shows Slice
selected, no document, and an owned Home overlay, so it does not establish the
Vector document workspace. Unobscured pixels exactly match title #282828,
toolbar/tool tray #404040, context #3B3B3B and workspace #262626. At an unobscured
column, title/toolbar/context boundaries occur near physical y=50/132/179,
corroborating the declared 33/53/31 DIP region stack with frame and rounding.
The left tool-tray interior spans about 60 physical pixels, consistent with
40 DIP. The right studio shelf starts near physical x=2474 and occupies about
395 pixels including boundary/frame, consistent with the 265 DIP factory width.
These measurements are valid for that captured Slice/no-document state only.

Document tabs have square corners, right/bottom 1 DIP borders and text margins
8 DIP left/3 DIP right; the final tab removes its right border. Hover and selected
tabs both use #363636. The close glyph occupies 8 × 6 DIP. Studio selected/hover
tabs instead use a 5 DIP radius pill inset 1 left/2 top/0 right/5 bottom DIP within
the 28 DIP header. Their label margins are 5 left/-4 top/5 right/0 bottom DIP and
the message-font size is 11 DIP.

The native ruler dimension query returns **20 DIP**. This is a source-bound scalar,
not a captured ruler rectangle; no document/rulers are visible in the initial
capture.

## Remaining unknowns and next evidence

The owner has explicitly stopped Affinity window launching, input, focusing,
closing and new GUI probing. Compiled resources and selected saved preferences
are the current implementation authority. Existing captures remain historical
corroboration only; future Affinity reference research must remain static. The
brief source-only startup/Home investigation is closed and creates no runtime
capture prerequisite.

The owner's exact Vector workspace association, tool order, selected document
context controls and studio book heights remain unresolved static questions.
Fourteen serialized workspace profiles were read, but a profile's page-group
composition alone is insufficient to associate its UID with a named persona.
One candidate profile has Color/Swatches/Stroke/Appearance; Layers/Line Style/
Effects/Styles; and Transform/Navigator/History groups, with Color, Layers and
Transform selected. Its Vector association is an inference, not a verified fact.

Saved color geometry is Triangle and the fill-flyout default is Wheel. The Color
studio independently binds an active mode that can be Wheel, Sliders, Boxes or
Tint; these saved preferences do not establish its active studio mode. A hue ring
with a triangle is consistent with the saved wheel geometry but must be marked
provisional if selected as AFFINT's initial composition.

These specific limits do not prevent implementing the compiled chrome dimensions,
palette, control states, docked geometry and measured saved input preferences.
Resource presence and control names do not establish complete interaction parity,
native accessibility, Linux behavior, responsiveness, or a finished editor.
