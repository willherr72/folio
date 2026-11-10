# Existing form-widget position and size

Continue the everyday editor goal by allowing corrections to a field's layout after
creation, including third-party fields and individual radio choices. Standing user
authorization permits implementation without routine questions. This is the
coordinator's design choice, not an assertion of commercial parity.

Use the existing immutable complete-source occurrence transaction. Select a widget
in Fill forms, open Position and size, change its displayed point coordinates with
numeric controls, mouse dragging or keyboard handles, preview, then Apply once.
Cancel discards candidates; Undo/Redo restores the entire source. A duplicated page
copy is isolated using the existing page-copy mapping. Other distinct widgets of a
shared field retain their rectangles, field identity and value behavior.

Preserve the field tree, actions, fonts, flags, values and all appearance streams.
Change only the selected widget's Rect. Moving preserves appearance; resizing scales
its saved appearance. Explain that later filling uses its original text settings.
Reflowing field text, editing definitions and rotating widgets remain subsequent work.
An alternative native regeneration would reflow but can regenerate unrelated widget
appearances and fonts; defer that until a source-bound appearance transplantation
path is verified. Recreating the field would lose identity/actions and is unsuitable.

Public request: annotationIndex, expectedBounds and expectedValue, bounds. Bounds
are source displayed physical points; workspace rotation is converted by the UI.
Require finite nonnegative coordinates, minimum 4 pt dimensions, complete placement
inside the source page and the workspace crop. Native code verifies original bounds
within 0.01 pt and exact normalized expected value before mutation. Enforce edit and
annotation permissions, assembly permission through existing duplication handling.

Use existing bounded form graph admission and bounded selected-widget inspection.
Permit text, checkbox, radio, combo and list; retain readonly filling flags. Refuse
hidden, NoRotate/NoZoom/locked widgets, missing saved appearances, and a single widget
object placed in multiple annotation slots until isolation is explicitly supported.
Distinct sibling widgets sharing a parent are supported. These refusals must be
actionable and must not mutate input or publish a partial candidate. No scripts run.

Verify native positions over intrinsic rotations, nonzero crop origins and UserUnit;
shared fields, duplicate occurrences, exact field/AP preservation, fill-after-move,
save/reopen and protected inputs. UI verifies stale preview invalidation, late result
cleanup, borrowed sources, dirty-value gating, keyboard/crop behavior and Undo.
Windows acceptance and independent-reader geometry/pixels remain required before a
verified executable is advertised. One final scoped reviewer, parent red/green fixes.
