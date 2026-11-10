# Complete-source desktop text edits

Connect the accepted native occurrence text APIs to the existing desktop Edit
text selection and single-run dialog. This removes the shared-field isolation
refusal for supported top-level runs without changing the editing experience.

Use new optional adapter methods as a capability pair: `listOccurrenceTextRuns`
and `replaceTextOccurrence`. Canvas inspection prefers the pair; legacy adapters
retain their existing path. Nested-form, group and paragraph operations retain
their current APIs and limits. Do not silently retry a failed full-source edit
through the old isolated path.

Capture the document/pages at selection. Apply sends all same-source occurrences
and the selected occurrence through native IPC. Reuse `pageReplacementRequest`
and `commitPageReplacement` to validate mapping, geometry, rights and frozen
page identity, remap all same-source current pages, and commit one history step.
Keep overlays, crop, rotation, page IDs, bookmarks and older source versions.
Reject any returned source already owned by any tab/history; never dispose such
a borrowed source. Dispose fresh results on malformed/stale/unmounted responses.
Keep draft input after failures and block workspace mutations during Apply.

Alternative: a new edit-only remapping model would duplicate the now-tested page
replacement invariants. Reusing the existing mapping is the implementer's choice
under standing autonomous authorization. No confirmation gate or commit/release.

Acceptance: frontend protocol/selection/App ownership/undo regressions, existing
suite and typecheck, one final scoped review, debug build and actual Windows
shared-field duplicate workflow: edit, re-edit, undo/redo, save/reopen, field fill
and saved linked values, controlled recovery. Independent readers must confirm
text and interactions. Native engine acceptance remains the preceding checkpoint;
this delta adds IPC wiring only. No parity claim.
