# Common form field creation implementation plan

> Use superpowers:executing-plans inline. Standing user authorization removes routine
> design/permission stops. One final scoped reviewer, no implementer delegation.

**Goal:** Create, preview, apply, fill and save common form fields on existing pages.
**Architecture:** Native complete-source candidate mutation with selected-occurrence
mapping; frontend uses preview ownership and existing commitPageReplacement.
**Stack:** Rust/lopdf/PDFium/Tauri, React/TypeScript, Vitest, Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-25-form-authoring-design.md

## Constraints and review focus

Native edit+annotate rights; repeated pages require assembly. Physical-point bounds
and font size, crop containment, upright current-view text, immutable source bytes.
Names/labels <=256 scalars/1024 bytes; unique top-level names without dots; no prefix
conflict. Choices <=256 unique labels/32768 bytes. Existing graph/resources preserved;
signed/XFA refused. No commits/releases. Freeze exact pre-edit baselines for review.
Tests cover indirect shared resources, hierarchy names, duplicate page isolation,
rotated/scaled appearances, and late candidate disposal.

## Task 1: Native field definition and selected-occurrence creation

Create src-tauri/src/form_author.rs and form_author_engine.rs; extend acroforms.rs
with bounded authoring document/name inspection. Wire engine.rs, lib.rs, app.rs.
Create src-tauri/tests/form_authoring.rs.
`CreateFormFieldRequest { bounds:AnnotationRect, rotation:u16, name,label,required,
font_size:f32, field:NewFormField }`, with tagged text{multiline}, checkbox,
dropdown{options}, list{options,multi_select}. Public API
`create_form_field_occurrence(source,occurrences,selected,request)->PageContentReplacement`.

- [x] Write failing native type/fill/reopen/limits/owner/duplicate/geometry tests.
- [x] Implement bounded field graph mutation with appearances, unique resources,
      existing occurrence mapping and restored source protection/overlay ownership.
- [x] Verify fresh native field metadata before returning, and release on error or
      abandoned reply. Run native form, occurrence and protection regressions.

## Task 2: Authoring workspace and candidate ownership

Create src/editor/form-authoring.ts and components/FormAuthorDialog.tsx; wire
adapter.ts/types.ts/App.tsx; add form-authoring geometry/dialog/app tests.
Native request uses source-display point bounds and workspace rotation. UI converts
current-view rectangles with existing to/fromDisplayPoint. Candidate maps via
replacementPages and commits via commitPageReplacement only after native raster loads.

- [x] Write failing geometry and lifecycle tests, including pending selection changes,
      returned borrowed IDs, cancel/late replies, page duplication and Undo/Redo.
- [x] Add page placement rectangle with pointer/keyboard/numeric editing and controls,
      explicit native Preview and Apply; reject areas outside retained crop.
- [x] Integrate tool entry and source adoption, leaving Fill forms available after
      Apply. Run focused/full frontend and TypeScript checks.

## Task 3: Review and verified checkpoint

- [x] Freeze baseline/delta/hashes; one scoped review and regression-driven fixes.
- [x] Build isolated Windows app; create/fill all types, verify duplicate isolation,
      rotation/crop, Undo/Redo, save/reopen and narrow-window keyboard controls.
- [x] Independent values/appearances/untouched pixels/graph checks, source hashes,
      loaded PDFium and owned-process cleanup; record evidence and remaining gaps.
