# Folio

Your everyday PDF workspace. A local Windows desktop PDF editor built with Rust,
Tauri and native PDFium. No AI, account, cloud upload or subscription.

![Folio desktop editor](docs/folio-desktop.png)

## Get Folio

Download the Windows portable ZIP from [Releases](https://github.com/willherr72/folio/releases).
Extract it and open **Folio.exe**. Keep the resources folder beside the executable.
Microsoft Edge WebView2 Runtime is required.

In this development checkout, double-click **Launch Folio.cmd** after building.
Version 0.9.0 lives in **artifacts/Folio-v0.9.0/Folio.exe**.
The ZIP is **artifacts/Folio-v0.9.0-windows-x64.zip**.

## Everyday editing

- **Documents:** Open adds a PDF in a new top tab. Each document retains its edits,
  undo/redo history, selected page, zoom and scroll position when you switch tabs.
- **Edit existing text:** choose **Edit text**, click an outlined run, change **Replacement text**, and apply. Supported horizontal runs use standard Helvetica, Times and Courier or verified embedded TrueType fonts, including subsets. Longer replacements can use available space. If the subset lacks a letter, choose and preview an explicit substitute font. Complex layouts remain unsupported. [Support details](docs/existing-text-editing.md).
- **Text:** choose Text and click a page. The Content field selects the placeholder
  immediately, so typing replaces it. Choose a font and style, adjust size/color, and drag the note to move it. **More fonts…** searches installed fonts or imports a local TTF/OTF. Preview your text, then choose **Apply font**. Supported fonts are embedded so recipients do not need to install them. [Font support](docs/font-support-roadmap.md).
- **Select and copy:** drag across embedded PDF text in Select mode, then press
  **Ctrl+C** to copy it, including spaces and line breaks. Scanned pages without
  embedded text require OCR, which is not included.
- **Draw:** write or sketch directly on any page. Choose pen color and width in
  Properties. Each stroke is one undo step; Select lets you move or delete it.
- **Signature:** draw or choose a saved signature. Give drawings a name to keep them in
  the local library, with rename and delete controls. Move over a page to preview,
  click to place, or press **Escape** to cancel. Properties adjusts the placed ink's
  width while keeping its proportions.
- **Highlight:** choose Highlight and drag across embedded text, including multiple
  lines. Select highlights on the page or in Review to change their color or note.
- **Comment:** click a page to place a note and start typing. Review lists highlights
  and comments throughout the document; select one to jump to its anchor. Notes can
  be edited, moved or deleted. These edits support undo/redo in each tab.
- **Review persistence:** highlights and comments save as standard PDF annotations
  and reopen with editing controls. Supported annotations from other readers are
  imported too. Unsupported types, locked notes, and notes with special zoom/rotation
  or visibility flags remain in the source content with their original appearance.
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
  Source and font snapshots allow recovery even if the original PDFs or imported font files moved. Undo history starts fresh.
- **Save a copy:** save a portable PDF with editable text, drawings and signatures,
  alongside highlights and comments. Reopening in Folio restores their controls,
  including on rotated or reorganized pages. No original files or sidecars are needed.
  The arrow beside Save offers **Flatten text and ink**, which turns those additions
  into page content. Highlights and comments remain annotations. A flattened export
  does not mark your editable workspace saved.

Start with **examples/Welcome to Folio.pdf**. Ctrl+Z/Ctrl+Shift+Z undo and redo;
Ctrl+O opens a PDF in a new tab and Ctrl+S saves a copy of the active document.
Ctrl+Tab/Ctrl+Shift+Tab switch tabs; Ctrl+W closes the active tab. Closing an
unsaved tab asks for confirmation. Closing Folio checks every tab for unsaved
changes, including inactive tabs. Cancelling a close leaves your documents open.

Dark mode changes the interface; PDF pages retain their original colors.

## Prototype boundaries

- Existing-text editing requires a verified encoding, glyph mapping and simple horizontal layout. Ambiguous embedded fonts, complex scripts, individually positioned text and nested forms remain unsupported.
- Drawn signatures are visual marks, not certificate-based digital signatures.
- Editable additions use standard PDF annotations with self-contained appearances and
  versioned Folio metadata. Other readers can display them; edits made by another
  reader may prevent Folio from restoring handles. Unsupported or changed metadata
  leaves the annotation in the PDF with its native appearance. See
  [editable PDF format](docs/editable-pdf-format.md).
- Added text supports twelve standard PDF faces and installed/imported static TrueType-outline fonts with editable embedding permissions. Custom fonts support covered Latin, Greek, Cyrillic and selected punctuation/symbols; CFF, variable/color fonts, collections, complex scripts and combining sequences are not supported yet. Unsupported characters produce a clear error.
- OCR, redaction, form editing and password-protected PDFs are not implemented.
- Printing uses bounded raster images (up to 2400 pixels wide), so it does not preserve
  selectable/vector text in virtual-printer output. Save a copy preserves source vectors.
- Opened PDFs are held as immutable source bytes, with a 512 MiB per-file limit.
- Windows x64 is the tested target. Foxit performance parity has not been established.

## Local signature library

Named signatures are stored in this application's local WebView profile. There is no
account or upload. The library holds up to 30 signatures and reports storage errors.
Deleting a library entry does not remove signatures already placed in a document.
Visual signatures do not provide certificate verification.

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
v0.6 editable persistence/extensive testing, v0.6.1 performance fixes, and v0.7 existing-word editing.
Physical 8 GB hardware validation remains tracked in [issue #10](https://github.com/willherr72/folio/issues/10).

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
Its launcher uses an isolated test profile. See [v0.9.0 text-edit verification](docs/verification-v0.9.0.md), [v0.6.1 performance verification](docs/performance-v0.6.1.md), the [compatibility corpus](docs/corpus/README.md), and [earlier verification notes](docs/verification.md).

## Internals and license

React keeps an undoable edit plan. Rust owns source documents and serializes
PDFium work on a dedicated thread. Nearby pages use cached native renders and
embedded text geometry.
Export copies PDF pages and adds text, vector ink and standard review annotations.

MIT for Folio's original code. Dependencies retain their own licenses; see
[third-party notices](THIRD_PARTY_NOTICES.md). Portable releases include PDFium
licenses and collected dependency notices.

[Changelog](CHANGELOG.md)

## Compatibility and performance corpus

The [corpus guide](docs/corpus/README.md) records fixture provenance, reproducible
commands and measured Windows baselines. Small synthetic PDFs are versioned; large
scans and 300-page fixtures are generated locally. No private documents are included.

The [complex-text development gate](docs/shaped-text-interop.md) records native
shaping and PDF serialization experiments for issue #14. These require the
nondefault `shaped-text` Cargo feature and are not enabled in application builds.
Reader copy and selection compatibility must pass before broader script support
is offered in the editor.

The [native semantic PDF verification](docs/semantic-native-verification.md)
passes exact Unicode in three readers for 32 unmixed sample/rotation cases.
Mixed-direction text remains refused, and MuPDF cluster selection still differs.
[Shaping resource checks](docs/shaping-resource-limits.md) document the pinned
completion-status patch and bounded outline traversal. These development results
do not enable complex-script input in the released editor.

[Long-text verification](docs/semantic-long-native-verification.md) extends this
experiment through exact character-definition reuse, including a 4,096-scalar
fixture. It also records an RTL word-order failure that was hidden by repeated
words. RTL separators and joiners now produce an explicit refusal; they still
need a proven reader-order strategy before editor integration.
