# Folio

Your everyday PDF workspace. A local Windows desktop PDF editor built with Rust,
Tauri and native PDFium. No AI, account, cloud upload or subscription.

![Folio desktop editor](docs/folio-desktop.png)

## Get Folio

Download the Windows portable ZIP from [Releases](https://github.com/willherr72/folio/releases).
Extract it and open **Folio.exe**. Keep the resources folder beside the executable.
Microsoft Edge WebView2 Runtime is required.

In this development checkout, double-click **Launch Folio.cmd** after building.
Version 0.4.0 lives in **artifacts/Folio-v0.4.0/Folio.exe**.
The ZIP is **artifacts/Folio-v0.4.0-windows-x64.zip**.

## Everyday editing

- **Documents:** Open adds a PDF in a new top tab. Each document retains its edits,
  undo/redo history, selected page, zoom and scroll position when you switch tabs.
- **Text:** choose Text and click a page. The Content field selects the placeholder
  immediately, so typing replaces it. Adjust size/color and drag the note to move it.
- **Select and copy:** drag across embedded PDF text in Select mode, then press
  **Ctrl+C** to copy it, including spaces and line breaks. Scanned pages without
  embedded text require OCR, which is not included.
- **Draw:** write or sketch directly on any page. Choose pen color and width in
  Properties. Each stroke is one undo step; Select lets you move or delete it.
- **Signature:** draw in the signature pad, then move over a page to preview its
  actual placement. Click to place it, or press **Escape** to cancel.
- **Pages:** drag thumbnails by their handles to reorder. A line shows the drop
  position. Page properties also provide move, rotate, duplicate and delete.
  **Add PDF** appends pages from another PDF to the active document.
- **Reading:** scroll continuously between pages. Hold **Ctrl** while scrolling
  to zoom around the pointer.
- **Settings:** choose Light, Dark or System theme, continuous or single-page
  view, default zoom, pen color and pen width. Preferences are remembered.
- **Search:** Ctrl+F finds words or phrases in embedded PDF text and your added text.
  Enter/Shift+Enter move through highlighted results; Escape closes search. Each tab remembers its search.
- **Print:** Ctrl+P prints the current edits. Choose all pages, the current page, or
  a range, then fit/actual size and orientation. The Windows dialog selects the printer,
  paper and copies. Printing does not mark your edits saved.
- **Recovery:** local checkpoints preserve native tabs, edits, page arrangement,
  zoom and scroll after an unexpected exit. On restart, restore or discard the workspace.
  Source snapshots allow recovery even if the original PDFs moved. Undo history starts fresh.
- **Save a copy:** export a new PDF while preserving source vector content.

Start with **examples/Welcome to Folio.pdf**. Ctrl+Z/Ctrl+Shift+Z undo and redo;
Ctrl+O opens a PDF in a new tab and Ctrl+S saves a copy of the active document.
Ctrl+Tab/Ctrl+Shift+Tab switch tabs; Ctrl+W closes the active tab. Closing an
unsaved tab asks for confirmation. Closing Folio checks every tab for unsaved
changes, including inactive tabs. Cancelling a close leaves your documents open.

Dark mode changes the interface; PDF pages retain their original colors.

## Prototype boundaries

- Existing words in a PDF are not editable.
- Drawn signatures are visual marks, not certificate-based digital signatures.
- Exported additions become page content; reopening does not restore their editing handles.
- Added text uses Helvetica with a verified Latin character set and common punctuation.
  Unsupported characters, including CJK, emoji, nonbreaking spaces and soft hyphens,
  produce an export error instead of silently disappearing.
- Search, printing, OCR, redaction, form editing and password-protected PDFs are
  not implemented.
- Printing uses bounded raster images (up to 2400 pixels wide), so it does not preserve
  selectable/vector text in virtual-printer output. Save a copy preserves source vectors.
- Opened PDFs are held as immutable source bytes, with a 512 MiB per-file limit.
- Windows x64 is the tested target. Foxit performance parity has not been established.

## Local recovery data

Folio checkpoints after one second of inactivity, or within five seconds during continuous
editing. A crash can lose changes since the last successful checkpoint. Confirmed normal
closing discards recovery data; cancelling a close keeps your work. Source files are never
automatically overwritten. Recovery is separate from saving a PDF with editable handles.

Data lives under the application local-data folder (normally
%LOCALAPPDATA%/com.folio.desktop/recovery). Set **FOLIO_DATA_DIR** to choose a different
local data directory. A second instance using the same directory can continue with
recovery disabled; it cannot overwrite the first instance's checkpoints. Failed checkpoints
show a visible warning. Checkpoints are limited to 32 MiB of edit/view metadata.

## Roadmap

[GitHub Issues](https://github.com/willherr72/folio/issues) is the central backlog.
Milestones group v0.4 recovery/search/printing, v0.5 reusable signatures/highlights,
v0.6 editable persistence/extensive testing, and v0.7 existing-word editing.

## Build from source

Install Node.js, stable Rust with the MSVC toolchain, Visual Studio C++ Build
Tools with the Windows SDK, and Microsoft Edge WebView2 Runtime.

In PowerShell:

```powershell
npm ci
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-pdfium.ps1
npm run tauri dev
```

Build a release executable and portable ZIP:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-portable.ps1
```

Setup downloads a pinned PDFium binary and checks its SHA-256.
All PDF processing afterward happens locally.

## Verification

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Browser checks: install Playwright Chromium, start the Vite server, then run
**scripts/smoke-ui.mjs** and **scripts/smoke-upgrades.mjs** with Node.
Explicit browser demo mode uses generated pages and exports a JSON edit plan.
Real PDF editing runs in the desktop app.

The native smoke opens a real PDF through Windows dialogs, edits it, draws,
drags thumbnails, checks both close-confirmation decisions, saves and reopens.
Its launcher uses an isolated test profile. See [verification notes](docs/verification.md).

## Internals and license

React keeps an undoable edit plan. Rust owns source documents and serializes
PDFium work on a dedicated thread. Nearby pages use cached native renders and
embedded text geometry.
Export copies PDF pages and adds text and vector ink.

MIT for Folio's original code. Dependencies retain their own licenses; see
[third-party notices](THIRD_PARTY_NOTICES.md). Portable releases include PDFium
licenses and collected dependency notices.

[Changelog](CHANGELOG.md)
