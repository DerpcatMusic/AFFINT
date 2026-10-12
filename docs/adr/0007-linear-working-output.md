# ADR 0007: retain linear working color through native display

Status: verified in coordinator `linear-f32-native-05` and real GPU/editor smoke
`linear-f32-runtime-02`, with bounded native window observations. See ADR0009.
Date: 2026-10-12.

The document evaluator already retains f32 linear-premultiplied highlights during
operations. Its initial RGBA8 output cannot carry that information into subsequent
checkerboard composition. An independently derived white/alpha32/+2EV example
predicts display207 instead of255. This is a declared initial precision limit;
the new native path must remove it without changing explicit PNG output policy.
See `docs/research/linear-display-followup.md` and `gpu-reuse.md` for source evidence.

## CPU working output first

Keep RGBA8 rendering as an explicit export representation. Add
an explicit linear-premultiplied f32 representation for native presentation. Output
format belongs in the immutable request identity alongside page, dimensions and
view; stale-result checks must compare it. No automatic precision downgrade.

Use one owned pixel representation per frame and a borrowed typed view such as
`FramePixels`. Do not expose a detached owned pixel buffer, fabricate an empty
RGBA8 slice for a float frame, or panic on a format mismatch. Existing consumers
must handle the representation explicitly. Both variants retain their byte lease;
dimension arithmetic and admission account for4 versus16 bytes per pixel.

The ordinary native path uploads borrowed f32 pixels into nonfiltering RGBA32Float
storage and runs the existing linear-premultiplied display conversion. The source
lease stays alive for every retained owner. Any cast uses a proven safe crate;
app-owned unsafe and a second full-frame conversion allocation are unnecessary.
Clip and encode once after checker/artboard composition. Hybrid remains the final
opaque RGBA8 display seam. Diagnostic readback remains explicitly separate.

Before switching formats, redesign admitted output, current/candidate/retired
texture and staging budgets. A4MP float-source/display pair is76.29MiB; two are
152.59MiB, already above the current96MiB policy. Record simultaneous ownership,
queue completion and device generation, rather than simply raising a texture cap.
Keep original pixels, requested dimensions and the old valid result on refusal.
No global process or driver-memory cap follows from these separate partitions.

The selected native implementation keeps the shared CPU renderer at256MiB and
admits at most two native CPU pixel results through uploader/worker backpressure.
Explicit export also uses this shared renderer; any additional RGBA8 export owner
must fit the same byte admission and may be refused while native work is live.
Accepted display state retains revision/request metadata and its GPU pair after
the upload no longer borrows CPU pixels. The lease releases with those pixels;
metadata never fabricates ownership of a dropped frame.

Canvas residency is320MiB with a hard two-pair ceiling: current plus candidate,
or current plus retired. Two full-monitor3348×1926 F32-source/RGBA8-display pairs
need about245.98MiB. Each actual dimension uses checked arithmetic, and a larger
request or complex CPU scratch can still be refused without resizing. The limit
does not admit another pair while a retired owner awaits completion.

Explicit upload staging is two8MiB MAP_WRITE|COPY_SRC buffers,16MiB total, reused
for256-byte-padded row bands by one fixed worker. Chunked `queue.write_texture`
alone cannot establish this ceiling: queuing every band still stages a full frame.
The pinned wgpu29 `StagingBelt::allocate` privately grows active/closed/free chunk
owners without a public hard allocation ceiling, so it is not a drop-in bounded
pool. A small fixed-buffer state machine uses public wgpu mapping/copy/completion
interfaces; no general arena, dependency fork or optional primary-buffer mapping
feature is required. Driver/toolkit overhead remains separate and unmeasured.

Candidates stay private through all row uploads. A complete, current candidate is
promoted only with successful conversion and Hybrid submission. Surface retry,
obsolete work, refusal and upload error preserve the previous accepted display.
Copied CPU pixels can then drop while staging and texture owners remain through
their final submitted use. Cancellation and submission return do not release a
GPU lease. The completed coordinator proof verifies these requirements, including
actual callback owner-drop acknowledgement after concurrent polling.

Acceptance uses a real document with translucent exposure/masks and an independent
linear mathematical oracle, then actual GPU display readback. The current synthetic
half-float proof alone cannot establish the new document handoff. Preserve existing
RGBA8/finer-coverage/analytic/source/lease tests and final≤1-code display tolerance.
Record an additional working-float tolerance before enabling acceleration.

## Accelerated document slice afterward

Reuse the pinned wgpu29 device/queue, Kurbo geometry and Hybrid presentation.
The bounded reuse audit found no inspected drop-in that provides the entire
float/mask/isolation/ownership contract. Independent operation kernels are justified;
copying a whole editor or writing another GPU/geometry framework is unnecessary.

First accelerate originalRGBA8 source residency, linear-premultiplied four-tap
affine sampling, ordered crop/exposure/mask/opacity and isolated group composition.
Use bounded RGBA32Float tile scratch and current CPU coverage. Preserve source
centers, transparent border taps, target-local hidden-mask evaluation and all
semantic request inputs. Each candidate is one revision; partial tile completion
must not mix revisions into the accepted result.

Original sources upload only when absent from admitted residency. Intermediate
and final owners remain charged through their last GPU submission, including later
UI sampling. Public queue completion is the retirement seam; submission return and
cancellation are not completion. Ordinary presentation has no pixel readback.
Any required scalar GPU-error handoff must be disclosed and measured separately.

Validate actual adapter usages/features and coordinate/numeric accuracy. RGBA32Float
nonfiltering shader sampling/composition avoids optional float filtering/blending.
Unsupported accuracy or budget produces an explicit refusal or the declared exact
CPU path. Half-float is a separate future precision policy: positive16EV can exceed
its65504 range, and f32 shader arithmetic cannot recover rounded/overflowed stores.

Public Vello Common CPU coverage is a later candidate with its own independent
gates. Opposite-winding disjoint subpixel contours, holes, seams and numeric/work
limits must pass before replacing current8× coverage. The current optimization
does not change that policy. No GPU speedup or world-performance claim is established.
