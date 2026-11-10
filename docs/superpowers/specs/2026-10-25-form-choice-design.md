# Existing dropdown and list choices

Design begun during the recovery audit, under the user's standing authorization
for autonomous progress. That full native gate is now complete and verified.
Probe evidence and remaining uncertainties
are in `artifacts/form-choice-recon-20261025/findings.md`.

## User behavior

Add **Edit choices…** to selected existing dropdown/list fields in Fill forms.
Show ordered rows with display text and stored export value, Add/Remove and Move
up/down controls, current selection and default selection. Labels may repeat;
identify rows independently of their text. Explain the scope when widgets share
one logical field. An editable dropdown's free-text answer is separate from an
option selection. Keep the current value draft gate used by Field properties.

Editing a label preserves that option's selected identity. Reordering remaps its
index. Changing the export value of a selected option updates its stored answer
and must be visible in the draft summary. Removing a selected/default option
requires choosing a replacement or explicitly clearing that selection before
Preview/Apply. Do not silently select the next row. Clearing a selection is valid
even for a required field; the Required flag remains unchanged.

Allow zero options where the source field semantics permit it. Do not convert
between list/dropdown, single/multiple choice or editable/noneditable in this
milestone. Preserve original flags except an explicit manual-order choice clearing
Sort. If the source requests sorted options, changing its rows requires that
visible opt-in; unchanged rows preserve Sort. Do not silently apply locale-dependent
sorting or reinterpret row identities.

Defaults describe reset behavior, not the current answer. Preserve unmodified
raw defaults. A duplicate export value can represent several rows and has no
separate default-index array: show that ambiguity rather than inventing a unique
default row. If a removed/changed option leaves a default value without a valid
meaning, require an explicit default change. Editable free-text defaults remain
distinct from row choices.

Native probing exposed a further distinction: a dropdown with absent V may display
its DV even though no option is selected. Keep preserves that raw absence/default
state. Changing its default requires explicitly choosing or clearing the current
answer, so a default edit cannot silently change the visible answer. Explicit null
V masks this fallback. Clearing a displayed fallback requires repainting even when
the abstract selected-row state was already empty. The private native generator
reads the staged PDF Context directly and copies V/I/DV operands including absence;
it must not replace an absent V with null merely to normalize the model.

Use a real native preview with page navigation for linked widgets, Cancel, Reset,
and Apply. Editing any draft invalidates the old preview. Apply publishes exactly
the inspected candidate as one Undo step. Undo/Redo, save/reopen and crash recovery
must retain the choices, answers, defaults and shared field identity. Preserve
keyboard focus across preview and row moves; keep the dialog usable at 640px.

## Native ownership and model

Use a separate choice transaction, not the metadata-only form-properties endpoint.
Inspection binds complete source bytes, logical field owner and the selected page
widget to an expected key. Read the bounded field tree; do not trust Parent alone.
Expose original row indices as source-bound identities, display/export strings,
current selection, defaults, list top index, relevant flags and linked widgets.
Requests reference each original row at most once; inserted rows have no original
index. Never accept client-supplied PDF object IDs or font-resource identities.

Decode raw strings only after charging byte budgets. Handle PDFDocEncoding,
UTF-16 and the existing UTF-8 BOM convention consistently. Preserve unchanged
option objects/representations where possible. Keep the existing 2,000-option
inspection ceiling and aggregate inspection budget; do not silently truncate or
reduce an existing document to the 256-option authoring default. The model bounds
each string to4,096UTF-8bytes and total option/current/default text to128KiB in
source, draft and resolved output independently. Raw PDF inspection must charge
encoded bytes before decoding and remain within its own bounded scan budget.
The model does not replace graph/decompression/copy or raw decoding limits.

Read-only PDF inspection is implemented separately. It preserves explicit null
as an inheritance terminator and refuses missing/mismatched hierarchy Parent
links for choices. Current null plus stale nonempty indices is a disagreement,
not a missing value that authorizes those indices. A blank noneditable current
value/default without a blank option means no selection/default. Numeric legacy
I is admitted when valid; array indices must be ascending and unique. Dropdowns
ignore list-only MultiSelect/TI in the effective model, retaining the raw source.
Raw strings are bounded to8194bytes and512KiB aggregate before decoding; decoded
retained text is charged to the128KiB model budget. Invalid UTF-16/UTF-8 is refused.

Confirm V/I agreement with native interpretation. Inconsistent or ambiguous
current selection must not be silently repaired. Reorder indices in ascending
order and retain the selected rows' corresponding export values. Keep the list's
first visible row by identity where it survives; define a bounded replacement
when that row is removed. Apply changes at the logical owner, isolating inherited
or aliased option arrays from unrelated fields. Preserve all other values,
actions, names, flags, geometry, annotations and source security policy.

## Appearance strategy and prerequisites

The selected strategy is an isolated per-widget native generator with the actual
widget's font context, followed by replacement of only its text appearance content
in the original graph. Do not publish the generator's document or global flags.
Full-widget regeneration demonstrably erases custom artwork in the probe.

The first acceptance path requires an unambiguous balanced Tx block and a proven
graphics-state boundary. Preserve drawing before/after it, resource aliases,
BBox/Matrix, clipping and annotation appearances other than N. Verify unchanged
regeneration before permitting edits; inspect generated resource graphs with the
existing bounded Copier/Equivalence machinery. Bind generated font resources to
the exact original font graph, including encoding, metrics and embedded program.
Never assume matching font names prove identity. Missing glyphs or fallback fonts
must be reported, not silently substituted. Explicit substitution is a separate
future capability unless needed to finish a coherent first implementation.

The controlled probes establish exact no-op pixels for standard Helvetica and
full embedded WinAnsi DejaVuSerif; empty dropdowns, editable free text and lists
generate appearances. The artwork and embedded-font probes are separate. They do
not yet establish combined resource transplantation, subsets, Type0 fonts,
divergent widget fonts or arbitrary transformed/marked-content appearances.

The original document's NeedAppearances policy remains untouched. The private
probe needs that flag to generate missing appearances, but retaining it in the
probe output made one independent reader show an export value as the label.
Original documents requesting global regeneration need an explicit supported
path with preserved unrelated-field behavior, or a precise availability reason;
blindly clearing the flag is not an acceptable shortcut. Resolve this admission
rule, graphics-state proof and font/resource transplantation before appearance
implementation and feature acceptance. The independent value model can proceed
under its separate plan. Metadata-only default changes should not needlessly
regenerate pixels.

## Integration and evidence

Follow the existing complete-source candidate, producing-adapter cleanup, stale
session refusal and commitFormReplacement history pattern. Retain all linked
workspace occurrences and existing field values. Native checks enforce editing
and annotation permissions; the frontend is not the security boundary. Do not
execute document actions/scripts. Determine whether option-changing actions make
an edit unavailable without weakening their preservation or existing admission.

Acceptance includes paired values, repeated labels/export values, consistent and
inconsistent indices, inherited/aliased arrays, linked widgets, multiselection,
free text, empty selection, defaults, top index, original-font failure, artwork,
resource conflicts, protection, stale requests, cleanup and bounded inputs.
Windows tests exercise preview, keyboard/narrow layout, Undo/Redo, later filling,
save/reopen and recovery. Independent raw-object/font/pixel checks must confirm
selection and appearance preservation. One final scoped review, parent fixes and
no rereview. No commits, publishing, source-PDF overwrite or parity claim.
