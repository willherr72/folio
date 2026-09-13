# Shared native text preview checkpoint

This #14 checkpoint prepares a PDF and vector preview from one native shaping
pass. It is behind the nondefault `shaped-text` feature and is not registered
as a Tauri command or connected to the desktop editor. The published release
remains v0.9.1.

`prepare_semantic_text` retains the PDF, positioned layout, and a versioned
serialize-only preview in one immutable result. Both visible representations
use the same validated outline callbacks, cubic conversion, and glyph positions.
Preview placement operands use the PDF writer's decimal precision. Each unique
glyph outline appears once in the preview; repeated glyphs reference it.
The preview contains exact logical text and the source font hash, while the
paired PDF retains the complete source font. Existing PDF-only APIs retain
their signatures. Serialized preview data is bounded to 8 MiB without allocating
a second serialized copy; existing shaping, outline and PDF budgets remain.

`ShapedTextPreview` renders native paths through SVG definitions/references,
with native color and explicit source-page rotation. Each mounted component
isolates its outline IDs. The browser does not shape or substitute fonts.
Unsupported preview versions show an unavailable cue. This component consumes
trusted native output; it is not a parser for arbitrary imported JSON.

## Measured results

The eight accepted short samples at four rotations produce 32 prepared pairs:
ligatures on/off, composed/decomposed accents, two-axis combining marks, Arabic,
Indic, and supplementary Unicode. At 150% and 300%, all **64 browser/PDFium
comparisons pass** with no browser errors. Maximum observed differences:

- Ink bounding boxes: **0 pixels**.
- Weighted ink centroid: **0.549 pixels** (rounded upward).
- Weighted ink area: **4.346%** (rounded upward).
- Ink unmatched within the declared two-pixel neighborhood: **0%**.

The fixed comparison tolerances are bounds 2 px, centroid 1 px, weighted area
20%, and at most 0.5% unmatched ink within 2 px. These check geometry across
different rasterizers; they do not assert pixel identity. Comparator unit tests
reject missing, displaced and materially altered ink. Representative Arabic,
Indic and rotated-mark side-by-side screenshots were visually inspected.
The retained [machine-readable report](shared-text-preview-verification.json)
contains all 64 measurements and the exact thresholds.

The same 32 PDFs pass the strict semantic inspector: exact original/reopened
Unicode in PDFium, MuPDF and pypdf; retained source fonts; cluster geometry
in tolerance; unchanged original/reopened PDFium and MuPDF rasters. The four
mixed-direction refusals remain enforced. Native tests also cover repeated
outline reuse, exact source identity/page dimensions and existing invalid-input
refusals. Component tests cover all rotations, scale, color, ID isolation and
unsupported versions.

## Reproduction

From the repository root, use an unused native output directory (the generator
refuses to overwrite evidence):

```powershell
cargo run -j 1 --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --example shaped-text-probe -- --semantic-preview
python -B -X utf8 scripts/inspect-semantic-native.py --input artifacts/shaped-text/shared-preview-native --output artifacts/shaped-text/shared-preview-inspection.json --summary artifacts/shaped-text/shared-preview-inspection-summary.json --check-invariants
```

Start Vite on port 1427, then run the browser harness in another terminal:

```powershell
npx vite --host 127.0.0.1 --port 1427 --strictPort
node scripts/smoke-shaped-text-preview-browser.mjs
node --test scripts/preview-ink-comparison.test.mjs
```

Native artifacts include preview JSON and two PDFium PNGs per pair. The browser
harness writes 64 browser PNGs, 64 side-by-side PNGs and `results.json` beneath
`artifacts/shaped-text/shared-preview-browser`.

## Remaining integration

Actual editable text boxes still need a versioned schema, asynchronous prepared
result/font ownership, input and IME handling, cluster-aware caret/deletion,
undo/redo, recovery, duplicate-tab and canceled-export coverage. The preview
page currently includes research margins and a fixed ink color; application
placement/color must be an explicit shared native contract before wiring it in.
Mixed-direction text, RTL separators/joiners and more than 255 distinct semantic
definitions remain refused. This checkpoint does not establish universal script
support, portable caret geometry or a real Acrobat/Foxit manual reader check.

## Regression validation

Full native `cargo test -j 1 --offline --locked --features shaped-text`:
127 passed, 3 existing ignored, no failures. Frontend: 222 passed across 45
files with `vitest run --maxWorkers=2 --minWorkers=1`; the initial default
parallel run timed out one existing text-edit test at five seconds. That test
passed alone in 1.96 seconds, then the complete bounded-worker suite passed
without code or timeout changes. Production TypeScript/Vite build, three ink
comparator tests, Rust formatting and whitespace checks passed. Independent
review of the native prepared result found no blocking issues.
