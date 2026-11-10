# Interactive PDF form filling

User intent: continue toward everyday Adobe/Foxit usefulness autonomously. The
preservation prerequisite is complete; users must now be able to fill existing
AcroForm text, checkbox, radio and choice fields without flattening them or losing
shared identity. This is a new subsystem, implemented in the existing worktree.

## Evidence and approach

Throwaway artifacts/probe-form-fill.py calls the exact bundled PDFium library.
Focus, select-all, replace-selection and blur update the value AND appearance of
both widgets of a shared text field. MuPDF and pypdf read the resulting value and
MuPDF extracts the new visible text on both pages. This is the preferred approach.
Direct V dictionary edits leave stale appearances; recreating appearances ourselves
would duplicate native layout/font behavior and reduce fidelity.

Expose native inspection and filling on an immutable source. Use PDFium's existing
thread-safe bindings through a retained borrowed pointer at the single engine
initialization point (its global owns the original Box for process lifetime).
Keep raw form/document/page/annotation handles in RAII wrappers, with the bytes and
pinned FPDF_FORMFILLINFO alive until close. No JavaScript platform or action/file/
network callbacks. Null callbacks alone do not disable internal ResetForm/Hide
actions: temporarily replace recognized action S names with unique unknown types
without dropping their graph, then restore and count-check them after saving.
Read native flags/options; preserve custom checkbox/radio states
through native input events. Validate the exact committed value before publication.

## Workflow and ownership

A Fill forms toolbar action opens a keyboard-accessible dialog for the selected
page. List fields with names/labels and native values; text inputs/textarea,
checkboxes, radio selection and select controls reflect field type and flags.
Editable combos offer index-based option selection separately from custom text;
display labels never substitute for export values.
Explain unsupported/read-only fields and that calculations/scripts do not run.
Applying one field creates a new whole-source version. Every current page using
that source switches together, preserving page IDs, order, rotation, overlays and
bookmark targets. Undo/redo and recovery retain both sources. A stale/unmounted
request releases the candidate rather than committing it to another tab.

Source bytes include original imported annotation appearances. Candidate opening
must strip the same imported annotations from the runtime render, while retaining
the edited raw bytes for export/recovery and the existing workspace overlays.
Do not reimport overlays or reset bookmarks when committing a source replacement.

## Correctness gates

Bound graph/string/options/page loading and validate existing form hierarchy before
editing. Reject signed/XFA rewriting, read-only fields, unsupported rich-text/file
selection fields, invalid option indices, stale expected values, embedded NUL and
truncation. Never mutate the original source or publish a failed candidate.
Shared widgets must all have up-to-date appearances. Required fields can be cleared
while drafting (required status displayed; submission is outside this workflow).
PDF scripts are preserved but never executed; calculated/validated results are not
claimed. Native API failures must yield useful errors, not silent success.

Test text (Unicode, multiline, length limits), custom-state checkbox, radio group,
choice export/display labels, shared widgets across pages, readonly/malformed,
source immutability and reopen/export. Frontend tests cover stale ownership,
multi-page source replacement, overlay/bookmark preservation, undo/redo and controls.
Final verification uses actual Windows UI plus MuPDF/pypdf saved values and rendered
appearances; original source hash and unchanged surrounding page pixels checked.

Overall parity remains open: this milestone does not implement form design,
JavaScript execution, XFA, signing or submission.
