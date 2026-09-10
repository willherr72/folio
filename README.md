# Folio

A small, local desktop PDF editor. Built with Rust, Tauri and native PDFium.

Folio is an early Windows prototype for adding text and handwritten signatures,
organizing pages, and saving a new PDF. No AI, account, cloud upload, or subscription.
The source is yours to modify under the MIT license; third-party dependencies
retain their own licenses.

## Open the app

After the portable build, double-click **Launch Folio.cmd** in this repository.
The executable lives at **artifacts/Folio/Folio.exe**. Keep its resources folder
beside it. The portable archive is **artifacts/Folio-windows-x64.zip**.

Start with **examples/Welcome to Folio.pdf**. It has room for a note and signature,
a page for trying page tools, and a landscape page.

## Build from source (Windows x64)

Install Node.js, the stable Rust MSVC toolchain, Visual Studio C++ Build Tools
with the Windows SDK, and Microsoft Edge WebView2 Runtime.

From this directory, in PowerShell:

```powershell
npm ci
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-pdfium.ps1
npm run tauri dev
```

For a release executable and portable ZIP:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-portable.ps1
```

The setup step downloads a pinned PDFium Windows binary and checks its SHA-256.
All PDF processing after setup happens locally. There is no runtime network
dependency other than the installed WebView2 runtime itself.

## Design

The interface keeps an undoable edit plan. Rust retains open documents and
serializes PDFium operations on a dedicated worker. The frontend displays native
page renders and editable overlays. Export copies source PDF pages and adds
text and vector ink; it does not flatten every page to a screenshot.

See [the design](docs/superpowers/specs/2026-09-10-folio-design.md)
and [implementation plan](docs/superpowers/plans/2026-09-10-folio.md).

## Prototype boundaries

- Existing words in the PDF are not editable yet.
- Handwritten signatures are visual marks, not certificate-based digital signatures.
- Exported additions become page content; reopening does not restore their editing handles.
- This first viewer shows one selected page at a time.
- No claim of Foxit performance parity has been established.
- OCR, redaction, certificate signing and form editing are outside this version.

## License

MIT for Folio's original code. See [third-party notices](THIRD_PARTY_NOTICES.md)
and the bundled PDFium license directory for dependencies.
