# Changelog

## Unreleased

- Require exact external-reader text in the text-area verifier by default; diagnostic results cannot satisfy the release gate. Retain 80 experimental separator-encoding cases with their copying failures.

- Add a development-only native Latin text-area layout API with measured wrapping, alignment, exact logical source ranges and explicit overflow. Existing desktop overlays are unchanged.
- Add native-vector/PDFium preview comparisons and saved-reader fixtures. Preserve the observed soft-wrap newline and repeated-space copy failures as integration gates for #16; no text-area tool is enabled yet.

## 0.13.0

- Edit one selected text occurrence inside a supported nested form without changing shared placements on the same or other pages.
- Preserve occurrence identity, immutable undo/redo, search, save/reopen and recovery through the existing text dialog. Add a two-page shared-occurrence practice PDF.
- Verify resource isolation, glyph placement and page appearance before publishing an edit. Bound expanded form traversal and direct resource copies; prune unreachable private resources after repeated edits.
- This initial subset supports standard Latin Type1 fonts, printable ASCII and verified positive scale/translation in text-only forms. Embedded fonts, complex form state, grouping and substitution remain unsupported for form occurrences.

## 0.12.0

- Add **Edit together…** for explicitly selected compatible adjacent text pieces. Check the selection, then replace the combined text in one undo step.
- Verify actual font identity, encoded text, character positions and whole-page appearance before grouping. Refuse generated or omitted spaces, uncertain ordering, marked/tagged content and unsupported text state.
- Preserve immutable sources, neighboring content, save/reopen and recovery through the existing replacement pipeline. Add a split-word practice PDF and native, UI and independent-reader coverage.
- Issue #15 now includes bounded adjacent groups; isolated nested-form occurrences remain open.

## 0.11.0

- Edit verified single text runs with rotation, scale, shear, horizontal scaling and text rise while preserving their affine placement.
- Preserve explicit character/word spacing and bounded TJ adjustments for uniquely mapped same-font edits, including spacing activated only by a new replacement. Existing font substitution remains restricted on explicitly spaced pages.
- Retain original font resources, source isolation, no-op glyph/raster validation, crop/collision checks and immutable undo/recovery sources. Ambiguous mappings and edits that move relative neighboring runs are refused.
- Add native reference-PDF comparisons, independent MuPDF/pypdf verification, embedded multibyte-font coverage, and tests for dormant spacing, shorthand operators and split streams. Issue #15 remains open for selected adjacent groups and isolated nested forms.

## 0.10.1

- Shaped text boxes can exceed 255 distinct character definitions using one bounded semantic CID font; existing smaller boxes keep their original representation.
- Plain Backspace/Delete in shaped Content removes complete graphemes, including combining marks and supplementary Unicode, while preserving native undo/redo and IME behavior.
- Native, independent-reader and browser checks cover wide text, ligature interiors, character geometry and editable/flattened save/reopen. Mixed direction and RTL word separator restrictions remain.

## 0.10.0

- Add opt-in **Shaped text** for single-line custom-font text boxes, with native outline previews, direction and ligature controls, and font validation at the authored size.
- Preserve shaped controls and exact embedded fonts through editable save/reopen and recovery. Flattened exports share native glyph placement, color and semantic text; search uses logical cluster bounds.
- Commit IME composition as one edit, retain unsupported drafts for recovery, discard stale preview work, and protect fonts through canceled requests. Share duplicate preparations, bound preview memory, and offer retry after failures.
- Preserve reader/search order on eligible rotated simple-text pages while retaining source display geometry.
- Bound malformed font character-map and shaping work; retain explicit unsupported-script and 255-definition guards. Mixed-direction text, RTL separators/joiners, whitespace-only lines and general paragraph reflow remain unsupported.

## 0.9.1

- Make coincident PDF character boxes selectable together, so ligatures and combining sequences no longer hide behind overlapping character spans.
- Preserve exact copied text, including partial ligatures and supplementary Unicode, and use the full shared source rectangle for highlights.
- Keep cross-page copying within the originating document and retain cached text measurements when switching tabs.
- Combine native UTF-16 surrogate pairs into one selectable Unicode character with the complete source bounds.

## 0.9.0

- Redesign More fonts with a searchable font list, exact-font preview and explicit Apply font action; match the print dialog in light/dark themes and smaller windows.
- Edit verified embedded TrueType fonts, including supported simple subsets and Type0 CID fonts, while preserving source placement and surrounding content.
- Preview and explicitly choose a substitute when the original subset lacks a character. Embed the substitute in the changed PDF source for export, reopening and recovery.
- Protect font previews during canceled/late loads, dialog switching and subsequent PDF imports.
- Record the complex-text shaping and reader investigation, with separate tracked stages for shaped text, positioned runs/forms and bounded wrapping. Complex-script editing remains future work.

## 0.8.0

- Choose installed fonts or import local static TrueType-outline TTF/OTF files for added text boxes through More fonts… in Properties.
- Use the exact same font bytes in previews and embedded PDF appearances; retain custom fonts through undo/redo, multiple tabs, editable save/reopen, flattened export and recovery without the original font file.
- Check embedding permissions and character coverage. Explain unsupported fonts or characters without silently substituting glyphs; complex shaping remains separate work.
- Bound font resources, share repeated font programs, release unused resources, and validate imported PDF font resources before restoring editable controls.
- Preserve overlapping text/ink order in custom-font flattened copies and clean up discarded recovery font resources.

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
