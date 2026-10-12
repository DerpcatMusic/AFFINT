# Linear working output: release observations

2026-10-12. The root coordinator built and measured the verified CPU renderer's
explicit RGBA8 and linear-premultiplied F32 representations. This measures CPU
evaluation and output allocation/writing, not native upload, GPU document processing,
input latency or comparison with another editor. All successful runs completed
their requested samples and released every renderer charge after dropping frames.

## Build and workload scope

The retained release executable is
`.research/benchmarks/linear-working-output-01/workloads-release`, SHA256
`143c6a2508e953239faf9742e3794b8dc6e86f375ccbccf14ae86416f959a59f`.
Its environment record contains exact source hashes, Rust/Cargo versions, build
command and the preceding coordinator verification `linear-f32-native-04`.
The release settings match the earlier observations: opt-level3, thinLTO,
codegen-units1 and line-table debug information. Hardware remains Ryzen7800X3D;
the CPU governor is `performance`. Rust1.98.1 compiled this increment. Document and
CPU-render crates declare1.85; storage/native app inherit the workspace's1.95.
Neither minimum has been separately tested. No call-stack profile was recovered.

The procedural photo scenes, source dimensions, affine placements, live operations,
mask/group semantics, 8× canonical coverage and 256-pixel color tiles remain the
same. CSV schema2 now names the output representation. Setup, the first admission
probe, warmup, output inspection and frame drop remain outside sampled render calls;
deadline checks remain inside them. Both formats use the same executable, with no
implicit conversion or reduced-resolution representation.

## Observed results

| Exact output | Format | p50 / observed p95 seconds | Warmups / samples | Output bytes | Peak renderer charge bytes | Child peak RSS KiB |
| --- | --- | --- | --- | --- | --- | --- |
| 6000×4000 | linear F32 | 2.544343677 / 2.878017069 | 1 / 7 | 384000000 | 392757312 | 574480 |
| 6000×4000 | straight sRGB RGBA8 | 2.938565532 / 3.389442496 | 1 / 7 | 96000000 | 104757312 | 293364 |
| 8000×6000 | linear F32 | 5.054917420 / 5.140467940 | 1 / 3 | 768000000 | 776757312 | 1136556 |
| 8000×6000 | straight sRGB RGBA8 | 5.880998030 / 5.890335945 | 1 / 3 | 192000000 | 200757312 | 574124 |

Original source charges remain192000000 bytes for24MP and384000000 for48MP.
Held frame charges equal the output-byte column, and every released charge is0.
The larger F32 output is deliberate retained precision, not an allocator regression
or a second pixel copy. These module counts are separate from process RSS and GPU
budgets. Linux child-rusage covers the complete fork/exec process lifetime and may
include a launcher floor. It is not an allocator attribution or global memory cap.

F32 avoids final unpremultiplication/sRGB encoding but writes four times as many
output bytes. The observations record the resulting whole-call cost. The runs were
sequential rather than randomized/paired; other desktop/user work was uncontrolled,
and separate Codex source work could run. Seven/three samples make p95 the observed
maximum, not a reliable tail guarantee. RGBA8 timings also differ from the earlier
2.69s/5.38s observations, so neither a causal format speedup nor a cross-increment
regression is established by these runs.

## Admission remains explicit

The default512MiB harness budget admits24MP F32 but cannot admit48MP F32's output
plus scratch. A deliberate512MiB refusal probe returns exit2 and
`render allocation needs 776757312 bytes with 0 of 536870912 already charged`.
It retains the requested8000×6000 dimensions, emits no frame, records zero completed
samples and leaves zero renderer charges. There is no downscaling or RGBA8 fallback.
The successful48MP F32 run explicitly selected1024MiB. This does not raise the
native editor's shared256MiB CPU output/scratch partition or320MiB GPU canvas limit;
native presentation evaluates the requested viewport against original source pixels.

## Reproduction and retained evidence

Every invocation used `scripts/measure-workload` and the exclusive coordinator
lane, in a separate process. Each directory retains the exact command, binary
hash, CSV, stderr, elapsed CPU/time and child RSS record:

- `.research/benchmarks/linear-f32-photo24-01`
- `.research/benchmarks/linear-f32-photo48-01`
- `.research/benchmarks/linear-rgba8-photo24-01`
- `.research/benchmarks/linear-rgba8-photo48-01`
- `.research/benchmarks/linear-f32-photo48-refusal-01`

The summarized successful rows are also retained in
`.research/benchmarks/linear-working-output-01/summary.json`.

```sh
./scripts/measure-workload --label <new-label> --outer-seconds 660 -- --workload photo24 --samples 7 --warmups 1 --max-seconds 300 --output-format linear-f32
./scripts/measure-workload --label <new-label> --outer-seconds 660 -- --workload photo48 --samples 3 --warmups 1 --max-seconds 300 --render-mib 1024 --output-format linear-f32
```

Use `--output-format rgba8` for the corresponding encoded-output observations.
The same executable was built by one root-controlled `cargo build --locked
--release -p affint-render --example workloads -j2`, with
`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`. Native F32 runtime acceptance and
future cold/warm GPU evaluation have their own evidence; these CPU results do not
establish either.
