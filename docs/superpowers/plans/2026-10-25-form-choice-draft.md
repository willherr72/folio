# Private Choice PDF Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. This is a private transaction stage of the full editor; no publishable PDF endpoint yet.

**Goal:** Apply source-bound choice edits to an isolated PDF graph while preserving unrelated data and accurately reporting required appearance work.
**Architecture:** Reuse the validated Context and model resolver. Return an opaque PreparedChoiceEdit with crate-private document access and public read-only original/resolved/change summaries; do not expose bytes or register a source. Reinspect the mutated graph before returning it.
**Tech Stack:** Rust/lopdf, existing copy budgets and source-bound inspection; native field comparisons in tests.
**Spec:** docs/superpowers/specs/2026-10-25-form-choice-design.md.

## Global constraints

Preserve prior checkpoints/failed evidence. No commits, publication, source-PDF
overwrite or concurrent Cargo/builds. All existing model/raw/graph limits remain.
No document mutation before expectedKey validation and model resolution. Clone
unchanged option operands only after CopyBudget admission; never mutate referenced
arrays/strings globally. Only the selected terminal owner may receive new Opt,
V,I,DV,TI,Ff entries. Widget AP/geometry/resources/actions and other objects stay
unchanged in this stage. Full appearance/font/native/worker/UI gates remain required.

## Value and preservation rules

If option values/display order are unchanged, retain the original Opt representation.
For changed options, retain each unchanged original operand by reference or exact
direct clone; encode new/changed rows as one string when export=display, otherwise
an export/display pair. Store the new array only at the selected logical owner.

Keep original V when answer meaning stays the same; compare multiselect export
multisets rather than incidental array order. Reordering remaps I. Materialize
indices when newly duplicated exports would otherwise lose selected row identity.
An explicit selection change writes the intended V and I. Clear writes null V
and an empty I array, masking inherited settings. Free text writes text V and
empty I. Retain numeric legacy I if its effective indices stay valid and unchanged.

Keep original DV for unchanged defaults; explicit clear writes local null DV.
Explicit defaults use one export string or a value array as appropriate. A source
blank noneditable V/DV may mean no selection/default when no blank option exists;
if the edit introduces a blank option, mask that raw blank with null to retain its
no-selection/default meaning. Do not claim default reset-action interoperability
from field-value reads alone; that remains a final workflow gate.

For lists, remap TI only when its effective top row changes. Dropdowns retain raw
ignored TI/MultiSelect bits. Manual-order opt-in clears only Sort in effective Ff.

## Appearance classification

For lists, repaint when display rows, selected row indices or top index change.
For dropdowns, repaint when the displayed selected/free text changes. Default,
export-value or unselected-dropdown-label changes need not repaint existing AP.
This classification does NOT waive font verification for new labels or source
appearance/global-flag compatibility checks. Prepared results remain unpublished.

## Review focus

Shared inherited Opt/V/DV objects must remain unchanged for other fields.
Duplicate exports need I even when the current export string did not change.
Adding a blank option must not turn an empty answer/default into a selection.
No-op and default-only edits must preserve saved AP bytes and opaque graph data.
Repeated widget/page placements and stale source requests must retain ownership.

## Task1 — private transaction

**Files:** new src-tauri/src/form_choice_draft.rs and form_choice_draft_tests.rs;
modify form_choices.rs to inspect an existing Context and lib.rs exports/test wiring.

**Interfaces:** SetFormChoicesRequest{page_index,annotation_index,expected_key,edit};
prepare_choice_edit_pdf(bytes,&request)->EngineResult<PreparedChoiceEdit>.
PreparedChoiceEdit exposes original(),resolved(),has_changes(),needs_appearance_update().
Its Context is crate-private; no serialization or public PDF-byte accessor.

- [x] Archive baseline; write failing graph-level tests with two logical fields,
  linked widgets, shared arrays/values and saved artwork. Observe missing API failure.
- [x] Implement staged owner-only changes, raw-object preservation, explicit masks,
  selection disambiguation and reinspection. Keep input immutable and reject stale keys.
- [x] Verify no-op, reordered values/indices, default-only, selected export changes,
  blank collision, duplicate exports, raw legacy representations and unchanged
  surrounding object graphs. Read prepared values with PDFium in test-only copies;
  those copies do not establish appearance acceptance.
- [x] Run library/model/inspection/property regressions; record exact hashes and
  remaining appearance/publication/UI gates. No fresh app build for the private stage.
