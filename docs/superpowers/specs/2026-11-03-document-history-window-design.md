# Document history window

Keep the nearest 100 document actions per tab. A new edit records its prior document, removes older past entries, and clears Redo. Undo and Redo move within that same window; they never alter a retained document. No-op edits retain their history identity and Redo. A completed overlay drag counts as one action, including a drag finalized by switching tabs.

The current document and saved digest remain independent of eviction. Source, font, and image cleanup must continue using all retained past/present/future states and recovery leases. Eviction makes an old native version eligible for the existing source cleanup; it must not close a version used by another tab or recovery writer.

The Undo tooltip communicates the 100-action policy. This is a count bound, not a total-memory budget. Recovery continues to restore the current document without Undo history. No portable publication or unrelated changes are in scope.

Acceptance: model rollover and branch/no-op tests; actual App gesture completion paths; source ownership at rollover; full frontend tests/build; a real desktop chain exceeding 100 native edits, exact Undo/Redo endpoints, early source release, saved-PDF fidelity, and owned-process cleanup. Reuse unchanged native evidence only after input equivalence checks.
