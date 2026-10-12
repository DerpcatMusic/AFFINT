# Bounded native editable persistence

Date: 2026-10-12. Status: implemented and coordinator-verified.

Saving a live workflow must retain original pixels, relationships, transforms,
masks, ordered operations, stable identities, and the irreversible allocator
watermark. It must not replace the last valid file with incomplete output, hide
corruption, or let a completed save of older artwork mark newer edits clean.

`affint-storage::Store` owns native encoding, decoding, and file publication. It
accepts immutable document snapshots and returns a validated session on load.
There are no UI, GPU, external document decoder, or vendor implementation types.
Native schema 1 is original AFFINT framing, not an Affinity interchange claim.

The format consists of a fixed 56-byte header, a bounded UTF-8 JSON manifest,
and contiguous exact RGBA8 source blobs. The header is eight bytes `AFFINT01`,
little-endian u64 manifest length, little-endian u64 exact file length, and the
manifest's 32-byte SHA-256 digest. The manifest includes format/document versions,
the allocator watermark, pages, layers, source-ID references, and blob descriptors
with validated dimensions and SHA-256 digests. Source blobs follow descriptor
order, with lengths derived from checked dimensions. There are no supplied paths,
extracted files, compression bombs, or binary sequence-length allocation hints.
Unknown fields/variants/schema, trailing bytes, missing/unreferenced blobs,
length disagreement, and checksum failure are refused.

Existing crates handle the substantial primitives: Serde/serde_json encode the
editable records, sha2 hashes bytes, and tempfile creates/publishes staging files.
A ZIP container was considered, but its general central directory, archive paths,
compression, and parser allocation policy add machinery this first flat format
does not need. Postcard is compact and supports streaming writes, but accepting
arbitrary binary sequence-length hints needs custom bounded decoding before Serde
allocates. JSON's byte cap is a simple first admission boundary for metadata; raw
image bytes remain binary. Compression and chunked large-document storage should
follow measurements and recovery requirements rather than introduce an early
custom compression implementation.

Save streams borrowed original source slices. Blobs with equal SHA-256, dimensions,
and exact bytes share one encoded source. Load reconstructs one immutable source
allocation per blob and shares it under all referenced asset identities. Equal
bytes with different image dimensions retain separate geometry. Pixel corruption
is checked before publishing a session; document validation then checks the entire
editable graph. The format never flattens unsupported content into a successful save.

Default file admission is 600 MiB total, 8 MiB manifest, and 512 MiB distinct source
pixels. Lengths and source totals are checked before application pixel allocation;
application byte buffers use `try_reserve_exact`. JSON keeps its normal recursion
limit. Metadata allocation in Serde and document tables is bounded by serialized
bytes/count limits but remains infallible internally; these admission bounds are
not a process RSS or system-wide OOM guarantee. Document, renderer, loaded-file,
and GPU budgets still need coordinated application ownership. Larger documents
are explicitly refused under this policy, never silently resized.

Publication uses a same-directory NamedTempFile, complete encoding, flush,
file `sync_all`, atomic replacement, then directory synchronization on Unix.
Any error before replacement leaves the destination intact. A failure after
replacement cannot truthfully be reported as an untouched destination: SaveReceipt
reports `PublishedUnconfirmed` with the actual directory error. Other platforms
currently report unverified directory durability. SaveReceipt also carries the
exact saved revision; the app may mark clean only when it matches current artwork.
Atomic rename/fsync behavior still depends on the actual filesystem and OS.

Verified by `scripts/verify --package affint-storage --label native-storage-01`:
formatting, ten meaningful tests, and all-target Clippy with warnings denied.
Tests exercise editable persistence, source sharing/dimensions, pixel/manifest
corruption, truncation/appends, forged giant lengths, unsupported fields/formats,
older saved revisions, byte-limit failure, interrupted write, and failed rename.

Limitations: schema 1 does not save session history, alternate futures, external
unknown content, color profiles, float sources, encryption, recovery journals, or
Affinity/PSD/PDF files. It is a native persistence seam for the implemented subset.
No external fidelity or performance claim follows from self-round-trip tests.

Source audits: [tempfile 3.27](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html)
documents replacement and missing implicit synchronization;
[serde_json 1.0.154](https://docs.rs/serde_json/1.0.154/serde_json/) and
[sha2 0.11](https://docs.rs/sha2/0.11.0/sha2/) are MIT OR Apache-2.0 registry releases.
The lockfile pins the actual dependency graph; transitive audit remains ongoing.
