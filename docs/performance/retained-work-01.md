# Retained work: exact-output observations

2026-10-12. The coordinator measured the retained CPU renderer after the combined
renderer/camera increment passed 153 tests plus a doctest, formatting, all-target
Clippy and native build. These are bounded observations on a Ryzen 7 7800X3D,
48 GB Linux machine with Rust 1.98.1. The RX 6600 is used by separate native proofs,
not these CPU workloads.

## Release workloads

Each workload has one exact warmup and seven timed samples. A newly constructed
uncached renderer supplies a bit-exact F32 oracle outside **every** timed sample.
Setup, transactions, oracle evaluation and frame drops are excluded. Retention is
bounded to 128 MiB inside a shared 256 MiB renderer allowance. No output resolution,
coverage policy or original asset is reduced.

| Workload | Exact output | p50 / observed p95 ms | Reused / evaluated tiles per sample | Child peak RSS KiB |
| --- | --- | --- | --- | --- |
| Move one element among 64 curves | 512×512 F32 | 12.186 / 15.452 | 252 / 4 | 110044 |
| Pan 256 pixels over a 6000×4000 original | 1024×768 F32 viewport | 7.705 / 9.886 | 9 / 3 | 157812 |

Every curve sample reuses 63 prepared geometries and prepares the one changed
curve. The photo's original 96,000,000-byte allocation is retained and charged
once. These seven photo positions are from the first sweep over a thirteen-position
path, so the reported hits are not repeats of the same viewport. Both workloads
pass all fresh-reference comparisons and release all leases after clearing/dropping
owners. No retention refusal occurred.

Curve output is 4,194,304 bytes; cache residency grows from 87,730,704 to 94,178,928
bytes. Photo viewport output is 12,582,912 bytes; cache residency grows from
111,961,512 to 130,839,480 bytes. The CSV separates original capacity, retained
pixels, conservative metadata and total renderer charges. These overlap document
source admission and are not a global process memory cap. Child RSS covers the
entire process and may include a fork/exec launcher floor.

The photo result is **not a full 24 MP output benchmark**. Earlier full-output
measurements remain separate. These timings exclude GPU upload, conversion,
presentation and native input. Seven samples make observed p95 the sample maximum;
background work was uncontrolled. No paired baseline, general latency improvement
or fastest-editor comparison is established.

## Native and GPU scope

A separate real RX 6600/Vulkan proof uses a 192×168 F32 source window from an
original raster with exposure/alpha and an integer 8×6 pan into a 64×40 viewport.
Its retained crop agrees bitwise with a fresh raster reference; final page/checker
composition agrees within one encoded channel value. The actual same-scene MUI
selection marker, truthful original evaluation key, private publication and
completion-aware resource retirement are exercised. This does not assert arbitrary
curve antialiasing equivalence between independently shifted render requests.

An isolated X11 native probe at recorded scale factor 1 and physical/logical
1024×720 exits normally after protected discard, retaining 933 bounded samples
with zero drops. A drawn rectangle appears, Undo removes it and Redo restores it.
During 189 held-drag presentations, submitted render count stays 16; during 82
chrome-hover presentations, it stays 17 and GPU conversion count stays 6. The pan
includes native-density retained presentations; some fractional/coverage changes
remain pending CPU work. Both pan and zoom settle with sampled serial equal to
desired serial and no pending work. Camera movement leaves the document revision
unchanged. These are optimized-development CPU wall observations, not optical
latency or release-native performance measurements.

Dense final output writes and native upload remain. Exact cross-camera tile reuse
is deliberately limited; unsupported mappings miss rather than reduce fidelity.
See [ADR 0010](../adr/0010-retained-exact-cpu-work.md) for cache identity, proof
domains and ownership.

## Reproduce

Run each command serially in the shared verification lane:

```sh
CARGO_TARGET_DIR=target cargo build --locked --release -p affint-render --example retained_workloads -j 2
scripts/measure-workload --label curves-local --example retained_workloads --outer-seconds 180 -- --workload curves64 --samples 7 --output-format linear-f32
scripts/measure-workload --label photo-local --example retained_workloads --outer-seconds 180 -- --workload photo24-pan --samples 7 --output-format linear-f32
```

The measured executable SHA256 is
`87372d93708a1b828cd97197f71d2ed46e0e75b1a885f107cfdf986cccacf882`.
[Build/source identity](evidence/retained-build.json), [curve samples](evidence/curves64.csv)
and [photo samples](evidence/photo24-pan.csv) are distributed with this report.
The complete private command/runtime records remain outside the public release.
