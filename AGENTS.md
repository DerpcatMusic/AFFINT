# AFFINT engineering contract

Read PRODUCT.md, CONTEXT.md, docs/architecture.md, and docs/work-log.md before work.
The user authorized ongoing implementation of a unified native Rust vector, photo,
and layout editor. Follow the user's current instructions over this file.

## Product invariants

- Source assets are immutable. Cropping, transforms, masks, vector booleans,
  adjustments, filters, typography, and layout remain editable operations.
- Every document mutation crosses the document module's transaction interface.
  Validate a staged change before publishing it; errors and cancellation leave
  the prior document and history intact. Gestures commit as one undoable edit.
- Undo/redo preserves identity and content; identifiers are never reused after
  branching history. Reordering or reparenting cannot introduce a graph cycle.
- Saving replaces the destination atomically after complete serialization and
  validation. Failed writes preserve the original. Report durability limits.
- Import fidelity is separate from render fidelity. Unsupported content must be
  retained opaquely, refused, or opened with an explicit restricted-save policy.
  Never silently discard unknown content or undecodable pixels on save/export.
- Render at the requested output resolution. No hidden preview decimation,
  flattening, resolution reduction, or degraded output advertised as exact.
  Display sampling at a zoom level is separate from destructive source reduction.
- CPU reference and accelerated results have explicit error tolerances. Cache keys
  include every semantic input, and cached/uncached output must agree.
- No fake menu coverage: enabled controls execute real behavior. Unimplemented
  features stay absent or explicitly unavailable with a truthful reason.

## Rust and module design

- Use the local rust-best-practices, codebase-design, domain-modeling, and applicable
  UI skills. Read relevant references, but verify technical claims against Rust
  documentation; skill prose is not a substitute for correct Send/Sync semantics.
- Design deep modules with small interfaces, explicit invariants/error modes,
  and behavior tests through the same interface used by real callers.
- Keep document, rendering, persistence, and UI knowledge local to their modules.
  The document module must not depend on the UI, filesystem, or GPU framework.
- Prefer proven crates for geometry, text shaping, image codecs, color management,
  GPU access, and persistence. Record a reuse decision before reinventing a solved
  subsystem. Audit license, provenance, maintenance, fidelity, platform support,
  and integration cost before importing code; preserve upstream attribution.
- Borrow by default. Use immutable shared assets and deliberate copy-on-write
  structure; document ownership and bounded lifetimes. Do not clone whole pixel
  buffers for edits, undo, repaint, or worker handoff.
- Encode identities, validated dimensions, finite transforms, and revision tokens
  in types. Keep unchecked constructors private. Validate external input once at
  a meaningful seam, and preserve invariants internally.
- Library errors use structured types (thiserror where useful). Propagate errors;
  never erase an export/import/worker failure with .ok(), a silent fallback, or
  default pixels. Any fallback must be an explicit documented product policy.
- No unwrap/expect/panic/todo/unimplemented in production paths. Application-owned
  core code forbids unsafe; isolate any genuinely required platform FFI, explain
  safety invariants, and review it separately. Dependencies may legitimately use
  audited unsafe internally.
- Cap allocation by bytes, not just item counts. Account for every pixel owner,
  assets, history, cache residency, staging, and outstanding worker jobs. Bound
  recursion; use checked arithmetic for input-derived lengths and coordinates.
- Choose static or dynamic dispatch from the actual seam and measurement; avoid
  unnecessary abstraction, hypothetical backends, giant modules, and pass-through
  wrapper crates. Document tradeoffs instead of imposing slogan-based rules.
- Rustfmt, meaningful clippy lints, rustdoc on public interfaces, and a locked
  dependency graph are required. Lint exceptions need a narrow justification.

## Research and provenance

- The owner explicitly authorized deep binary reverse engineering/decompilation
  of the installed Affinity app in addition to public/black-box research. Assigned
  Pi reverse-engineering workers may inspect owned installed binaries with REA,
  Ghidra and managed decompilers. Keep raw vendor output exclusively under the
  ignored .research/vendor-affinity area; do not embed it in product source/docs.
- Preserve separation of research and implementation: reverse-engineering workers
  publish independently worded behavior/interface/data-flow specifications with
  evidence and unknowns. Implementors do not read raw vendor pseudocode/source.
  Open-source reuse still requires a source/license audit and attribution.
  Do not describe copied proprietary implementation as clean-room code.
- Research workers write behavior specs, evidence, and unresolved questions.
  Implementors consume those specs and audited open-source references.
- The owner explicitly forbids opening or manipulating their installed Affinity
  windows. Continue Affinity research from owned binaries, disassembly, decompilation,
  resources and saved UI preferences only; do not launch, focus, capture, send input
  to, or close Affinity windows. Existing validated captures remain historical
  corroboration. Root manages AFFINT's own authorized verification separately.
- Separate desired Affinity/Photoshop/Illustrator behavior from independently
  verified behavior. Do not claim a feature is absent from Affinity without evidence.
- Pin upstream snapshots and record exact paths/commits for reused code. Original
  icons and assets must have clear licenses; no copied vendor branding/artwork.

## Performance and verification

- Measure representative release workloads on recorded hardware. Report p50/p95,
  peak resident bytes, uploaded/read-back bytes, cache hit behavior, and image size.
  A fast UI library is not proof of a fast imaging pipeline or world leadership.
- Optimize measured hot paths: incremental dependencies, dirty regions, GPU
  residency, batching, and bounded background work before unsafe/SIMD complexity.
- UI rendering/input must not block on decode, export, file I/O, or heavy imaging.
  Worker results carry document revisions and obsolete results are discarded.
- Verify meaningful invariants, undo/redo/branching, failed transactions, corruption,
  save failure, cache invalidation, external format oracles, and full-resolution
  CPU/GPU agreement. Tests must catch plausible bugs, not mirror implementation.
- Never accept a self-round-trip alone as external file compatibility.

## Worker ownership and single build coordinator

- Pi researchers: openai/gpt-6.1-sol, thinking=xhigh. Codex implementors:
  gpt-6.1-sol, reasoningEffort=xhigh. No nested spawning without coordinator approval.
- Each worker owns named files/modules. Re-read shared files before edits, and do
  not change another worker's files or delete shared artifacts.
- Only the root build coordinator may compile, check, run tests, or benchmark.
  Workers do not invoke cargo/rustc/build tools. Request verification with a
  concrete command and expected evidence instead.
- Exactly one build/check/test/benchmark process at a time for AFFINT. Use the
  shared scripts/verify entrypoint and its exclusive lock. Batch changes into a
  meaningful increment before verification; do not compile after each small edit.
- After each major increment, run a bounded improvement pass for correctness, ownership,
  fidelity, measured bottlenecks, and architecture. Implement justified improvements;
  do not manufacture speculative rewrites or claim speedups without measurements.
- Continue autonomously until the owner explicitly says stop; do not mark the
  product complete merely because a milestone builds.
- Keep research and evidence under this repository or its configured persistent
  artifact directory. Never put unique evidence only in /tmp or remove others'
  targets/research. Do not install or change unrelated user software/configuration.
- Record landed behavior, checks, unresolved defects, and next concrete work in
  docs/work-log.md. Maintain an honest capability matrix and continue to the next
  increment after a passing milestone; a scaffold is not the complete product.
