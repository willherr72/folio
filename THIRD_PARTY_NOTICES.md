# Third-party software

Folio's original application code is MIT licensed. Dependencies retain their own licenses.

- Tauri: MIT OR Apache-2.0 — https://github.com/tauri-apps/tauri
- ttf-parser 0.25.1: MIT OR Apache-2.0 — https://github.com/harfbuzz/ttf-parser; full notices are included under licenses/rust/ttf-parser-0.25.1 in the portable distribution.
- pdfium-render: MIT OR Apache-2.0 — https://github.com/ajrcarey/pdfium-render
- PDFium and bundled dependencies: see the licenses distributed in resources/pdfium. Source: https://pdfium.googlesource.com/pdfium/
- Windows PDFium binary distributor: https://github.com/bblanchon/pdfium-binaries
- Frontend and Rust dependency versions are recorded in package-lock.json and src-tauri/Cargo.lock.

The engine download is pinned to chromium/8044 and checked against SHA-256
78a17d9a5f14467631c26a3ac8741b27a0471ecc05bd6a119b523598160a0537.
The downloader retains the upstream license files and writes provenance.json.
Folio does not claim ownership of the third-party components.

The development compatibility corpus includes the unmodified DejaVu Serif font
and its full license in `tests/fixtures/corpus/fonts/LICENSE_DEJAVU.txt`.
Fixture provenance and checksums are recorded in the adjacent manifest. Corpus
PDFs contain synthetic test data; Python generation/inspection tools are development
dependencies and are not embedded in the desktop application.

## Standard font metric data

The WinAnsi advance tables in `src-tauri/src/persistence.rs` are adapted from
ReportLab 4.5.1's `pdfbase/_fontdata_enc_winansi.py` and
`_fontdata_widths_{helvetica,helveticabold,timesroman,timesbold,timesitalic,timesbolditalic}.py`,
loaded through `_fontdata.py`. Helvetica oblique shares upright widths; Courier
uses a fixed advance. No ReportLab font binaries are included.

ReportLab's three-clause BSD license is reproduced in [docs/licenses/REPORTLAB.txt](docs/licenses/REPORTLAB.txt).
Additional notices in the distribution: Copyright (c) 2000-2025, ReportLab Inc.;
Copyright ReportLab Europe Ltd. 2000-2017. Source: https://www.reportlab.com/.
