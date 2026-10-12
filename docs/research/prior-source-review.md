# ArtCraft source-review findings carried into AFFINT

Review performed 2026-10-12 with three Pi GPT-6.1 Sol workers and independent parent
source checks/reproductions. This is a sampled review, not a complete product audit.
Original /tmp checkouts and reproduction logs disappeared during the session; this
record preserves observed results and immutable source references. Do not represent
those lost artifacts as presently available. Recreate fixtures when a reuse decision
depends on them. New artifacts live persistently in AFFINT.

## PhotoCraft

Snapshot c450e755aed5e77fa6d4de7f04996085f5871085. Substantive COW raster storage,
staged edits, history, PSD/PSB, content-addressed persistence and CPU/GPU compositing.
Eleven history tests and thirteen copied atomic-write tests passed in focused checks.

- A warmed effect cache omitted Transparency Shapes Layer from identity. Changing
  the flag produced 36 differing pixels in an original 8x8 test versus a cache purge.
  [shape calculation](https://github.com/storytold/photocraft/blob/c450e755aed5e77fa6d4de7f04996085f5871085/crates/compose/src/lib.rs#L648),
  [cache identity](https://github.com/storytold/photocraft/blob/c450e755aed5e77fa6d4de7f04996085f5871085/crates/compose/src/lib.rs#L1336).
- A selection owning 65,536 bytes of tiles reported zero history pixel bytes.
  [tile-owner traversal](https://github.com/storytold/photocraft/blob/c450e755aed5e77fa6d4de7f04996085f5871085/crates/ops/src/tiles.rs#L14).
- Source tracing found warned empty surfaces for failed PSD channels, with no
  original channel payload retained for saving. Not reproduced end to end.
  [import](https://github.com/storytold/photocraft/blob/c450e755aed5e77fa6d4de7f04996085f5871085/crates/io/src/psd_import.rs#L113).

Lesson: complete semantic cache keys, comprehensive resource ownership accounting,
and a preservation/restricted-save policy for unsupported imported content.

## FilmCraft

Snapshot 156086dc87282026aeec33f8a220e4629bccbe31. Real Rust codecs, integer time,
staged edits, media caches and export machinery. An original 12-frame High-profile
H.264/B-frame fixture decoded byte-identically to FFmpeg; time assertions and three
atomic-write tests passed. The parent reran the reproduction harness successfully.

- A one-byte truncation produced a deferred error after decoder flush. The app
  adapter returns flushed pictures without collecting the decoder's take_error.
  [adapter](https://github.com/storytold/filmcraft/blob/156086dc87282026aeec33f8a220e4629bccbe31/crates/codecs/src/video.rs#L280).
- Loading an inconsistent current-schema project with next_id equal to an existing
  ID succeeded; adding an item replaced the prior map entry. Normal saves were not
  shown to generate this state.
  [allocation](https://github.com/storytold/filmcraft/blob/156086dc87282026aeec33f8a220e4629bccbe31/crates/project/src/lib.rs#L1359).
- Source tracing found decode errors converted to missing picture/silence before
  export. This was not reproduced in a complete export job.
  [video](https://github.com/storytold/filmcraft/blob/156086dc87282026aeec33f8a220e4629bccbe31/crates/render/src/lib.rs#L519),
  [audio](https://github.com/storytold/filmcraft/blob/156086dc87282026aeec33f8a220e4629bccbe31/crates/render/src/audio.rs#L207).

Lesson: fallible completion interfaces, export error propagation, allocator/identity
validation on import, and atomic output replacement.

## Other parent checks

- SoundCraft a10eb47032099491c32ac4eed17ce5fef03c03bf: 57 DSP tests passed. Recording
  callbacks grow Vec buffers; 4+ strips dispatch and join Rayon work. These are
  source-level real-time concerns, not measured dropouts.
- DeckCraft be14248577b92059eb2b8c0c9460cceffbc1f01d: an external engine harness
  confirmed dirty_before=true, document count 1->0, response=null on file.close.
  Source confirms the UI routes Close directly to that handler; window quit has a
  separate dirty-document prompt. No manual GUI reproduction was performed.
- RAWmakase 0f1bc320207658c9778922a4d6cf87bd174be2b2: 134 model tests passed, three
  ignored. Modular Rust with a C++ LibRaw/LittleCMS adapter; transactional catalog
  writes and Camera Raw mean/p95 DeltaE2000 regression comparisons are worth studying.
  Full color/GUI suites were not run; distance regressions do not prove Adobe parity.
- Tooscut 16c995bc371e9250d440ef599f1c3a826b35bb75: 59 audio/keyframe tests passed.
  Rust/WASM core with React/TypeScript and browser WebCodecs; not pure Rust. Its
  README lists Elastic License 2.0; component licensing requires separate auditing.
- Graphite e3bd661f5498bb824943a7d458aebb93bc9d245c: older procedural comparator,
  Rust engine plus Svelte UI. A rawkit default test build ran ZERO tests. No whole
  Graphite test suite or overall quality ranking was established.

All code-reuse decisions require new exact source/license checks. Existing UI/menu
coverage or self-estimated parity percentages were not treated as quality scores.
