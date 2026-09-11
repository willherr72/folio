# Folio v0.8.0 verification

Verified on Windows on 2026-09-11. Issue #11 adds installed/imported fonts for added text boxes. [Supported formats and limits](font-support-roadmap.md).

- **187 frontend tests passed in 41 files.** Font selection, exact-byte loading, missing characters, undo/redo, shared tab/history ownership, late-load cleanup and discarded/unmounted recovery resources are covered alongside the full regression suite.
- **81 native tests passed; three existing opt-in checks remain ignored.** New tests cover static font styles, embedding permissions, unsupported formats, glyph coverage, immutable deduplication, 16/128 MiB and 64-font limits, native Windows catalog, editable/flattened PDF export and extraction, rotation, tampering, mixed text/ink order, resource-graph bounds, atomic failed-import cleanup, font recovery without original files, generation cleanup and obsolete source fonts at the 64-font recovery limit.
- **16 real-browser combinations passed.** Each text rotation was combined with each page rotation using actual DejaVu font bytes. Preview and thumbnail font families match, and selection extents follow browser glyph advances. No browser errors.
- **Production TypeScript/Vite/Tauri build, Rust formatting and diff checks passed.** The pinned ttf-parser 0.25.1 dependency and its notices are included in the portable distribution.

## Packaged desktop and independent readers

The isolated production app selected installed Arial Italic, exercised undo/redo, imported a local DejaVu Serif file, displayed a missing-emoji correction, and exported editable and flattened PDFs. It reopened the editable PDF after both original PDF and imported font were moved, restored the font and text controls, found an accented search term, and exposed the flattened text for selection. No WebView errors occurred. Native dialogs were submitted separately in the owned process; generated default-named save outputs were renamed to distinct fixture names before reopening.

Independent pypdf and PyMuPDF extraction both returned **Custom café Ω Ж** and unchanged base content. The embedded TrueType program is exactly 379,740 bytes, with SHA-256 matching the imported font. At 150% rendering, editable and flattened copies had **zero differing pixels** under the per-channel eight-level comparison threshold. The original PDF hash remained unchanged. The independent rendering was visually inspected.

[Machine-readable results](verification-v0.8.0.json) include font/output hashes and measurements. Tested and shipped executable SHA-256:

`14c57a54267d898e17ce3c52857af8ce053825edf2b13d9f7dbb3511dedb8468`

Independent review found and verified fixes for mixed text/ink flatten ordering, untrusted resource expansion, discarded recovery font ownership, and obsolete source fonts consuming recovery capacity. No unresolved review findings remain in this scope.

## Reproduction and limits

Run `npm test`, `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --tests --lib`, and `cargo fmt --manifest-path src-tauri/Cargo.toml --check`. Serve Vite on port 1431, then run `node scripts/smoke-custom-fonts-browser.mjs`.

For desktop reproduction, place an owned single-page `Original.pdf` and a copy of the bundled DejaVu Serif fixture named `Imported.ttf` in `artifacts/custom-font-desktop`, with no existing outputs. Launch the release app through `scripts/corpus-start-desktop.ps1` using an isolated profile and port 9246, then run `node scripts/smoke-custom-fonts-desktop.mjs` and submit its native dialogs. The harness moves the generated original/font and expects editable/flattened output names. Run `python -X utf8 scripts/verify-custom-fonts.py` with PyMuPDF and pypdf installed for independent inspection.

Static TrueType outlines and the documented unshaped character set are supported. Existing embedded/subset text editing, CFF, complex scripts, variable/color fonts and reflow remain separate work. No new performance parity, physical low-memory or real-printer claim is made.
