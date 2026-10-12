# Initial PNG pixel interchange

The first PNG importer preserves supported samples as full-resolution straight
RGBA8 originals. RGB, grayscale, grayscale-alpha, and expanded palette/lower-bit
samples normalize channels without reducing image dimensions. Sixteen-bit sources
are refused until the document can retain their original precision. There is no
embedded-preview or thumbnail fallback.

Explicit sRGB tags establish sRGB. Untagged samples return `SourceColor::AssumedSrgb`;
the caller must make that interpretation visible. ICC, EXIF/orientation, animated
PNG, HDR/CICP, and gamma/chromaticity tags without sRGB currently require a policy
the importer cannot faithfully provide, so they are refused. This subset does not
claim profile conversion, metadata-preserving file interchange, or managed display.

The maintained png 0.18.1 decoder validates the entire stream through `finish`,
including chunks after pixel delivery. Decoding uses a capped file length, original
pixel count, and the crate's allocation budget. Application-owned output reserves
exact RGBA8 capacity first; RGB/grayscale expansion runs backwards within that
allocation instead of creating a second full image. Limits cause refusal, never
resizing or quantization. Decoder allocations and source/output owners still need
the app's overall admission policy.

PNG export borrows exact-sized straight RGBA8 pixels, writes an explicit sRGB tag,
and checks writer finalization before atomic file publication. The rendering owner
retains its output budget lease throughout export. File synchronization and directory
durability follow the native storage publication policy; a published-but-unsynced
directory is reported separately from a failed write.

Verification `png-io-first` passed formatting, all fourteen storage/PNG tests and
all-target Clippy with warnings denied. PNG tests use original independent Pillow
12.3.0 fixtures with recorded sample bytes: RGB, RGBA including colored zero-alpha
pixels, grayscale-alpha, and sixteen-bit refusal. They catch excess-pixel fallback,
deferred IEND checksum corruption, exact output/tagging, and invalid-export preservation.
Self-export round-trip is not a complete external-codec or color-management oracle.
Native app proof will add actual full-workflow export and independent decoder checks.

Primary source: [png 0.18.1 decoder](https://docs.rs/png/0.18.1/png/struct.Decoder.html)
and [Reader](https://docs.rs/png/0.18.1/png/struct.Reader.html); imported samples are
not passed through a generic color-converting JPEG adapter. Copyright/license
notices remain those of the locked Rust PNG ecosystem dependencies.
