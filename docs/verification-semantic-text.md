# Semantic text development verification, 2026-09-11

This is the initial semantic serializer milestone. See the subsequent
[long-text verification](semantic-long-native-verification.md) for character
definition reuse, the RTL word-order regression, and current limits.

This continues issue #14 behind the nondefault `shaped-text` feature. It does
not enable complex-script input in the editor or create a release. The default
engine change combines adjacent PDFium UTF-16 surrogate entries into one
selectable Unicode scalar, retaining the union of their displayed bounds.

## Checks

| Check | Result |
| --- | --- |
| Frontend Vitest suite, 43 files | 209 passed |
| TypeScript and Vite production build | Passed |
| Rust tests and library with default features | 93 passed, 3 existing ignored |
| Rust tests and library with `shaped-text` | 115 passed, 3 existing ignored |
| Full Unicode 16 `BidiCharacterTest` and Unicode 17 `GraphemeBreakTest` | 91,707 and 766 rows passed |
| Native semantic PDF inspection | 32/32 positive cases and 4/4 mixed-direction refusals |
| Rust formatting and Git whitespace checks | Passed |
| Vendored dependency inventory, working tree and staged Git bytes | 85 upstream files, 2 documented changes verified |
| Hostile-font provenance | Source plus all 4 generated font hashes verified |
| New Python development scripts | All 6 parse |
| Default dependency graph | HarfRust, read-fonts and font-types absent |

The feature suite includes four semantic serialization tests, thirteen shaping
tests, and the default engine's supplementary-character regression at all four
page rotations. The Unicode environment variable points to the complete,
checksum-verified character corpus rather than the tracked bidi sample.
The three existing ignored tests require separate benchmark or native-process
setups; this run does not claim those measurements.

Reproduce from the repository root:

```powershell
npm test -- --maxWorkers=2 --minWorkers=1
npm run build
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --tests --lib
$env:FOLIO_FULL_BIDI_TEST = '<path to BidiCharacterTest-16.0.0.txt>'
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --tests --lib
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
python scripts/verify-shaping-vendor.py
cargo run --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --example shaped-text-probe -- --semantic
python scripts/inspect-semantic-native.py --check-invariants
```

The example requires a fresh output directory. The inspector requires PyMuPDF
1.27.1 and pypdf 6.14.2. Resource fixture regeneration uses fontTools 4.62.1.
Python tools are development probes, not runtime application dependencies.

## Scope and remaining work

[Native reader evidence](semantic-native-verification.md) retains every raw
reader string, font/PDF/raster hash and maximum geometry difference. Exact
Unicode passes all three measured readers before and after native export.
PDFium cluster bounds agree within 0.012010 points for the 24-point fixtures.
MuPDF still partitions cluster selection horizontally; mixed-direction text is
refused, and the semantic font is capped at 255 scalars. Original font bytes
are retained as an unused resource; visible text is vector outlines, which do
not preserve TrueType hinting or establish existing-run editability.

[Resource checks](shaping-resource-limits.md) describe the partial-shaping
failures and their explicit rejection, the pinned two-file HarfRust patch,
and bounded composite-outline validation. These are algorithmic limits, not
a hard process-memory quota or wall-clock deadline.

No new packaged desktop smoke or manual Acrobat/Foxit copy/search check was
performed in this development increment. Production preview, IME, cluster
selection/search, undo/recovery and font ownership integration remain open in
issue #14. Version and launcher remain at v0.9.0.
