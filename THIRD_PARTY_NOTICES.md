# Third-party software

Folio's original application code is MIT licensed. Dependencies retain their own licenses.

- Tauri: MIT OR Apache-2.0 — https://github.com/tauri-apps/tauri
- ttf-parser 0.25.1: MIT OR Apache-2.0 — https://github.com/harfbuzz/ttf-parser; full notices are included under licenses/rust/ttf-parser-0.25.1 in the portable distribution.
- pdfium-render: MIT OR Apache-2.0 — https://github.com/ajrcarey/pdfium-render
- PDFium and bundled dependencies: see the licenses distributed in resources/pdfium. Source: https://pdfium.googlesource.com/pdfium/
- Windows PDFium build patches and component notices: https://github.com/bblanchon/pdfium-binaries
- Folio's PDFium source corrections and pinned build inputs: patches/pdfium/form-text-state-inheritance.patch, patches/pdfium/fractional-horizontal-advances.patch, patches/pdfium/ordered-default-colors.patch, patches/pdfium/embedded-form-font-preference.patch, patches/pdfium/fractional-form-borders.patch and scripts/pdfium-source-lock.json. Build instructions: docs/building-pdfium.md.
- Frontend and Rust dependency versions are recorded in package-lock.json and src-tauri/Cargo.lock.

The engine download is pinned to chromium/8044 and checked against SHA-256
78a17d9a5f14467631c26a3ac8741b27a0471ecc05bd6a119b523598160a0537.
The downloader retains the upstream license files and writes provenance.json.
Folio does not claim ownership of the third-party components.

The pinned pdfium-render 0.9.4 bindings carry a local correction that serializes
native calls omitted by the upstream thread-safe wrapper. The upstream dual-license
notice, complete Apache-2.0 text, original archive checksum, patch and maintenance notes
are retained in `src-tauri/vendor/pdfium-render-0.9.4/` and included under
`licenses/rust/pdfium-render-0.9.4/` in portable packages. This is a local change,
not an upstream release or endorsement.

Protected-document development uses a locally patched lopdf0.45.0 snapshot, MIT
Copyright (c)2016 Junfeng Liu. Its license, exact patch, source hashes and maintenance
notes are in `src-tauri/vendor/lopdf-0.45.0/`. The portable license collector includes
these under `licenses/rust/lopdf-0.45.0/`. The changes correct owner-key derivation,
stream-dictionary encryption, encrypted-load error propagation and complete AES-256
permission validation; they support direct encryption dictionaries and add an opt-in
metadata inspection mode. They do not imply upstream endorsement.

The development compatibility corpus includes the unmodified DejaVu Serif font
and its full license in `tests/fixtures/corpus/fonts/LICENSE_DEJAVU.txt`.
Embedded-text fixtures also contain DejaVu Serif subsets; their generation and license are recorded in `tests/fixtures/embedded-text/README.md`. The complex-text research probe embeds only this redistributable DejaVu fixture; Windows fonts are read for local shaping experiments and are not distributed.
Fixture provenance and checksums are recorded in the adjacent manifest. Corpus
PDFs contain synthetic test data; Python generation/inspection tools are development
dependencies and are not embedded in the desktop application.

The issue #14 development gate additionally includes DejaVu Sans and static Noto
Sans Arabic/Devanagari fonts. Full DejaVu and SIL Open Font License notices and
immutable source hashes are in [tests/fixtures/shaped-text](tests/fixtures/shaped-text/README.md).
Versioned Unicode development data carries the complete
[Unicode License V3](tests/fixtures/shaped-text/unicode/LICENSE_UNICODE.txt).
Native shaping dependencies, versions and licenses are documented in
[the dependency decision](docs/shaped-text-dependencies.md). They are included
in default builds starting with v0.10.0; full dependency licenses are bundled
under `licenses/rust`.

The native shaper uses a locally patched HarfRust 0.13.3 snapshot. Its
complete upstream MIT license, archive/source checksums, exact patch, and
maintenance notes are in [src-tauri/vendor/harfrust-0.13.3](src-tauri/vendor/harfrust-0.13.3/FOLIO_VENDOR.md).
The local changes expose and enforce shaping work limits; they do not imply
upstream endorsement. Hostile-font regression inputs are modified DejaVu
subsets with the original license and generation provenance alongside them.

## Standard font metric data

The WinAnsi advance tables in `src-tauri/src/persistence.rs` are adapted from
ReportLab 4.5.1's `pdfbase/_fontdata_enc_winansi.py` and
`_fontdata_widths_{helvetica,helveticabold,timesroman,timesbold,timesitalic,timesbolditalic}.py`,
loaded through `_fontdata.py`. Helvetica oblique shares upright widths; Courier
uses a fixed advance. Additional nominal Standard/MacRoman glyph advances in
`src-tauri/src/standard_font.rs` use the same licensed width sources, including
`_fontdata_widths_courier.py`. No ReportLab font binaries are included.

ReportLab's three-clause BSD license is reproduced in [docs/licenses/REPORTLAB.txt](docs/licenses/REPORTLAB.txt).
Additional notices in the distribution: Copyright (c) 2000-2025, ReportLab Inc.;
Copyright ReportLab Europe Ltd. 2000-2017. Source: https://www.reportlab.com/.

The portable distribution includes HarfRust’s local change notice, complete patch,
and source provenance in `licenses/rust/harfrust-0.13.3/` alongside its full LICENSE.

## Local OCR

The recognition engine uses [Tesseract.js6.0.1](https://github.com/naptha/tesseract.js),
[tesseract.js-core6.0.0](https://github.com/naptha/tesseract.js-core) and English,
French and Spanish [tessdata_best4.1.0](https://github.com/tesseract-ocr/tessdata_best/tree/4.1.0)
models under the Apache License2.0. Full upstream licenses accompany the embedded
OCR assets; portable packages also include dependency licenses and
`licenses/ocr/tessdata_best/LICENSE`. The asset manifest records each source, size
and SHA256. Folio's wrapper restricts asset loading and manages cancellation; the
upstream worker, core and model bytes are unmodified.
