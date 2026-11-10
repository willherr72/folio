# Native Choice Editing Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. This is the native value-model task of the full choice editor, not completion of that feature.

**Goal:** Resolve choice edits without confusing display labels, export values, selected rows, free-text answers or reset defaults.
**Architecture:** A pure bounded Rust model transforms a source inspection plus source-indexed draft into resolved options/current/default/top-index settings. The later PDF adapter owns field-tree validation, raw-object preservation and appearance publication. No IPC or UI advertises this kernel alone.
**Tech Stack:** Rust, serde, existing EngineError/EngineResult.
**Spec:** docs/superpowers/specs/2026-10-25-form-choice-design.md.

## Global constraints

The recovery audit is terminal and verified; preserve its executable/evidence.
No commits, publishing, source-PDF overwrite or concurrent Cargo/builds.
Options:0–2,000; each string <=4,096 UTF-8 bytes; combined inspected or draft
option/value string payload <=128 KiB, checked before cloning. Each boundary
validates its own input independently. Unknown JSON fields and tags are refused.
Source row indices are unique in a draft and refer only to the captured source.
Do not derive identity from display or export strings. Keep repeated values valid.
The PDF adapter must separately charge raw encoded bytes before decoding.
Source inspection binds source/page/widget identity; pure model does not authorize
publication. Full feature still requires font/artwork preservation, preview/UI,
Undo/Redo/save/reopen/recovery, review and Windows acceptance.

## Decisions

Current Keep maps original selected indices to their surviving draft rows. If a
selected row is removed, require explicit Set or Clear. Updated exports follow
the retained row. Explicit Set references final draft indices and emits sorted
unique indices; reject duplicate indices, out-of-range indices and multiple
selections in a single-select field. Text is allowed only in editable dropdowns.
An explicit text value equal to an option export is ambiguous and must use a row
selection instead. Existing ambiguous raw values belong to PDF inspection refusal.

Default Keep preserves the original values, never inventing a unique row for a
duplicate export. Each old default must retain a surviving original row with that
same export, unless it was an editable free-text default. Otherwise require explicit
Set/Clear. Set from rows yields unique export values in final row order; multiple
defaults require multi-select. Explicit default text follows the editable rule.
Absent defaults remain absent; default Clear is explicit, with PDF materialization
handled by the adapter to mask inherited defaults.

Preserve top-visible row identity. If removed, choose the first surviving later
original row, then the nearest earlier row, then index0. Empty options use0.
On an originally sorted field, changing options requires explicit manual_order=true,
which clears only Sort. No platform-dependent implicit sorting. Unchanged options
preserve Sort unless the explicit manual-order choice clears it.

## Review focus

Duplicate labels/exports must not change selected identity after reorder.
Removing a selected/default row must not silently select a neighbor.
Editable text equal to an export must not acquire inconsistent V/I semantics.
Large or malformed source inputs must fail before a transformed output is cloned.
Inherited default clearing must be represented explicitly for the later adapter.

## Task1 — model and resolution

**Files:** create src-tauri/src/form_choice_model.rs and
src-tauri/tests/form_choice_model.rs; modify src-tauri/src/lib.rs exports.

**Interfaces:** public ChoiceOption{export_value,display}, ChoiceDraftOption{
source_index:Option<usize>,export_value,display}, ChoiceSelection::{None,Rows,Text},
ChoiceEditSelection::{Keep,Clear,Rows,Text}, ChoiceModel{options,selection,defaults,
combo,editable,multiple,sorted,top_index}, ChoiceEdit{options,selection,defaults,
manual_order}, ResolvedChoiceEdit{options,selection,defaults,top_index,sorted,
defaults_changed}. Defaults are Option<Vec<String>> in the source/resolved models;
the action enum keeps an explicit clear distinct from omission. resolve_choice_edit
(&ChoiceModel,&ChoiceEdit)->EngineResult<ResolvedChoiceEdit> is the kernel.

- [x] Add integration tests importing the planned API; observe the missing API
  failure before implementing it. Tests cover reordering duplicate values, changing
  selected export values, removal requiring explicit choices, explicit multiselect,
  free text, duplicate default ambiguity, sorted opt-in and top-index remapping.
- [x] Implement validated types and bounded resolution without PDF or filesystem
  side effects. Return errors without mutating either input. Preserve unchanged
  source/default semantics for the later graph adapter.
- [x] Exercise limits, invalid flags/indices/source mappings, unknown transport
  fields/tags and exact empty/free-text/default distinctions. Run the focused test.
- [x] Run library regression once the focused tests pass; record exact evidence
  and source fingerprint. Do not rerun the whole 45-minute suite for this isolated
  unintegrated kernel; broaden after PDF/worker integration.
- [x] Update the ledger and full-feature plan. Continue into source-bound PDF
  inspection/mutation and appearance proofs; no feature or parity completion claim.

Model gate:259library+17model tests passed,3ignored, exit0. The original missing
API failure and an intermediate lifetime compile error are retained. Source
fingerprint8a075d58be66fd61963471370a0cec8ace83a737a918173c2833174d37558ced.
Evidence: artifacts/form-choice-model-20261025/verification.json. This completes
the model task only; no PDF/worker/UI integration or whole-feature review yet.
