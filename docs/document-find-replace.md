# Document Find and Replace

This workflow is verified in the current source test build and is being integrated
into the separate 0.53 development portable. The completed 0.52 portable and selected
0.50 retain their own recorded features; packaging does not replace the launcher.

Open a PDF and choose **Find** (Ctrl+F). Enter literal text; Match case and Whole
words control matching. Enter/F3 and Shift+Enter/Shift+F3 navigate matches. Search
state stays with each tab. Ctrl+F returns focus to the existing query.

For replacement:

1. Choose **Replace text** from Find, or Ctrl+H in the document workspace.
2. Enter Find text and Replace with, choose matching options and **Check matches**.
3. Review the page/match list. A blocked selected match prevents the whole batch.
   Choose a subset explicitly and check matches again after changing selection.
4. Choose **Preview replacements**. Inspect the Before/After images; use Preview
   page to view the affected pages. Apply becomes available after the shown pair
   loads. Changing the query, options or selection invalidates the preview.
5. Choose **Apply replacements** to commit one document Undo action. **Cancel**
   discards the pending result. Undo/Redo retain the earlier and applied versions.

Replacement includes imported and duplicate page occurrences and editable added
text. Search considers the current visible page area. Local crop/rotation,
workspace metadata/bookmarks, unrelated annotations and native protection remain
part of the preserved document. Original source files are not overwritten.

Matching is literal Unicode text, including case-fold matching where enabled;
replacement strings do not interpret regular-expression captures. Native font,
layout, neighboring-content and permission checks still govern existing PDF text.
If an embedded subset cannot represent a replacement, use the existing Edit text
or paragraph font-choice workflow for that edit. Form answers use Fill forms.
An empty replacement may remove added text; deleting a whole existing text object
can require the separate Remove text workflow.

One request is bounded to 64 changed text objects on 32 pages and 512 KiB actual batch
strings. Larger selections need smaller checked subsets. Blocked selections do
not become silent partial success. Expired previews must be created again.

The desktop edits the current workspace, including unsaved arrangement and added
text. [CLI/MCP automation](automation.md) operates on saved PDFs through explicit
inspection/planning, preview, Apply and save. Its validated source selections and
native limits remain in force.

Compiled tests cover Apply/Cancel/Undo and cancellation after native Apply but
before its reply, narrow previews, protected saves and independent saved-output
checks. Physical system Ctrl+V/installed IME, broader producer/font/reader/DPI and
maximum-size performance remain separate acceptance work. No Adobe/Foxit parity
or public-release certification is implied.
