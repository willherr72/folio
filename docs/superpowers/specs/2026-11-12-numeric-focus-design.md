# Precise numeric field focus

Tab and Shift+Tab into a precise numeric field must allow typing a replacement, as an ordinary number input does in the same Windows WebView. The field must expose the exact draft before selection is restored. Pointer focus must retain native caret placement. Focus and blur must not emit edits or round the owner's draft.

The existing synchronous compact-to-exact value update collapses native selection. Restore selection after expansion for non-pointer focus. Track pointer focus locally and compose existing pointer/focus handlers; clear pointer intent on completion, cancellation and blur. Retain type=number, bounds, step, read-only behavior and external updates. Consumer onFocus runs after repair and can override selection.

Alternatives considered: retaining compact values while editing hides precision; replacing the native number control changes keyboard and accessibility semantics. The local focus repair is the smallest change addressing the observed cause.

Evidence: artifacts/numeric-keyboard-observation-20261112/desktop-1/result.json contains actual Tab/Shift+Tab failures beside ordinary-number successes in the same WebView. No PDF was modified.

Acceptance: focused unit regressions fail before the fix and pass afterwards; the full frontend suite and build pass; a freshly built Windows candidate passes actual Tab, Shift+Tab, programmatic focus and pointer editing checks. Keep selected portable 048 unchanged. A component candidate is not a newly qualified portable release.
