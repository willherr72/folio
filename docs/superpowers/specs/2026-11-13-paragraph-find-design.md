# Find and replace in the current paragraph

The broad goal is a useful desktop PDF editor. The existing styled paragraph
workspace supports literal text editing, local Undo and native Preview/Apply,
but offers no way to navigate or replace repeated text in its draft. Add an
explicit current-paragraph Find and replace panel. The user previously opted
out of design questions; these are executor choices, not newly approved details.

## Approaches

Whole-document replace would need a separate multi-page native transaction,
permission/admission decisions and review of each changed area. Browser find
cannot replace or understand the immutable styled draft. Use a draft-owned
paragraph panel now, preserving the eventual native publication route.

## Behavior

Find literal, non-overlapping matches across style runs, including LF hard
breaks. Ignore case by default with Unicode JavaScript simple case folding;
Match case and Whole words are explicit options. Match endpoints must be whole
grapheme boundaries. Whole words use Intl.Segmenter word boundaries and require
word-like content, not ASCII-only word classes. Empty query yields no matches.
CRLF/CR normalize to LF before searching/replacing; no regex syntax or $ expansion.
The scope label says Current paragraph. Query/replacement are bounded by the
existing4096UTF8byte capacity; malformed Unicode/unsafe controls reject without
changing the draft. No Unicode normalization of source text is introduced.

Next/Previous wrap, select the exact matched range and focus the draft so it
is visible. Ctrl+F/Ctrl+H open/focus the local panel while editing; F3/Shift+F3
navigate. Enter in Find navigates, Shift+Enter reverses. Escape first closes
Find and restores paragraph focus without cancelling the whole edit. Native
composition events are left alone. A visible button offers the same actions.
Selection navigation does not invalidate an existing PDF preview or add Undo.

Replace changes only a currently selected exact match, selects/navigates the
next match, and is disabled otherwise. Replace all applies the current match
set atomically against the current immutable draft. It preserves every untouched
style span; inserted text inherits the first matched grapheme's style. Each
Replace or Replace all is one local Undo step, including selection restoration.
No-op replacement must not add history. Final capacity/grapheme/style checks
remain4096UTF8bytes/64styles; no intermediate growth or partial failure.
Derived fonts/colors/highlights, original source identity, baseline/frame and
paragraph settings remain bound to the existing editor. A replacement that joins
a differently styled grapheme may refuse safely; do not split clusters.

Busy/dragging disables replacement, stale matches are recomputed from current
state, and a new original source clears Find state. Font picker/modal composition
must not be intercepted. Native fit/font eligibility still decides Preview.
No PDF change occurs before existing Apply; Cancel/document Undo remain intact.
No version, dependency, native API, renderer, permission or selected portable change.
System clipboard transport is unrelated; previous realCtrl+V gate stays open.

## Verification

Pure model tests cover Unicode/mixed styles, literal regex-like characters,
word/case/linebreak semantics, navigation, first-style inheritance, formatting
and source metadata, oneUndoRedo, no-op, final capacity, invalid/overlap/stale
ranges and final joined graphemes. UI tests cover shortcuts/focus/Escape,
recomputation, disabled replacement, busy state and preview invalidation only
on actual text changes. Fresh full frontend, normal optimized GUI, actual owned
Windows keyboard workflow on a public authored PDF, independent saved text/font
and unaffected pixels. Final single fresh reviewer, oneImportantfixpass, no re-review.
