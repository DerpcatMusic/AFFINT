# ADR 0009: bounded F32 canvas upload and atomic GPU publication

Status: coordinator `linear-f32-native-05` passes formatting, all120 tests plus
doctest, all-target Clippy and the normal build. Real GPU/editor smoke
`linear-f32-runtime-02` also passes after correcting callback acknowledgement.
Date: 2026-10-12. This records ADR 0007's concrete native
ownership implementation, superseding ADR 0005's initial RGBA8 upload accounting.

The native viewer must retain working highlights through checker composition while
preserving the prior valid canvas on refusal or failure. A complete F32 viewport can
exceed 100 MB. The UI therefore transfers one leased `Frame` to one fixed uploader;
it never clones that pixel allocation, keeps an accepted CPU frame, or performs a
full-frame copy during painting. PNG export explicitly requests straight sRGB RGBA8.

Native request identity includes revision, page, exact dimensions/view, output
format, render serial and device generation. The render worker has one running job,
one replaceable pending snapshot and one completion. Submission drops its old
completion under the same lock before replacing pending work. A busy uploader or
private presentation candidate leaves the sole completed CPU frame in that mailbox.
This permits at most two native pixel owners: uploader plus running/completed
renderer. Both retain the existing shared 256 MiB output/scratch budget. Concurrent
RGBA8 export may add an owner only if that same byte admission succeeds.

Each canvas owns one exact RGBA32Float source and one opaque RGBA8 display texture,
20 bytes per requested pixel before driver overhead. Allocation is charged against
320 MiB and two total pair slots. The slots contain current plus private candidate,
or current plus retired work. A 3348×1926 physical viewport needs 128,964,960 bytes
per pair, 257,929,920 for two, within this policy; complex CPU scratch or a larger
viewport may still be refused explicitly. These partitions are not a global process
RSS or physical GPU-memory cap. Surface/backbuffers, Hybrid bookkeeping, MUI atlas
and layer allocations remain separate as recorded in ADR 0005.

One uploader reuses exactly two 8 MiB MAP_WRITE|COPY_SRC buffers. Rows use checked
256-byte padding, and a band never exceeds its buffer. Mapping/copying/remapping and
bounded GPU waits run on that worker. CPU pixels are a fallible borrowed byte view,
not another owned serialization buffer. Native upload requires a little-endian host
and checked actual adapter usages; the nonfiltering RGBA32Float path needs no float
filtering, blending or optional primary-buffer mapping feature. Unsupported cases
produce an error with original dimensions and precision intact.

The pinned wgpu 29.0.4 `StagingBelt` has public `allocate`, but its private active,
closed and free chunk storage can grow without a public hard allocation ceiling.
Queuing every `write_texture` band also stages a complete frame. Those reuse paths
cannot establish this fixed 16 MiB pixel-staging policy, so a small private pool is
justified. This introduces no general arena, alternate rendering framework or fork.
Bytemuck 1.25.2, already present in the dependency graph, supplies
[`try_cast_slice`](https://docs.rs/bytemuck/1.25.2/bytemuck/fn.try_cast_slice.html) for
`[f32;4]` to bytes, with default features disabled. The registry manifest declares
MIT/Apache-2.0/Zlib choices; the MIT notice is retained at
`crates/affint-app/licenses/bytemuck-MIT.txt`. AFFINT code forbids unsafe.

All uploaded bands target a private candidate. Before handoff, after upload and
before promotion, the module validates the complete identity. Validation, internal
and out-of-memory GPU failures are inspected through wgpu's thread-local scopes.
Only complete upload followed by valid display conversion and a successful final
Hybrid submission may replace current state. Missing/occluded/retrying surfaces,
stale results, admission refusal and errors retain the prior accepted canvas.
An unsampled candidate cannot become accepted. Same-sized changes use a separate
candidate too, because overwriting the current source would expose partial artwork.

The display shader loads exact source texels, composites linear working RGB over
the checker/artboard, clips and encodes once into opaque UNORM. The renderer
canonicalizes exactly zero-alpha RGB; the float shader consumes RGB unconditionally.
White vector alpha 2^-130 followed by four +32EV operations legitimately has normal
RGB 0.25 and subnormal alpha. WGSL permits arithmetic/comparison flush-to-zero, so
an `alpha > 0` floating-point guard could erase this visible RGB. The
[W3C floating-point evaluation contract](https://www.w3.org/TR/WGSL/#floating-point-evaluation)
motivates the oracle; it is not evidence of an observed prior RX 6600 failure.

After the last CPU borrow, the frame drops and its renderer lease releases. GPU
texture and staging leases remain with their owners until final submitted use,
including later Hybrid sampling. `Queue::submit` return and cancellation are not
completion. The public queue completion callback drops retired owners and wakes
the window. The uploader polls retained devices at finite 20 ms deadlines while
retirement is pending, including during settled UI idle. Old device-generation
buffers continue to consume the single staging allocation until safe retirement;
new devices cannot duplicate that allocation. Host teardown also retires its
current/candidate owners after their final submitted work. Refer to pinned wgpu
`src/api/queue.rs` (`on_submitted_work_done`), `src/api/device.rs` (thread-local
error scopes/poll), and `src/util/belt.rs` for the audited public seams.

GPU queue completion and callback completion are distinct when more than one thread
polls the device. Pinned wgpu-core 29.0.4 `device/life.rs::triage_submissions` drains
callbacks while holding the queue's lifetime lock, and
`device/global.rs::device_poll` invokes `UserClosures::fire` after those locks are
released. A second thread's `Device::poll(Wait)` can therefore observe an empty
GPU queue while the first thread still owns or executes the callback. The
[public queue callback contract](https://docs.rs/wgpu/29.0.4/wgpu/struct.Queue.html#method.on_submitted_work_done)
guarantees servicing on a polling/submitting thread, not acknowledgement of another
thread's callback by every concurrent poll.

The first real proof retained correct color/Hybrid results within1/255, then failed
its zero-pair assertion because it used GPU waiting as callback-owner-drop
synchronization. Each retirement now has a small completion token. The callback
drops its charged owner first, then publishes the token under a mutex and notifies
its condition variable. Explicit diagnostics capture registered tokens and wait
for their acknowledgements within one two-second deadline before checking the
unchanged zero-pair/byte assertions. The ordinary host remains asynchronous;
neither timeout nor acknowledgement fabricates resource release. Failure artifacts
record acknowledgement results and actual accounting before returning an error.
A deterministic regression holds an owner's destructor on a channel, verifies
that callback entry has not acknowledged completion and still applies two-pair
backpressure, then releases it and verifies real lease release. It uses no sleeps
and does not require a GPU. The mutex/condition predicate prevents a lost wake.

The extended `--gpu-proof DIRECTORY` retains the original RGBA16Float color,
physical clip and MUI ordering fixture and adds real public document/render output
through the ordinary uploader and converter. It uses independent scalar f64
oracles: alpha32 white +2EV predicts working RGB128/255 and display255 on the dark
checker, versus legacy RGBA8 display207; zero-alpha originals remain canonical;
positive subnormal alpha retains normal RGB; hidden target-local alpha/luminance/
inverted masks and isolated group opacity/exposure agree. Both checker cells are
checked. These finite fixtures allow absolute working error3e-7 and final display
error1/255; this is not a universal HDR arithmetic bound.

A nonuniform 1281×1100 raster with editable exposure requires three upload bands
and reuse of a staging buffer. Its independent per-coordinate oracle detects row
stride, offsets, swapped bands and stale buffer contents. Expected copied payload
is22,545,600 bytes, padded rows22,809,600 bytes with stride20,736; the pair charge is
28,182,000 bytes and explicit staging remains16,777,216. Runtime artifacts record
actual counts/errors, diagnostic readback bytes and released CPU leases. Padded
bytes are the sum of stride × rows for staging row spans; they are accounting
arithmetic, not measured PCIe or other bus traffic. Real
publication probes exercise private/unsampled candidates, stale serial/device
rejection, successful replacement, two-pair backpressure and completed retirement.
Cancellation crosses the ordinary mailbox and checks that no obsolete result or
CPU/GPU pair survives completion; scheduling may stop before, during or after a
copy, so the proof does not claim deterministic partial-band cancellation.

Ordinary editing has no GPU pixel readback and still uses CPU document evaluation.
Diagnostic readback is isolated to the explicit proof. No imaging GPU acceleration,
latency improvement or world-performance claim follows from this display increment.
The root runtime proof passes on RX6600/RADV/Vulkan with working error at most
9.16e-8 in these finite fixtures and final display error at most1/255. It records
the actual three-band payload/row-span counts, released CPU leases, successful
publication and zero retired texture/pair charges after callback acknowledgement.
Staging remains16MiB while the uploader is alive. Built-binary source/edit/save/
reopen/exact-export and captured-page smoke also passes. Evidence is retained in
`.research/native-editor-evidence/linear-f32-runtime-02`. Native window captures at
1024×720,1440×940 and2560×1406 logical units use150% scaling; the last size fills
the monitor's usable area below its34-pixel logical panel. Protected close exits0.
The keyboard action captures do not establish a new page-navigation result because
focus changed; the separate multipage smoke and prior native page proof retain
their established scope. No pointer/dialog/IME/accessibility or other-platform
qualification follows from these captures.
