# Desktop complete-source reusable form editing

Expose the verified native form transaction through existing text selection and
dialog controls. Preserve fonts, source ownership, one-step undo and complete
document connections. Native baseline: artifacts/occurrence-form-20261024/
checkpoint-native-accepted/verification.json; runtime algorithms stay unchanged.

Use a complete adapter capability pair: listOccurrenceFormTextRuns and
replaceFormTextOccurrence. ExistingTextLayer uses that pair consistently, falling
back only when the pair is unavailable. Native failure never retries isolated
editing. App freezes the selected document/page/path and commits the validated
occurrence mapping through existing commitPageReplacement. Borrowed IDs across
all tabs/history cannot be adopted or closed; abandoned fresh results are closed.
Font IDs and source-path arrays are retained exactly. Existing font leases and
dialog rejection/cancellation behavior remain in place. Tool availability includes
complete native form capability even without a legacy adapter implementation.

Add native IPC wrappers/registration and optional adapter methods, shared capability
helpers, App routing and layer listing. No new controls or layout design is needed.
Test native bridge arguments, partial-pair fallback, delayed listing cancellation,
duplicate mapping/Undo/Redo/save, borrowed IDs, malformed mapping, draft retention,
unmounted Apply and font substitution. Legacy tests explicitly opt into the legacy
route. Full frontend/types/build, one scoped review, actual Windows nested text
selection, editing a rotated/cropped duplicate, font choice, save/reopen/search,
linked fields and recovery, followed by independent saved-file comparisons.

Standing autonomous authorization applies. No commit/push/release, original
overwrite or broad parity assertion. Preserve failed evidence and old checkpoints.
