# Folio v0.10.1 verification

Verified on Windows x64 on 2026-09-13 with bundled PDFium chromium/8044.
[Machine-readable results](verification-v0.10.1.json) record the tested executable
hash and the actual packaged workflows. This executable is included in the ZIP.

## Automated checks

- 276 frontend tests across 49 files; 143 native tests, with three existing opt-in tests ignored.
- Eleven real Chromium editing cases cover grapheme boundaries, supplementary characters, independent ligature letters, trusted backward/forward deletion, caret positions and native undo/redo.
- TypeScript/Vite and optimized Tauri production builds passed; independent code review found no outstanding blocker.
- Forty-eight native wide-font PDFs (24 original/export pairs) preserve exact Unicode in PDFium, MuPDF and pypdf. Original visible outlines and fonts survive export. The pinned reader geometry model and existing cluster-grouping tolerance pass; see [native evidence](issue14-native-wide-verification.json).
- Locked Rust/npm dependencies are unchanged from v0.10.0 apart from Folio's version. The package reuses its 551 audited dependency license records and full notices.

## Packaged Windows app

Tests drove the production WebView and native Open, Import font and Save dialogs,
using an isolated recovery directory and synthetic/public documents.

- Font import, native shaped preview, ligature toggles, undo/redo and WebView composition passed.
- Editable and flattened saves preserve the source. Editable reopen restores font/settings after the source PDF and imported font are moved.
- Search and flattened drag-copy return exact `office café Ω`. Print setup opens and cancels; no physical job was submitted. Tab return retains prepared outlines.
- Forced termination of the owned test process followed by restart restores the unsaved shaped draft, font, outlines and searchable text with source files unavailable.
- Whole-grapheme and supplementary deletion, independent ligature-letter deletion, and native undo/redo passed in the packaged WebView.
- A wide text box beyond 255 semantic definitions passed native preview, editable/flattened saves and flattened reader reopen with exact Unicode.
- Arabic `سلام` and Indic `नमस्ते` passed native font import, shaped preview, flattened save and native reader reopen.

The first new desktop harness run could not pointer-select an annotation beneath
the reader text layer after recovery. The harness now uses the annotation's
supported keyboard focus/Enter selection. No application behavior was changed
for that automation failure.

## Remaining acceptance

Physical Windows IME and interactive Foxit/Acrobat copy/search remain pending.
WebView composition and independent extraction are not substitutes for those checks.
Mixed-direction and RTL word-separator/joiner inputs remain explicitly refused:
the additional wide-font and reader-policy probes still fail logical text or
character ordering. Issue #14 remains open; see [acceptance status](issue14-status.md).
