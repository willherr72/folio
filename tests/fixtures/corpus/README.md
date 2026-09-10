# Folio synthetic compatibility corpus

This corpus contains only generated document text, vector shapes, and seeded synthetic scan pixels. It contains no private records, external PDFs, photographs, or downloaded page content. Synthetic document content is covered by the repository MIT license. Font software is covered separately by `fonts/LICENSE_DEJAVU.txt`.

`fonts/DejaVuSerif.ttf` is an unmodified font file copied from the DejaVu distribution bundled by the local Matplotlib installation. Its exact SHA-256 is recorded in `manifest.json`. The full accompanying redistribution notice is included. Upstream: https://dejavu-fonts.github.io/ ; license: https://dejavu-fonts.github.io/License.html . Embedding this bundled file makes regeneration independent of fonts installed on the host. Latin, Greek, Cyrillic, mathematical glyphs, embedded TrueType and Unicode extraction are covered; this is not a shaping/RTL conformance suite.

Regenerate the four versioned small fixtures with:

    python scripts/corpus-generate.py

Generate the additional 300-page table document and 40-page, 200dpi grayscale scan into ignored `artifacts/corpus` with:

    python scripts/corpus-generate.py --large

The manifest records generator/tool versions, fixed seed, dimensions, settings, expected text samples, sizes and SHA-256 checksums. PDF IDs and timestamps are fixed. Byte identity is tested on repeated generation with the same toolchain; a toolchain upgrade may change compression or PDF serialization and requires reviewing regenerated checksums. Large scan pages have separate seeded paper-noise pixels to prevent deduplication from producing an unrealistically tiny document. Text in scan pages is rasterized and has no hidden OCR layer.
