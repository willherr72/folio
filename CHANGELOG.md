# Changelog

## 0.7.1

- Allow longer existing-text replacements to grow into available page space, preserving font size and baseline. Reject page overflow and new or expanded overlap with neighboring source text and graphics.
- Add a text-box Font picker with Helvetica, Times and Courier in regular, bold, italic and bold italic styles. Preserve font choice in undo/redo, recovery, editable export/reopen and flattened copies.
- Measure added-text selection/search geometry with the selected preview font. Preserve compatibility with older Helvetica annotations.
- Document the next steps for installed, embedded/subset and complex-font support.

## 0.7.0

- Edit supported existing PDF text runs through the new Edit text tool.
- Preserve the original standard Latin font, color and placement; report unsupported fonts, layouts and replacements that do not fit.
- Commit actual changed PDF content as an undoable page version, keeping duplicates and annotations independent.
- Search, copy, printing, save/reopen and recovery use the replacement text.
- Validate no-op geometry and rendering before changes, reject unsafe page features, and protect original files.
- Add font/rotation, content-preservation, source-lifecycle and browser editing regressions.

## 0.6.1

- Reuse measured text geometry when revisiting cached PDF pages, reducing dense-page tab layout work.
- Release off-screen thumbnail images so scrolling long documents respects the existing raster cache limits.
- Cover warm text selection, copy and highlighting across page rotations, duplicate-page metric reuse, and fast thumbnail visibility transitions.
- Add repeatable packaged tab and WebView process-memory profiling, including explicitly labeled CPU-throttled simulations.

## 0.6.0

- Added text, drawings and signatures remain editable after save and reopen, without original files or sidecars.
- Portable standard FreeText/Ink annotation appearances with versioned, validated Folio metadata.
- Explicit Flatten text and ink export, with editable Save a copy/Ctrl+S as the default.
- Restored text orientation, selection, search and dragging across page rotations.
- Recovery migrates earlier annotation import versions without resurrecting deleted additions.
- Redistributable synthetic PDF corpus, large-document generators and native/desktop performance reports.
- Expanded crop, rotation, organization, repeated save/reopen and bounded-cache regression coverage.

## 0.5.0

- Named local signature library with previews, reuse, rename and deletion.
- Proportional width controls for placed signatures and other ink.
- Embedded text highlights across lines, rotation and zoom.
- Anchored comments with immediate editing and a document-wide review list.
- Standard PDF Highlight/Text annotation export and editable reopen, including notes.
- Annotation undo/redo, per-tab isolation and recovery compatibility.

## 0.4.0

- Local crash recovery restores editable tabs, source snapshots, page organization, zoom and scroll.
- Atomic checkpoints, previous-generation fallback, safe discard and instance locking protect recovery data.
- Ctrl+F searches embedded and added text, with per-tab queries and rotated-page result highlights.
- Ctrl+P prints current edits through Windows, with page ranges, fit/actual sizing and printer properties.
- Immutable source snapshots survive moved originals and retain original-path export protection.
- Additional lifecycle, geometry, native printing and crash/restart regression coverage.
## 0.3.1

- Faster PDF tab switching: measure the text layer in one batch instead of forcing layout for every character.
- Reuse recently rendered pages when revisiting tabs or scrolling back.
- Limit retained page images by estimated decoded memory (96 MiB) and entry count, while protecting currently displayed pages.
- Regression checks cover cache reuse, eviction, late native renders and dense-page tab performance.

## 0.3.0

- Open PDFs in separate top tabs, retaining each document's edits, undo/redo
  history, selected page, zoom and scroll position.
- Open creates a new tab; Add PDF continues to append pages to the active document.
- Ctrl+Tab/Ctrl+Shift+Tab switch tabs; Ctrl+W closes the active tab.
- Closing an unsaved tab asks for confirmation; closing Folio checks all dirty
  tabs, including inactive documents.
- Creating text immediately focuses Content and selects the placeholder for typing.
- Pending signatures follow the pointer at their actual size, clamp to page edges,
  and offer a placement hint and Escape cancellation.
- Select and copy embedded PDF text with native browser selection, preserving
  spaces and line breaks through page rotation and zoom. No OCR is included.
- Load text geometry for mounted pages, bound its cache, and release closed sources.

## 0.2.0

- Centered app confirmation dialogs replace browser prompts for opening and closing.
- Freehand drawing with remembered pen color and width, one undo step per stroke.
- Ctrl+scroll zoom anchored to the pointer.
- Continuous page scrolling with lazy rendering and synchronized page selection.
- Settings for Light/Dark/System themes, page view and drawing defaults.
- Working thumbnail drag-and-drop on Windows, insertion markers and undo.
- Removed the PDF engine status badge.
- Fixed undo for a drag interrupted by a dialog and restored keyboard focus after dialogs.
- Versioned portable folders let the update coexist with the first build.

## 0.1.0

Initial prototype: native PDF viewing, added text, signatures, page organization,
merging, undo/redo, and vector-preserving PDF export.
