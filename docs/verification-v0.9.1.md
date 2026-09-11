# Folio v0.9.1 verification

Verified on Windows x64 on 2026-09-11 with bundled PDFium chromium/8044.
This release integrates shared character geometry into Folio's existing reader.
It does not enable the experimental complex-text writer in the desktop editor.

- **215 frontend tests passed in 44 files.** New regressions cover coincident
  ligature boxes, partial ligature copying, both halves of a supplementary Unicode
  scalar, unnormalized combining marks, original highlight geometry, boundaries
  between distinct rectangles, and copy selection scoped to one document.
  Existing page-text measurement/cache tests remain passing.
- **24 real-browser cases passed.** Native PDFium specimens at all four source
  rotations and both 150% and 300% zoom preserve exact copied text and original
  highlight unions. Each ligature case also copies and highlights a partial
  selection. [Machine-readable results](verification-v0.9.1-browser.json) are
  retained alongside this report.
- **93 native tests passed; three existing opt-in checks remain ignored.** The
  default engine now preserves PDFium surrogate pairs as complete selectable
  Unicode scalars with unioned geometry, including rotated pages.
- **Production TypeScript/Vite/Tauri build and Rust formatting passed.** The
  portable package includes notices for 546 dependency packages. License
  collection ran from the primary checkout because npm traverses the worktree's
  node_modules junction incorrectly; production dependencies are unchanged.

## Geometry and limits

Only consecutive visible characters whose four rectangle edges coincide within
0.02 page points are grouped. Every comparison uses the first original rectangle
to prevent cumulative drift. The reader preserves source Unicode, source order,
and original character indices. Copying part of a ligature preserves that exact
substring; highlighting covers its whole shared source rectangle. Copy offsets
cannot split a UTF-16 surrogate pair. This is a shared selection target, not
inferred font shaping or exact internal ligature caret positions.

The retained native specimens cover `office`, `q` with two combining marks, and
a supplementary mathematical letter. Their source PDF, font, raster, layout,
and PDFium hashes are recorded in
[the fixture provenance](../tests/fixtures/reader-selection/README.md).
Mixed-direction text creation, RTL word ordering, broad script coverage, and
portable cluster geometry in other readers remain under #14. No new Foxit,
Acrobat, physical-printer, or hardware-performance claim is made.

## Packaged desktop check limitation

The production executable launched successfully in a fresh, isolated test
profile and exposed its WebView. Its hidden Windows Open dialog could not be
submitted by the existing UI Automation helper or scoped Win32 attempts. The
app remained responsive. Consequently, this run did **not** execute the planned
packaged open/select/copy/highlight/save/reopen assertions. The browser matrix
and native regression suite are separate passing checks, not substitutes for a
completed packaged workflow. No production dialog behavior was changed to
accommodate the harness. The previous release's packaged workflow evidence
remains in [v0.9.0 verification](verification-v0.9.0.md).

## Reproduction

Run `npm test`, `npm run build`,
`cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --tests --lib`,
and `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
Serve Vite, then run `node tests/reader-selection.browser.mjs`.
The browser harness verifies retained raster/layout hashes and exercises real
mouse selection and the clipboard at 150% and 300% across all four rotations.

Tested executable SHA-256:

`963f7388805524b537d08fac833b0edf3c02a410093180e5f571bc56bf65b5a7`

Bundled PDFium DLL SHA-256:

`04100c03e41cac1f979e36e5e26fb860bcb5a7461f53830d3c098716624a27a9`
