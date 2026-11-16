# Desktop workspace Replace

The next user-facing milestone is Find and Replace in the current document, with a checked preview, explicit Apply/Cancel, and one Undo action. It must include imported and duplicated page occurrences and editable text overlays while retaining workspace metadata, bookmarks, crops, rotations, source protection and other annotations.

## Design decisions

Capture the immutable editor document and its history/adapter/tab identity. Do not compose Save and reopen as a snapshot: that would bake reversible crops and change editable overlay ownership. Stage native edits against each original source using complete-source occurrence editing. Keep local page IDs and transformations; adopt only new final sources. Preview and Apply consume the same validated mapping. Global source ownership includes other tabs and older Undo versions.

Native discovery uses the existing literal Unicode matching and trusted source-bound replacement planner. Duplicate occurrences remain independently selectable. Added text is a separate logical matching scope, with its existing font and area layout checked before preview. Blocked selected matches prevent the whole batch; never silently replace a subset. Existing native limits remain: 64 changed text objects, 32 changed pages, 512 KiB actual batch strings, 10,000 pages/matches and bounded responses. Empty native-object replacement and unsupported layout/font cases retain their explicit refusal.

The desktop host owns pending source candidates and resource leases by registered window/session. Cancel, stale responses and window destruction release owned candidates; they never close borrowed source IDs. Apply transfers only final candidates to the document session. Undo/Redo retain both original and candidate source ownership.

## Delivery sequence

1. A pure workspace transaction contract with real tests for capture, atomic validation, ownership, preservation and Undo. This is an internal foundation; it does not expose a premature Replace control.
2. Native current-workspace planning/staging and a window-owned lifecycle, with stale/cancel/late-failure tests.
3. Search entry and accessible desktop dialog: literal query/replacement/options, match selection/blockers, affected-page previews, Apply/Cancel and one Undo. Test races, narrow layout and focus.
4. Appropriate fresh frontend/native gates, normal build, actual Windows workflow and independently checked saved PDFs; one scoped review and evidence checkpoint. Component progress does not certify commercial parity or update a package/launcher.

## Transaction contract

`captureWorkspaceReplacement(session, borrowedSourceIds)` returns a frozen cloned document plus original tab, adapter and exact history reference. It rejects empty/oversized workspaces and invalid page identities. `workspaceReplacementPages(request, result, borrowedSourceIds)` validates all final source mappings and overlay patches before producing pages. `commitWorkspaceReplacement(session, request, result, borrowedSourceIds)` requires the captured state, adopts unique fresh candidate IDs and commits once. Unchanged sources/overlays retain their model identity; metadata/bookmarks remain from the current guarded document, not candidate import metadata. Candidate aliases, protection changes, missing/conflated/swapped original pages, stale overlay text and duplicate patch targets reject the entire result.

The result describes final source versions with one index per captured occurrence, plus page-qualified text-overlay edits. A new mapped index may retain the original index or refer to an appended occurrence clone; distinct original page indices must never collapse onto one output index. Native host ownership and the bound preview remain authoritative for edit content; frontend validation is an additional boundary, not a PDF proof.

## Constraints and deferred acceptance

Work inline in the existing dirty isolated worktree. Preserve originals, prior evidence, packages and launcher. No commit, cleanup, version/package change, clipboard probe or new private input. Serialize native work and use the existing build target. Keep failure evidence and exact validation limits. No fresh GUI or native acceptance is claimed by transaction-only tests.
