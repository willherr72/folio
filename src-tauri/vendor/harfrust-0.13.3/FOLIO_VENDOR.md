# Folio's HarfRust resource-status patch

This is the exact crates.io HarfRust **0.13.3** package with a small local patch to `src/hb/buffer.rs` and `src/hb/face.rs`. It remains MIT licensed; retain the upstream `LICENSE`. No external contribution or upstream acceptance is implied.

`FOLIO_PROVENANCE.json` records the crates.io archive URL/SHA-256, upstream git revision, SHA-256 of every original file, and before/after hashes of the two changed files. The registry's `.cargo-ok` cache marker is omitted. `FOLIO_PATCH.patch` is the complete unified source diff. All other upstream files are unchanged.

The patch adds `GlyphBuffer::is_successful()` and optional `ShapeOptions::max_glyphs` / `max_operations` setters. Both options default to `None`, preserving upstream limits. Explicit glyph limits apply to intermediate buffer lengths even if capacity was reserved earlier. Zero/negative operation budget is retained as failure before the original `leave()` resets counters. Recursion-limit failure already set upstream's private flag and is now observable.

This exposes limits already present in the engine; it changes no shaping algorithm, Unicode data, default features, or glyph mapping. Folio uses the API only through its nondefault `shaped-text` feature, rejecting every unsuccessful buffer before interpreting glyphs.

The caller's glyph limit constrains intermediate initialized glyph slots, not all process allocations, font caches, or allocator capacity. Operation counts are the shaper's instrumented work units, not a hard CPU-time deadline. Rust allocation failures outside these guards retain Rust's normal behavior.
