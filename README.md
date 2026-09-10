# Folio

Your everyday PDF workspace. A local Windows desktop prototype built with Rust,
Tauri and native PDFium. No AI, account, cloud upload or subscription.

![Folio desktop editor](docs/folio-desktop.png)

## Try it

Double-click **Launch Folio.cmd** in this repository. The ready-to-run executable
is **artifacts/Folio/Folio.exe**; keep its `resources` folder beside it.
**artifacts/Folio-windows-x64.zip** contains the portable distribution.

1. Open **examples/Welcome to Folio.pdf**.
2. Choose **Text**, click the page, and edit the note in the properties panel.
   Drag added text to move it; change its size or color.
3. Choose **Signature**, draw with your mouse or pen, then click to place it.
4. Click an empty part of the page to see page tools: rotate, move, duplicate,
   or delete. **Add PDF** appends pages from another document.
5. Choose **Save a copy**, then reopen the saved PDF to check the result.

Undo and redo work across edits and page operations. Shortcuts include Ctrl+Z,
Ctrl+Shift+Z, Ctrl+O and Ctrl+S. Original open source PDFs cannot be overwritten.
Edits remain in memory until you save; closing with unsaved changes prompts you.

## What this version does

- Native PDF rendering with zoom and lazy page thumbnails.
- Add and move multiline text with size and color controls.
- Draw and place vector signatures.
- Reorder, rotate, duplicate, delete and merge pages.
- Undo/redo and export a new PDF while preserving source vector page content.

## Prototype boundaries

- Existing words in a PDF are not editable.
- Drawn signatures are visual marks, not certificate-based digital signatures.
- Exported additions become page content; reopening does not restore editing handles.
- Added text uses Helvetica and supports a verified Latin character set and common
  punctuation. Unsupported characters, including CJK, emoji, nonbreaking spaces
  and soft hyphens, produce an export error instead of silently disappearing.
- One selected page is shown at a time. Search, continuous scrolling, printing,
  OCR, redaction, form editing and password-protected PDFs are not implemented.
- Foxit performance parity has not been established. This is a working prototype,
  not a finished replacement.
- Windows x64 is the tested target. Microsoft Edge WebView2 Runtime is required.

## Build from source

Install Node.js, the stable Rust MSVC toolchain, Visual Studio C++ Build Tools
with the Windows SDK, and Microsoft Edge WebView2 Runtime.

From this directory in PowerShell:

```powershell
npm ci
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-pdfium.ps1
npm run tauri dev
```

For a release executable and portable ZIP:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-portable.ps1
```

The setup step downloads a pinned PDFium binary and checks its SHA-256.
All PDF processing after setup happens locally.

## Verification

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1
```

The native tests require the fetched PDFium binary. Browser smoke checks use
`node scripts/smoke-ui.mjs` with the Vite server running and Playwright Chromium
installed. Explicit browser demo mode exercises the interface with sample
documents and exports a JSON edit plan; real PDF editing runs in the desktop app.

The desktop smoke starts the real app, opens a PDF through the Windows dialog,
adds text and a signature, duplicates and merges pages, saves, then reopens the
seven-page output. See [verification notes](docs/verification.md) for results.

## Internals and license

The interface keeps an undoable edit plan. Rust retains source documents and
serializes PDFium operations on a dedicated worker. The frontend displays native
page renders with editable overlays. Export copies PDF pages and adds text and
vector ink, preserving the original page content.

See [the design](docs/superpowers/specs/2026-09-10-folio-design.md)
and [implementation plan](docs/superpowers/plans/2026-09-10-folio.md).

MIT for Folio's original code. Dependencies retain their own licenses; see
[third-party notices](THIRD_PARTY_NOTICES.md). The portable bundle includes the
PDFium license directory and collected dependency notices in `licenses`.
