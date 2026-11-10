# Form filling on the document canvas

The current Fill forms modal reproduces the page in a second preview and blocks
ordinary zoom/navigation. Native field editing already preserves shared fields,
source identity, undo, recovery and save/reopen. This change makes the main
document the field-selection surface and places persistent value controls in the
responsive Properties panel. It does not change native form publication.

## Approach and tradeoffs

The coordinator chooses canvas field targets plus a persistent value panel. This
uses the existing viewport (zoom, virtualization, rotation and crop) and accessible
HTML controls without pretending browser typography is the final PDF appearance.
Direct HTML inputs over every widget would need an additional layout/typography
engine and are a later enhancement. Merely opening the old modal from canvas
clicks would retain the duplicated-preview workflow and is not the intended result.

The existing full-source form preview remains available as an explicit overview,
including fields outside a workspace crop. The main canvas must never expose
invisible keyboard targets outside its visible crop.

## State, permissions and behavior

- Fill forms enters a dedicated canvas interaction mode. Each mounted page loads
  fields for its actual source/page and adapter. Stale replies never install
  targets. Hidden/invalid/outside widgets have no target; readonly fields remain
  inspectable. Fill permission is independent of annotate/edit-content permission.
- Selecting a target by pointer, Enter or Space selects its occurrence and focuses
  the appropriate value control. All existing text, multiline, password, checkbox,
  radio, choice/multi-choice and editable-combo behavior remains available.
- One form session owns a captured tab, document, page and field selection. The
  Properties editor remains mounted when the user scrolls or navigates away from
  its page; a Show selected field action returns to it. Zoom and read navigation
  remain usable while a draft exists. Choosing a different field requires Apply
  or Reset. Clicking the same field focuses without resetting its draft.
- Mutating document actions, tab switching/closing, Save and other editing tools
  remain blocked during a form session, matching prior modal draft protection.
  Done/discard exits; then Undo/Redo and save operate normally. Keyboard shortcuts
  must respect these same guards. Busy native publication blocks interaction.
- Applying uses the existing whole-source transaction and expected original
  field value. Shared widgets and duplicate pages refresh together. Publication
  failure preserves the draft. Late unmounted results are released; foreign or
  stale source ownership must not be accepted. No change to permission semantics,
  native actions/calculations or unsupported XFA/signed-form handling.
- Crop and workspace rotation affect only target placement. Reuse verified field
  geometry and NoRotate behavior; clip displayed bounds to the visible page.
  Native intrinsic rotation remains represented by returned field coordinates.
- Responsive properties automatically open for a selected field, keep one editor
  subtree, and expose controls and Apply/Reset/Done at narrow desktop widths.

## Acceptance

Behavioral tests cover no default modal, main-canvas targets, every supported
control family, correct page/field ownership, dirty navigation and reset, stale
loads, pending apply, failures/unmount, shared source updates, permission-only
fill access, crop/rotation/NoRotate, and keyboard focus. Existing overview tests
must still pass. Full frontend tests and build are required.

Run an isolated Windows executable through main-canvas selection, actual form
updates, all supported field types, zoom/navigation, Undo/Redo, save/reopen and
shared widgets. Check saved values and appearance with an independent reader.
Review one final scoped implementation and retain explicit gaps rather than
claiming full form authoring, scripts, signing, XFA or product parity.
