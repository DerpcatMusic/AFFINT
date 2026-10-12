# Native unified editor surface

Platform: native Linux desktop with intrinsic MUI, winit and the same-device
Vello Hybrid/wgpu canvas host. Windows/macOS remain portability objectives.
Authority: the owner's installed unified Affinity 3.2.3.4646 and actual reference
captures, with original AFFINT branding/icons and audited font bytes.

The generic shell was rejected for appearance and severe interaction lag. Core
verification at checkpoint `a0ad784` establishes document/render/upload evidence,
not UI acceptance. Reference measurements and limitations are in
`docs/research/affinity-ui-reference.md` and `DESIGN.md`.

The replacement orders a 33 DIP title/menu strip, 53 DIP unified toolbar, 30 DIP
context strip plus divider, docking area and 25 DIP status plus divider. A 40 DIP
tray uses 36 DIP cells. Document and studio headers use 28 DIP. Color/Swatches,
Layers/Effects and Transform/History/Pages expose supported commands/state;
unimplemented neighbors remain disabled. Full docking, commercial persona parity
and multi-document tabs are not implemented.

The single document opens an authored study with editable geometry and unchanged
original raster samples. Real tools: Move, Rectangle, Ellipse, Crop, Hand; Zoom
actions operate the actual camera. Fill, opacity, linear-light exposure, crop
and mask are reversible operations. Layers support selection, rename, order,
visibility and deletion. Blank pages copy dimensions, not artwork. Native files
preserve every supported page/layer/source/operation.

Shortcuts: V/R/E/C/H; Ctrl+N/O/S, Ctrl+Shift+S; Ctrl+Z/Shift+Z/Y;
Ctrl+0 fit, Ctrl+1 actual; Delete; Ctrl+Page Up/Down outside name editing;
Ctrl+Q uses File > Quit and the protected Save/Discard/Cancel close flow.
macOS command is accepted in place of Ctrl. Wheel pans; Ctrl+wheel zooms about
the pointer; Hand/middle drag pans. Text/IME and copy/cut/paste belong to focused
`layer-name` editing.

Consumed native input and repaint work are separate. Canvas drags require actual
document-point change. Stationary held MUI responses and graphics wakes do not
mutate previews or cancel rendering. The renderer also rejects semantically
unchanged previews. Continuous edits commit as one undo entry; Escape/focus loss
cancel them. Keyboard opacity changes retain their preview when MUI delivers its
atomic Begin/End on the following frame, then commit exactly one undo entry.

File commands use native dialogs and bounded workers. PNG import preserves
supported original samples and reports declared/assumed sRGB. Save stores editable
content. PNG export captures page and snapshot before its dialog and evaluates
full whole-pixel dimensions; later input cannot retarget it. Unsupported color
and metadata is refused. Dirty close/new/open uses Save/Discard/Cancel.

Ordinary display evaluates exact linear-sRGB premultiplied F32. One upload worker
borrows the frame into two fixed row-band staging buffers and a private
Rgba32Float candidate. Conversion and successful Hybrid submission precede
publication. Stale/refused/canceled/failed work preserves the current pair.
Retirement accounts for GPU completion; readback belongs only to explicit proof.

The authored camera increment plans the sampled candidate before composing chrome
and records the displayed camera only after presentation. Native-density retained
translation keeps its evaluated source key and pending followup. Checker/artboard
conversion uses destination coordinates. Zoom needing new density holds a coherent
display instead of magnifying the previous viewport. Canvas picking, selection and
rulers share the displayed mapping; a new canvas edit waits for matching document
content. Bounded 64 physical-pixel overscan has an explicit viewport-only admission
fallback, with full visible density preserved. Camera-only requests coalesce behind
active work and zoom settles for 100 ms. The native renderer enables 128 MiB retained
work inside its shared 256 MiB budget; diagnostic fresh references stay uncached.
This increment has not yet passed coordinator verification.

Coordinator entrypoints:

```
affint-app --export-demo PATH.png
affint-app --smoke-test ARTIFACT_DIRECTORY
affint-app --gpu-proof ARTIFACT_DIRECTORY
AFFINT_DIAGNOSTICS=1 affint-app --interaction-profile ARTIFACT_DIRECTORY
```

The profile retains at most 4096 samples and 128 completed-render timestamps,
with drop counts. It separates input/paint tree/frame costs, matched exact-render
age, consumed-input-to-surface-presentation observation, cancellation/replacement
counts and host stage CPU wall durations. `interaction-profile.json` is written
after event-loop exit with exclusive creation. These are diagnostic observations,
not a benchmark or GPU execution/display latency measurement.
Actual physical/logical window and viewport dimensions, scale factor, evaluated
source key/crop, display mapping/pending mode and retained cache charges/counters
are included; evaluation age refers to that source rather than a relabeled viewport.

Acceptance uses verified source resources/profiles and existing reference captures;
the owner stopped further Affinity launches. Exact Vector profile/tool/context
ordering and active studio color mode remain explicitly provisional. Inspect new
AFFINT captures at 1440×940 and 1024×720 logical, including 150% DPI. Verify controlled
real drag/hold/pan/zoom, one-step undo/cancel, opacity/color, clipping/menu order,
dirty save/reopen and clipboard. Headless/build/GPU passes do not replace native
interaction evidence. Replacement appearance/performance acceptance is pending.
