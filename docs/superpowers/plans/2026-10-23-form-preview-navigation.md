# Form preview navigation implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution of this milestone.

**Goal:** Make small form fields readable and reachable without changing unsaved values.

**Architecture:** Extract preview rendering and URL ownership into FormPagePreview. Keep field and draft ownership in FormFillDialog. Reuse FormPageTargets geometry for locating fields.

**Tech stack:** React, TypeScript, Vitest, Tauri/PDFium, Windows WebView acceptance.

**Spec:** Fit width remains the default; 1.5×, 2× and 3× fit-width views enlarge the page within an independently scrollable region. Raster requests scale from 900 to a bounded 2400 pixels. Retain a valid same-page raster while sharpening; never show a stale source. Show selected field centers the clipped visible target inside the preview only. Hidden or invalid targets cannot be located. Zoom and locate preserve drafts, selection, and native mutation semantics. Zoom persists through source replacements during Apply; reopening the dialog resets it.

## Constraints and review focus

- Preserve existing source edits and accepted portable packages. No Rust behavior changes.
- Late render results and unmount must revoke owned blob URLs exactly once.
- Source/adapter/rotation changes must hide obsolete geometry and raster immediately.
- Busy and dirty states must retain existing selection protection.
- Narrow windows must retain accessible toolbar, scroll region, value controls, and footer.
- Resizing and intrinsic/workspace rotation must keep raster and outlines aligned.

## Task: navigable form preview

- [x] Add failing regression tests for draft preservation, bounded resolution, locate geometry, fallback rendering, and URL lifetime.
- [x] Extract FormPagePreview, add zoom/locate toolbar and scrolling styles, integrate dialog.
- [x] Pass affected and full frontend tests and TypeScript/Vite build.
- [x] Obtain one scoped review and address actionable findings (none).
- [x] Build copied Windows executable and run zoom/rotation/resize/save/reopen acceptance with independent PDF verification.
- [x] Record evidence, source fingerprint, limitations, and next gap.

Evidence: artifacts/form-preview-verification.json. All700 frontend tests pass. Fresh native suite unnecessary: every src-tauri fingerprint entry matches the previous23-test checkpoint. Windows acceptance-2 passes; the first harness size assertion is preserved and superseded by current-layout geometry measurements. No product changes after review/build.
