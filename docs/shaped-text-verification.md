# Issue #14 native milestone — September 11, 2026

**Native/API spike completed; production portability gate failed.** This is a
development milestone, not a release or a claim of complex-script support in the
editor. The `shaped-text` Cargo feature is off by default. v0.9.0 remains the
current user-facing release.

## Implemented and measured

HarfRust shapes exact validated static TrueType font bytes into versioned visual
runs, positioned glyphs, UTF-8/UTF-16 cluster ranges, grapheme boundaries and ink
bounds. Logical input remains unchanged. Ligatures, accents, two-axis marks,
Arabic, mixed bidi, Devanagari and supplementary glyphs have native tests.
Missing glyphs, formatting controls, invalid assets and excessive geometry fail
explicitly. [Dependency decisions and remaining resource limitations](shaped-text-dependencies.md)
describe the admitted policy and upstream operation-budget limitation.

Two serializers were exercised: the actual PDFium CID/font/position/mark APIs,
and a standalone allocated-CID PDF using absolute glyph matrices and logical
ActualText. Sharing an existing mark across adjacent PDFium objects produces
one replacement span; independently marking each object duplicates copied text.

The native example generated nine cases at four rotations, with an original and
Folio-exported PDF for each. All **36** retain exact embedded font bytes and
intended glyph IDs/positions. Maximum serialized position error is **0.00001
point**. All 36 preserve PDFium and MuPDF rendering after export. A separate
native regression checks actual raster ink against native outline bounds at
every quarter-turn, within three pixels at 1,000-pixel render width; a blank
image cannot pass. It also pins the known incorrect selection geometry.

PDFium extracts the exact logical source in all 36 cases before and after export.
MuPDF and pypdf each succeed in **24/36**, with different failures. Arabic/mixed
text can be reversed or reordered, and Indic/marks can duplicate. Whole-line
ActualText collapses PDFium character boxes onto the first glyph. Changing to
logical cluster spans fixes some reader cases while breaking others.
[Raw result summary](shaped-text-verification.json) retains original and resaved
strings; [the interoperability report](shaped-text-interop.md) explains the
controls, versions, failed alternatives and scope.

## Verification

- Frontend: **209 passed / 43 files** with two workers. The first unrestricted
  run alongside a build hit three five-second timeouts and one recovery assertion
  in the same timed-out suite. The three affected files passed all 12 tests in
  isolation; the complete bounded-worker rerun passed. No frontend code changed.
- Frontend typecheck/production build: passed.
- Native with `shaped-text`: **105 passed, 0 failed, 3 pre-existing ignored**.
- Default native build: **92 passed, 0 failed, 3 pre-existing ignored**; the
  experimental test targets are omitted without the feature.
- Unicode: all **91,707** rows of pinned Unicode 16 `BidiCharacterTest.txt` and
  all **766** rows of Unicode 17 `GraphemeBreakTest.txt` passed. The checked-in
  bidi regression sample has 286 rows; the full source is a SHA-verified
  development artifact. This does not claim the separate class-based BidiTest,
  every UAX rule, or uniform Unicode 17 support across dependencies.
- Fixture integrity: all eight font/license/data hashes verified. Git attributes
  preserve upstream bytes across Windows checkouts.
- Independent inspector: **36/36** font/glyph/matrix/export-preservation checks
  pass. An intentionally altered resaved raster is rejected even when original
  glyph mappings and embedded font identity still match.
- Rust formatting and diff whitespace: passed. Independent review found and
  resolved an unchecked extreme glyph-ink bound and the inspector's missing
  export-preservation exit checks.

## Reproduce from the repository root

```powershell
python scripts/fetch-shaped-text-fixtures.py
npm test -- --maxWorkers=2 --minWorkers=1
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml --tests --lib
cargo test --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --tests --lib
cargo run --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --example shaped-text-probe
python scripts/inspect-shaped-native.py --check-invariants
```

Use the pinned Python reader versions in the interoperability report. The native
example refuses to overwrite an existing evidence directory; pass a fresh path
after `--`, and use `--input`/`--output` on the inspector for a subsequent run.
Set `FOLIO_FULL_BIDI_TEST` to the full versioned source to run the full bidi
character corpus; the runner verifies its SHA before reading cases.

## Work remaining before editor integration

1. Establish a representation that preserves logical copy and useful cluster
   selection geometry in PDFium and an independent reader. Preserve these
   failing cases as controls; do not normalize or reverse extracted text to
   manufacture agreement. Grouped positioning, semantic mappings and any
   reader-specific limitations need measured evidence before choosing a path.
2. Resolve detectable shaping-budget exhaustion and add hostile OpenType lookup
   fixtures. Output limits alone do not prove every lookup completed.
3. Integrate one native layout for preview/export, versioned annotations,
   cluster selection/search, IME/grapheme editing, undo/recovery, cancellation
   and font ownership. Verify zooms and real-reader manual copy/search.

Issue #14 remains open. Issues #15 (positioned existing text) and #16 (bounded
wrapping) remain separate work.
