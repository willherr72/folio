# Paragraph workspace Find keyboard ownership

Folio's paragraph editor already has local literal Find/Replace, draft history,
native Preview/Apply, and nested font selection. `ParagraphTextEditor` handles
Ctrl+F/H and F3 only inside its own DOM wrapper. Font, alignment and area controls
are siblings. After formatting, those shortcuts stop working even though the
same paragraph workspace owns the draft. Escape alone already has wider scope.

Make the paragraph workspace/dialog the keyboard owner for Ctrl+F/H, open-Find
F3/Shift+F3, and first Escape. Keep current search/match/replacement semantics.
Opening Find and moving among matches must preserve text, styles, draft history
and native preview. Return focus and selection to the paragraph input when
navigating or closing Find. Prevent duplicate dispatch from the input wrapper.
Recognize F3 only without Ctrl/Meta/Alt; Shift remains previous-match navigation.
Composition/keyCode229, disabled gestures, and prevented events retain ownership.
Nested portal dialogs keep their own shortcuts and Escape. Support both the
on-page workspace and fallback modal, plus a standalone editor wrapper fallback.

Use a native capture listener attached to the nearest paragraph workspace/modal,
with render-current handlers and cleanup on scope/unmount changes. This follows
the existing Escape ownership model while consolidating keyboard handling.
No global document/window Find hook; no app-wide search or font/native changes.

Required evidence: failing sibling-control shortcut tests before implementation;
passing focused/editor integration and full frontend suites at repository default
5000ms/two workers; normal optimized GUI/source/resource-bound build; actual
compiled Windows sibling-control navigation, nested modal isolation, Preview/
Apply/Undo/Redo/save/reopen with an independent public-PDF audit. Preserve the
reviewed051 source/package and selected050 launcher. No clipboard writes or
systemCtrlV/IME claim. Native source/shared-input equality permits explicitly
qualified prior native evidence, never a fresh all-native claim.

Existing managed worktree; no commit/merge/push/deletion/new portable selection.
Use public fixtures. Serialize builds and desktop tests, one Cargo job with the
existing D: cache/temp. One final fresh-context component review and one Important
RED/GREEN fix pass; no re-review. Keep broad goal active after this component.
