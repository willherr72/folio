# Changelog

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
