# Initial native shell and rendering integration

Status: superseded by Matari UI for the first shell; retained fallback, 2026-10-12.

The owner subsequently asked to prefer their intrinsic MUI framework if it fits
this workload better. No egui application code has landed. Inspect MUI's actual
native/GPU/input APIs and proof-build the selected integration before committing
the first shell. The source review now recommends MUI: ../research/mui-fit.md
verifies its native host, public same-device external texture seam, dependency
family, and concrete integration costs. The first MUI host proof uses wgpu 29.0.4
and Hybrid 0.2.0 consistently. The reasoning below records the earlier egui/GPUI
comparison, not a second shell implemented in parallel.

The owner delegated toolkit selection and suggested GPUI/GPUI Kit as candidates.
The workload requires editable full-resolution pixels, vector coverage, masks and
live operations. UI chrome speed alone does not establish imaging performance.

The source-verified report in ../research/ui-and-rendering.md found shared wgpu
device/queue access, native texture registration, and custom render callbacks in
released eframe/egui-wgpu 0.36.2. These offer a concrete path to presenting our GPU
renderer without routine full-frame readback. A float texture requires a deliberate
display-conversion shader; the default image texture path is not sufficient.

The examined GPUI/GPUI Pre public surface element is Apple-only; its canvas callback
is not itself an external Linux wgpu-texture interface. A GPUI Linux canvas could
be made to work, but a maintained renderer modification or another demonstrated
seam is required. GPUI Kit's current gpui-pre family also differs from gpui 0.2.2.
Starting with an unproven fork would divert work from document/render correctness.

Choose eframe/egui with wgpu for the first native shell, with independently owned
document and rendering modules. Do not introduce a hypothetical UI-backend trait.
Keep actual UI knowledge local so a later GPUI shell is possible after its texture
integration and maintenance costs are demonstrated. This decision makes no claim
that egui is the world's fastest toolkit, nor that its default widgets meet the
desired Affinity-like workflow without deliberate design.

Initial source pins: egui/eframe/egui-wgpu 0.36.2; wgpu 30.0.1. Vello 0.11.0 is an
optional vector/coverage candidate, not an unqualified floating-point photo
compositor. Coordinator verification will lock the actual compatible graph.
