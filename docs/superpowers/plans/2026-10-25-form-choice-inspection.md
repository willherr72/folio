# Source-bound Choice Inspection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. Continue the full choice editor; this task alone is not a desktop feature.

**Goal:** Read actual PDF options, current selection and defaults into the verified model without changing the source or guessing ambiguous row identity.
**Architecture:** Reuse form_properties' validated logical field tree through a narrow context, then decode bounded choice attributes and enumerate linked widget placements. A public read-only inspect_choice_pdf(bytes,page,index) returns FormChoices{expected_key,name,widgets,model}; no edit/publication endpoint is added yet.
**Tech Stack:** Rust/lopdf/serde; pinned PDFium for integration comparison.
**Spec:** docs/superpowers/specs/2026-10-25-form-choice-design.md.

## Global constraints

Preserve current verified model and prior executables/logs. One Cargo/build at a
time. No commits, publication or source-PDF overwrites. Option/string/count limits
come from form_choice_model. Raw decoded-input scan:512KiB aggregate,8194bytes per
encoded string before decoding; decoded text charged before retaining output.
Graph/decompression/field-tree limits remain the existing authoring limits.
Reject incomplete/invalid UTF-16 and invalid UTF-8 BOM text. Keep PDFDocEncoding.
Retain explicit null as an inheritance terminator to match native lookup; do not
change existing metadata-property inheritance behavior in this task.

## Selection rules and review focus

Validate ascending unique I indices, range and multiplicity before using them.
When V also exists, the multiset of selected exports must agree with V. A missing
V can use valid I. With no I, infer rows only for unique matching export values;
duplicate matches are ambiguous. Nonmatching single text is allowed only for an
editable combo. Empty I plus editable custom text means no option is selected.
Blank noneditable V/DV with no blank option means no selection/default; keep raw
objects untouched. Blank export options require their own native comparison.
Do not replace inconsistent V/I with whichever happens to look plausible.

Inherited attributes and shared/aliased options must bind the correct terminal owner.
Identical labels/exports need source indices, not deduplication.
Empty/absent/current/default variants must match native choice interpretation.
Raw count/string budgets precede decoding or retained output growth.
Repeated widget placements must be enumerated by page/annotation position.

## Task1 — PDF inspection

**Files:** create src-tauri/src/form_choices.rs and src-tauri/tests/form_choices.rs;
modify src-tauri/src/form_properties.rs, form_choice_model.rs and lib.rs.

**Interfaces:** narrow internal FieldContext owns Document, logical owner,
validated ancestry/widget IDs and properties. Its raw attribute lookup preserves
null and observes PDFium's32ancestor bound. Existing metadata load/change retains
its original behavior. FormChoices carries no public PDF object IDs.

- [x] Archive baseline exact bytes/hashes for touched existing sources.
- [x] Add failing PDF-fixture tests for pairs/strings, shared/inherited fields,
  ambiguous duplicate values, V/I mismatch, free/blank values, null overrides,
  malformed encodings/counts and source immutability. Observe missing API failure.
- [x] Expose the narrow validated context, implement bounded decoding and inspect,
  and validate the resulting ChoiceModel through the existing kernel validator.
- [x] Compare admitted selections/labels with actual pinned PDFium fields, including
  empty export strings and multiple selection. Adapt based on observed behavior,
  preserving contradictory evidence instead of guessing.
- [x] Run choice model/inspection, existing field-properties and library regression.
  Freeze hashes and document pending raw-graph mutation/appearance/worker/UI gates.

The future mutation stage must preserve raw defaults when unchanged and materialize
clears without accidentally exposing an inherited default. It must not return an
editable candidate until linked appearances and native selected values are proven.

Verified sourceab238b0e99c09534218d4cdcb33f6402146ef497e6656a3adf3dfab989dd940b:
259library+17model+14inspection+11existing-property tests passed,3ignored.
The null-V/stale-I and missing-Parent defects were reproduced with2failing tests
and fixed. Native ignored dropdown list settings then produced a separate failing
compatibility test, corrected by normalizing only the effective inspected model.
All failed logs and pre-correction sources remain under
artifacts/form-choice-inspection-20261025; verification.json binds final evidence.
No publication, fresh app build, desktop/UI or whole-feature review gate yet.
