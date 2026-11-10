# Preview and apply replacement page content

Under the user's standing autonomous authorization, expose the verified native
occurrence operation as **Replace page content…** in Page actions. This completes
the replacement workflow begun in the native occurrence milestone; it does not
claim broad PDF-editor parity.

## Interaction

Freeze the selected PagePlan and full EditorDocument. Open one local PDF through
the existing native picker, select its page with a bounded numeric field, then
prepare a replacement explicitly. Show the current page and composed candidate
with the destination's current crop, rotation, and overlays. Enable Apply only
after the current candidate has rendered successfully. Explain that incoming
content is fitted to the existing page, existing comments/forms/edits remain,
and incoming annotations are not imported. Native admission errors stay visible
in the dialog. Encrypted incoming files retain the native unsupported boundary.

Cancel, changing the PDF/page, and unmount invalidate the preview. Late replies
must release their owned source; never install them in the workspace. Keep the
original source alive during native work and discard it when the dialog closes.
Do not close destination/history sources. Use existing imported-resource cleanup
for discarded opened documents. Track ownership explicitly and transfer only the
applied candidate to workspace history.

## Atomic workspace update

The adapter/IPC returns DocumentInfo and ordered pageIndices. Collect every page
using the selected source from the frozen workspace in order, and pass the index
of the selected occurrence within that collection. Validate exact mapping length,
integer indices, each mapped page's finite matching dimensions, a new source ID,
and an unchanged workspace pages array before committing. Reject a candidate that
would map the selected occurrence together with an unchanged duplicate.

Update all old-source occurrences to the candidate source and their mapped native
indices in one undo step. Retain PagePlan IDs, overlays, crop, rotation, current
bookmarks, selection, and old source ownership for undo/redo/recovery. Other source
pages retain object identity. Ignore imported candidate overlays: the workspace's
current overlay arrays are authoritative. Reusing the existing generic recovery
schema avoids adding another persistence format.

## Availability and validation

Require native replacement and composed rendering support plus destination edit
permission; repeated selected indices also require assemble permission. Native
policy remains authoritative. Disable the rest of the editor while the modal is
open. Keep close/cancel available during asynchronous preparation; stale result
cleanup must be exercised with deferred mocks.

Tests cover selected duplicate mapping, mixed sources, undo/redo and bookmark/
crop/overlay retention, stale workspace and malformed responses, success and
failure previews, choosing another page, cancel/late replies, and App integration.
Then run frontend tests/build, the native command build, and actual Windows
preview/apply/undo/save/reopen acceptance with independent output inspection.
Obtain one final scoped review and correct demonstrated consequential findings.

No commits, releases, private-source changes, or old artifact cleanup.
