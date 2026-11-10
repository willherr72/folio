# On-page form entry

Clicking a form target currently focuses Properties. Add a compact editable card
beside the selected field so users can enter values on the document. This advances
the direct-entry gap in the quality checklist. It does not claim native-font glyph
editing: the card is a labeled draft; the existing native Apply generates the PDF
appearance. All currently supported field value kinds remain available.

The coordinator chose this workflow under the user's autonomous-work authorization.
No new native format admission, permission change or automatic save is introduced.

## Approaches

1. Independent controls in each PDF widget: naturally colocated, but duplicated
   draft ownership and rotated/NoRotate widget typography require a larger model.
2. Lift the complete form state into App: supports many surfaces, but changes the
   proven overview and asynchronous field ownership paths throughout the application.
3. Portal a compact editor from the existing FormFillEditor to the selected canvas
   target: one draft and apply path, accessible controls beside the field, and no
   native changes. Chosen by the coordinator. It is an anchored editing card, not
   a replacement for native appearance rendering or exact over-glyph typography.

## Behavior and ownership

- A canvas click/Enter/Space selects a field and opens/focuses the editing card.
  Merely entering Fill forms retains the existing initial Properties workflow.
- Properties and card share exactly one draft, error, pending state and apply/reset
  functions. Support text, multiline/password text, checkbox/radio, choices,
  multiselect and editable-combo values with the existing option-index semantics.
- Apply is explicit, one source transaction and undo step. Enter applies single-line
  text; Ctrl+Enter applies other controls; composition/IME Enter must not apply.
  Escape and Use Properties close the card while retaining the draft, and focus the
  Properties value control. Clicking elsewhere never applies or discards.
- Dirty draft blocks selecting other fields/pages' fields as before. Same-field
  reselection preserves the draft and reopens/refocuses the card. Pending Apply
  disables controls and dismissal, and duplicate application is guarded synchronously.
- Shared fields update through existing whole-source replacement. Failed Apply
  keeps the draft and permits retry; source refresh may briefly hide the card while
  fresh field metadata arrives. Do not bind old geometry to a new source version.
- Anchor identity includes adapter, page occurrence ID, source ID, source page index
  and annotation index. Position changes also invalidate the anchor measurement.
  Clearing an obsolete anchor must not clear a newer mounted anchor.
- Field target geometry remains the existing rotation/NoRotate/crop projection.
  The card is upright and uses actual screen bounds, including zoom. It is visible
  only while the owning target intersects the document viewport and window.
- Scrolling the page away or virtualizing it hides the card without losing the draft.
  Returning does not steal focus; an explicit selection request focuses it. Stale
  async field lists, another tab/source and hidden/off-crop fields cannot anchor it.
- Native rights and unsupported/read-only reasons remain enforced. Unavailable fields
  can show their explanation, with disabled value controls. Password text remains
  masked in both surfaces; no draft content goes into logs or local persistence.
- Overview remains a separate modal opened only with a clean draft. No overlapping
  modal/focus traps. Properties retains its persistent subtree at narrow widths.

## Placement and accessibility

The card is a nonmodal labeled region. Its input labels begin with `On page:` to
distinguish the two editing surfaces. Its actions are `Apply field on page`,
`Reset field on page`, and `Use Properties`. It announces that changes are drafts
until applied. Use the same value-control component in Properties and the card.

Clamp width to at most 320 CSS pixels and the window minus 24 pixels; keep a 12-pixel
window margin. Prefer below the visible target, then above, and otherwise clamp
vertically. Limit height to the window minus 24 pixels and allow internal scrolling.
Re-measure on scroll, resize, anchor/SVG/viewport resize, content resize and geometry
changes. Coalesce observation work in requestAnimationFrame; cancel observers and
pending frames on unmount. Focus a surviving Properties control if focused card
content disappears; do not steal focus from another intentional destination.

## Acceptance

Pure placement cases: all edges, clipped/hidden targets, small windows, large content.
React: shared draft/reset, all value kinds, unavailable/password fields, IME/keyboard,
same-field reopen, ownership/late clear, stale loads, pending failure/retry, and cleanup.
Existing canvas/overview/permission/undo/form tests retain their behavioral assertions.
Full frontend and TypeScript/build; native form regression as unchanged save/fill gate.
One scoped implementation review. Windows: click/type/apply/reset, failed/unchanged
paths as available, shared fields, zoom/scroll and 900x600 keyboard behavior; save and
reopen with an independent reader checking values and unchanged surrounding pixels.
Keep original files and prior checkpoints; no commit, release or parity claim.
