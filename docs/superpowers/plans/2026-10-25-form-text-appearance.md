# Existing Text-Field Appearance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Inline execution is preserved from standing authorization; no implementation delegation.

**Goal:** Let users change a selected text widget's font size, color and alignment without losing shared values or surrounding PDF content.

**Architecture:** Add source-bound settings inspection and an immutable selected-occurrence transaction. Reuse bounded isolated native regeneration, then expose explicit preview/Apply through the established dialog ownership model.

**Tech Stack:** Rust/lopdf/PDFium, Tauri, React/TypeScript, Vitest, Windows desktop acceptance.

**Spec:** docs/superpowers/specs/2026-10-25-form-text-appearance-design.md

## Global Constraints

- Current form-color full native gate must finish before changing product/test sources.
- One Cargo/build at a time; preserve originals, old executables and failed evidence. No commits/releases.
- Exactly one final scoped reviewer after implementation; parent fixes findings, no rereview.
- Fixed font size1–512 physical pt;0 Auto fit; unchanged settings are omitted from mutation payloads.
- Existing widget, page occurrence, protection and source ownership boundaries remain authoritative.

## Review Focus

- Local/inherited/page font aliases can resolve differently from saved AP: pin conflict rejection and embedded font identity in native tests.
- Repeated page copies and shared fields require distinct geometry/artwork but linked values: test both selected-first and selected-later occurrences.
- Auto fit/empty values may omit text paint: test later filling after save/reopen, not only initial AP bytes.
- UserUnit1.7 and mixed color spaces must survive unchanged settings: inspect saved physical sizes and unchanged color operators.
- A modal may close or change while a candidate/raster arrives: test orphan cleanup, borrowed IDs, and one-time ownership transfer.

### Task1: Native inspection and isolated mutation

Files: create src-tauri/src/form_text_appearance.rs and src-tauri/tests/form_text_appearance.rs; modify src-tauri/src/form_reflow.rs and src-tauri/src/lib.rs only for reusable bounded helpers and exports.

Interfaces: FormTextAppearance {style_key:String,font_label:String,font_size:f32,color:String,color_space:String,alignment:TextFieldAlignment}; TextFieldAlignment serializes left/center/right. TextFieldAppearanceChanges {font_size:Option<f32>,color:Option<String>,alignment:Option<TextFieldAlignment>}. SetFormTextAppearanceRequest {annotation_index:usize,expected_value:FormFieldValue,expected_bounds:AnnotationRect,expected_style_key:String,changes:TextFieldAppearanceChanges}.

- [x] Archive baseline files/hashes under artifacts/form-text-appearance-20261025 before edits.
- [x] Write failing native tests for changed-only settings, source font identity, shared sibling AP preservation, empty-field later filling, original CMYK/gray retention, and ambiguous same-alias fonts.
- [x] Run focused tests and preserve expected failures.
- [x] Implement bounded raw inspection and change helpers in form_text_appearance; reuse the existing form_reflow::change through a private font-bound placeholder without changing existing reflow admission or appearance policy (ledger ruling).
- [x] Test stale/invalid settings, unsupported saved artwork/settings and repeated-edit retention. Broader malformed/alias-cycle/flag/crop matrices remain recorded coverage gaps, not claimed acceptance. Run cargo test --manifest-path src-tauri/Cargo.toml --test form_text_appearance --test form_reflow; expect all pass.

### Task2: Public selected-occurrence transaction

Files: create src-tauri/src/form_text_appearance_engine.rs; modify src-tauri/src/engine.rs, src-tauri/src/app.rs and module inclusion. Extend native test above.

Interfaces: PdfEngine::inspect_form_text_appearance(&self,source_id:&str,page_index:usize,annotation_index:usize)->EngineResult<FormTextAppearance>; PdfEngine::set_form_text_appearance_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,request:SetFormTextAppearanceRequest)->EngineResult<PageContentReplacement>. Tauri commands use the same snake_case names. Inspection presents physical sizes; mutation converts only explicit size changes to raw units.

- [x] Write failing tests for page rotations/UserUnit1/1.7/2, repeated-first/later occurrence, protected policy and stale identities.
- [x] Implement worker requests, source permission checks, pre-copy admission, duplication mapping, raw mutation, unchanged value/field/geometry verification, and immutable publication with bookmarks/security.
- [x] Run native form appearance, reflow, positioning, filling and protected-form regressions; preserve independent saved-output checks and source hashes.

### Task3: Desktop controls and ownership

Files: create src/editor/form-text-appearance.ts, src/components/FormTextAppearanceDialog.tsx and tests/form-text-appearance.test.tsx; modify src/editor/adapter.ts, src/components/FormFillEditor.tsx and src/App.tsx. FormFillDialog forwards the shared callback.

Interfaces: FolioAdapter.inspectFormTextAppearance(sourceId,pageIndex,annotationIndex)->Promise<FormTextAppearance>; setFormTextAppearance(sourceId,occurrences,selected,request)->Promise<PageContentReplacement>. Dialog receives the same adapter/document/page/field/sourceIds/onApply/onCancel boundary as FormPositionDialog. Add onTextAppearance(field) to FormFillEditor.

- [x] Write failing tests for changed-only payloads/physical units, Auto fit, invalid sizes/colors, dirty-draft gating, reset and unsupported inspection.
- [x] Implement read-only original font label, explicit size/alignment controls and Original color versus Custom RGB choice, native preview, error feedback and one-time Apply using existing replacementPages/commit behavior.
- [x] Test stale candidates and rasters, close cleanup, foreign source IDs, Apply ownership and input changes invalidating preview. Run npx vitest run tests/form-text-appearance.test.tsx and relevant form/editor integration tests; expect all pass.

### Task4: Review and acceptance

- [x] Freeze the scoped delta and request one final source review with evidence and explicit coverage limits.
- [x] Reproduce/fix actionable findings with regression evidence; preserve original review, no rereview.
- [x] Run appropriate native suites, full frontend tests, setup/build and source fingerprint checks.
- [x] Use isolated Windows desktop to test inspection/preview/Apply/Undo/Redo, later filling and save/reopen, shared differently styled sibling, Auto fit and narrow640px controls. Independently inspect saved fonts/colors/values/geometry and unchanged pages. Stop only the owned process.
- [x] Bind executable/source/DLL/evidence hashes in verification.json, update README and product-quality-loop. Keep the larger goal active.

Self-review: all spec requirements map to tasks1–4; no next-feature product changes are authorized by a passing plan alone. Standing user authorization permits implementation once the current checkpoint gate is settled.

Completion evidence: artifacts/form-text-appearance-20261025/checkpoint-corrected/verification.json. The final review findings were reproduced and corrected; no rereview. See coordinator-response.json and the ledger for scope rulings and coverage limits. No commit or release; the broader goal remains active.
