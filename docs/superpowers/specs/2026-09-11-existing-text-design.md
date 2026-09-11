# Existing PDF text editing — v0.7

Issue #8 adds actual page-content changes. The first version supports a visible top-level, single-line text run in a validated standard Latin font, preserving font, color, baseline, transform and surrounding layout. Users choose Edit text, click an outlined run, change its content in a focused modal, then apply one undoable edit. Unsupported runs and scanned pages explain their limitations. Empty/control characters, unavailable glyphs, unsafe positioning and overflowing replacements are rejected; no automatic font substitution, cover-up overlay, OCR, paragraph reflow or redaction is introduced.

## Architecture

The native worker lists text objects with displayed page-coordinate bounds and eligibility reasons. Replacement works on an isolated one-page copy. It verifies expected original text to reject stale requests, validates a no-op replacement against original glyph positions, applies the replacement, and checks saved/reopened extraction. Rejection leaves the original source intact. Success registers the actual changed PDF bytes under a new source ID, retaining the original file path for overwrite protection.

The frontend commits only the chosen page's source ID/page index change. Page ID, dimensions, editor rotation and overlays are preserved; duplicates keep their own source version. Source IDs stay owned by the document session through history and are released on tab close. Existing source-keyed raster/text caches, search, export, print and recovery therefore observe the changed content naturally. Recovery snapshots only current versions; undo history remains session-only as before.

The edit dialog prevents other workspace operations while applying and displays errors without losing typed text. Requests validate page/source/run identity, so late responses cannot change another tab or resurrect a removed page. Undo and redo remain single steps.

## Verification

Native tests use actual PDFium on generated fixtures: standard font families, replacement lengths, page rotations/crops, unchanged originals, save/reopen extraction, stale edits, unsupported fonts and positioned/overflowing text. Independent extraction confirms old text was replaced rather than hidden. Frontend tests cover choosing/applying/cancelling, undo/redo, duplicate isolation, search invalidation, errors, and session source ownership. Browser/native smoke verifies hit targets across rotations and saved/reopened edits. Existing 153 frontend regressions and native suites must remain passing before packaging. Final supported-font/layout limits are documented from empirical results.
