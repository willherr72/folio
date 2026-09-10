# Third-party software

Folio's original application code is MIT licensed. Dependencies retain their own licenses.

- Tauri: MIT OR Apache-2.0 — https://github.com/tauri-apps/tauri
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
