# Contributing

AFFINT is an early native Rust editor. Consult the capability table in
[the roadmap](docs/roadmap.md), [the architecture](docs/architecture.md), and
[the engineering contract](AGENTS.md) before proposing a feature.

Use the repository's pinned Rust 1.98.1 toolchain. Workspace manifests declare
Rust 1.95; lower declared per-crate/minimum versions have not been qualified.
Keep `Cargo.lock` and MUI's git revision pinned. Build on Linux first; Windows and
macOS are portability goals, not validated release targets.

Run one coordinated check after a meaningful batch:

```sh
scripts/verify --label my-change --build-app --jobs 2
```

Original assets stay immutable. Submit document edits through validated atomic
transactions; continuous gestures become one undoable edit. Preserve failed-save
behavior, source precision and requested output dimensions. Test behavior through
the public interface against independent uncached/reference output. Cache identities
must include all semantic dependencies, and resource leases must outlive their data.

Use proven crates where appropriate, preserve attribution, and justify new
interfaces with real behavior. Benchmark release workloads before asserting a
speed improvement. Keep UI input free of decode/export/file work and show errors
truthfully. Unimplemented controls must not pretend to perform an operation.

Submit only original or license-compatible material. Do not contribute proprietary
decompiler output, vendor binaries, extracted artwork, credentials or personal
documents. Research specifications must distinguish verified behavior, static
observations and inference. The private `.research/` evidence store is excluded
from the public release.
