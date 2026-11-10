# Responsive editing properties

User authorized autonomous iteration and opted out of routine design approvals.
Previous cycle made verified progress (active-tab visibility).

## Design

At <=850px, replace the hidden properties sidebar with a collapsible bottom panel.
Keep the same editor DOM mounted when resizing so drafts, composition, selection
and focus survive. On wider windows retain the right sidebar. Use a labeled
toggle with aria-expanded/controls, independently scrollable content, and an
accessible Properties region. Selecting a different object opens controls; reading
without a selection starts collapsed. Users may collapse controls for page space.
Focus entering content reopens the narrow panel, including after wide editing.
State is scoped to document/selection identity; no PDF or recovery-schema changes.

Chosen approach: responsive grid placement of one persistent component, avoiding
duplicate form controls and reparenting. A modal drawer would obscure the document
and introduce focus trapping; permanently squeezing three columns leaves too
little page space. Existing Review and page controls remain reachable in the panel.
Desktop remains the product target; check 640x720 and 480x640 layouts as well as
wide editing. Do not claim all mobile/tablet interaction support.

## Tasks (inline execution)

- [x] Reproduce inaccessible Content control in a rendered narrow window.
- [x] Add PropertiesPanel component and focused behavior tests; replace only the
  existing aside wrapper in App.tsx. Update responsive CSS, preserving desktop.
- [x] Browser checks: new/existing text and comments, collapsed/expanded keyboard
  access, scrolling to final controls, resizing while typing/composing, selection
  changes, tab isolation, and no window horizontal overflow at target sizes.
- [x] Scoped review, full frontend/typecheck/build, actual Windows editing at narrow
  widths with save/reopen and original-file preservation. Retain unchanged native
  result explicitly. Reconcile review findings and record screenshots/hashes.
- [x] Update quality ledger and checkpoint. Overall parity remains unproven.

Files: new src/components/PropertiesPanel.tsx, src/App.tsx, src/styles.css;
new tests/properties-panel.test.tsx and browser acceptance script; Windows script.

Final checkpoint: `artifacts/responsive-properties-verified-20261023/verification.json`.
756 frontend tests / 108 files; eight browser and eight Windows flows pass. Review
found a stale collapse preference on context re-entry; failing regression captured
and reset logic corrected without remounting children. Earlier build preserved.
Text-edit checks concern overlay properties; original-PDF text dialogs are unchanged.
Composition is synthetic, not actual Windows IME acceptance. Independent MuPDF
checks saved text/comment, original text and untouched second-page text/pixels.
