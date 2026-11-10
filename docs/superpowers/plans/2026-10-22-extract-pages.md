# Extract pages implementation

Use superpowers:executing-plans inline; user authorized execution. Spec:
../specs/2026-10-22-extract-pages-design.md

1. Share page-selection parser with printing. Test ranges and ExtractPagesDialog
   validation, current/all modes, draft retention and busy behavior RED -> GREEN.
2. Integrate native Save options/Page actions, modal exclusion and export handler.
   App regression: unsaved edits survive extract, export uses current order/subset,
   all-pages extraction does not mark saved, cancelled/failed exports retain range.
3. Native subset export/reopen test for mixed sources, rotation and editable overlays.
   Run targeted native suite, full frontend suite and build. Browser acceptance and
   one fresh review; fix material findings with regression tests. Build checkpoint.

Review focus: no saved-digest/history mutation; no duplicate submissions or close
while pending; extracted plans include immutable edited sources and unsaved overlays;
range limits and current order; save cancellation/retry; source overwrite protection.
No commit/push/merge needed. Existing worktree changes must remain intact.
