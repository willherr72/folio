# Folio editor upgrades
Authorization: user explicitly requests a real app modal, drawing, Ctrl+wheel zoom, removal of the engine badge, settings, dark mode and continuous scrolling; existing autonomous implementation preference remains in effect. Routine reversible design choices are delegated.

## Design
Keep native Rust/PDFium export and the existing ink/text payload. Add a Draw tool storing one vector ink overlay per completed stroke, undoable as one action. Pointer capture keeps movement smooth; cancelled strokes disappear. Page-specific callbacks prevent edits landing on the wrong page during scrolling. Pen color and width remain editable and persist as defaults.

Replace window.confirm for opening/demo/desktop close with centered, accessible app dialogs. Trap focus, Escape cancels, restore focus, and suppress editor shortcuts behind any modal. Native close must await the decision. Cancel never changes the current document. Browser-only reload/unload retains browser protection where custom modal is impossible.

Settings: theme System/Light/Dark (default System), page view Continuous/Single (default Continuous), default zoom (90%), pen color (#2D2A26), pen width (2 pt), and reset defaults. Persist validated preferences locally, outside document history. Dark chrome leaves document pages white with original PDF colors.

Continuous viewport stacks correctly sized pages, lazily renders nearby full-size pages and releases offscreen full-size renders. Scrolling updates selected page and properties without scroll jumps. Thumbnail/navigation clicks scroll to the target. Editing an unselected visible page must select/edit that page. Existing page reorder/rotation/duplicate/delete remain correct.

Ctrl+wheel over the document adjusts PDF zoom (50..200%), suppresses WebView/browser zoom and keeps the page point under the pointer approximately anchored. Unmodified wheel scrolls normally. Toolbar zoom uses the same scale.

The engine status badge and its normal-path fetch are removed. Actual render/export failures remain visible.

## Validation
Failing tests first for modal cancellation/focus/shortcut isolation, persisted preferences, drawing/history, rotated coordinates, and multi-page routing. Browser smoke covers continuous scroll, page2 editing, drawing save-plan contents, Ctrl+wheel anchoring, dark/light and reload persistence. Native smoke verifies saved vector ink and app modal behavior. Independent review, release build, actual packaged-app check. Preserve the running first version; deliver a versioned release folder and update root launcher after verification.
