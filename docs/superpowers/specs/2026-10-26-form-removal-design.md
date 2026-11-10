# Existing form field removal

## Intent and scope

Remove an unwanted logical form field and all its linked widgets from the active document, with Undo/Redo and editable save/reopen. This follows existing field-property scope: every occurrence of that source in the active document changes; other tabs and original files retain their sources. Printed labels and page artwork remain. This is not redaction or flattening.

The coordinator chooses this scope under the user's standing autonomous-work instruction. Individual-widget/radio-option removal is deferred; silently treating a radio option as an entire field is forbidden. Inspection and confirmation explicitly identify the logical name, linked appearance count, and source page numbers, including source pages not displayed by the current composition.

## Approaches

Removing just an annotation leaves logical field state behind. Flattening leaves the unwanted appearance on the page. Chosen: edit the bounded AcroForm tree, detach all selected logical-field widgets from page annotations, prune empty ancestors, and remove calculation-order entries for removed nodes. Preserve all other page streams, annotations, field values/defaults, appearances and resources.

## Contract

- Common text, checkbox, radio and choice fields use existing bounded authoring/property admission and native Parent validation. Locked, signed, XFA and malformed sources retain existing refusals.
- Inspection returns expectedKey, fullName, linkedWidgets, pageIndices and removalReason. The digest binds the exact source and selection. Apply recomputes eligibility and validates expectedKey.
- Refuse selected field/widget/ancestor identities stored as reference aliases; canonical dictionary identities remain supported. Optional arrays resolving to null are absent. Action names use the same qualified-name rule as field inspection.
- Refuse scripts/unknown action structures and actions explicitly targeting removed fields by name. Refuse any remaining reachable reference to a removed object (including actions, structure associations or annotation replies). Do not silently expand an empty action target to all fields. Untargeted or unrelated supported actions remain unchanged.
- Prune calculation order structurally, preserving remaining order; detach arrays by copying their bounded contents, leaving shared originals untouched. Traverse output references with limits before pruning unreachable objects. Existing global input/output size limits remain.
- The immutable native result preserves source pages, bookmarks, policy and original-path metadata. No original disk writes. Existing source-replacement commit owns Undo/Redo, page mappings, other tabs and candidate cleanup.
- The UI offers Remove field from form editing only with edit-content and annotate permissions, no unsaved form draft, and desktop capability. A dedicated dialog requires a deliberate confirmation button after inspection; cancellation does nothing. Clear stale selection after success.

## Verification

Native fixtures cover linked widgets, siblings, nested parents, indirect arrays, calculation order, unrelated comments and actions, actions referencing removed nodes/names, stale requests, scripts, protected sources, save/reopen and unchanged originals. Structural equality of unaffected objects plus native field inspection/render evidence checks fidelity.

Frontend checks cover scope wording, blocked/loading states, failed inspection/apply, double activation, late results/candidate cleanup, permissions/drafts, active-tab ownership and one Undo/Redo transaction. Run focused suites and frontend build, then fresh scoped review and desktop save/reopen acceptance. Preserve failed evidence and state limitations explicitly.
