# Native reader selection evidence

`native.json` contains unchanged `originalGeometry` records from
`artifacts/shaped-text/semantic-native-reused-codes/results.json` (SHA-256
`8eadd2acf34ab31a48c31add08b07ee193c90b689898286c7558630ff00d947c`).
The selected cases are `ligatures-on` (`office`), `two-axis-marks`
(`q\u0307\u0323`), and `supplementary` (`A 𝐴 B`), each at native page rotations
0, 90, 180, and 270. These are real PDFium extractions of the semantic native
outline prototype, not manually constructed character boxes. No geometry or
Unicode normalization was applied. Page dimensions come from the PDF MediaBox
`[0 0 300 160]`, swapped for 90/270 degrees.

The adjacent PNG and layout JSON files are byte-for-byte copies of the native
probe artifacts. `native.json` records each source PDF, raster, layout, and font
SHA-256 plus the PDFium DLL SHA-256. Source PDFs and fonts are deliberately not
duplicated. The browser test verifies the retained PNG/layout hashes before use.
The PDF generator and native extraction provenance are documented in
`docs/semantic-native-verification.md` and `docs/semantic-short-reused-verification.json`.

Run a Vite server from the repository root (`npm run dev`), then run
`node tests/reader-selection.browser.mjs`. Override the origin with
`FOLIO_READER_ORIGIN` when needed. The test uses the production `PdfTextLayer`
over the native raster, a real Chromium mouse drag and clipboard keyboard
shortcut, at 150% and 300% zoom for every retained source rotation. It checks
shared-box cluster copy, a partial `ff` selection, supplementary scalar copy,
and highlight rectangles against the union of original PDFium character boxes.
Results are written to `artifacts/reader-selection/browser.json`.

This is bounded reader integration evidence. It does not establish arbitrary
font support, all-script editing support, or a native writer release gate.
