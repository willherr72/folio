# Compound dropdown filling implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing user autonomy replaces routine approval questions; no per-task implementers/reviewers. One fresh final review after concrete validation.

**Goal:** Fill ordinary and editable PyMuPDF compound dropdowns while preserving static frame, clip and surrounding document state.
**Architecture:** Admit Combo to the existing bounded static-frame normalization; retain all downstream native verification and conservative refusal paths.
**Tech Stack:** Rust/lopdf/PDFium/Tauri, existing Python/PyMuPDF/pypdf acceptance tools.
**Spec:** docs/superpowers/specs/2026-11-10-compound-choice-fill-design.md

## Global Constraints

Start from accepted039source d327. Preserve accepted039launcher and the separately deferred page-admission experiment, original producer PDFs and failed attempts.4MiB/262144operation/64MiB copy budgets unchanged. No public schema/dependency/version/frontend/cache/history changes. No physical printer, user-app restart, commit/push/publicrelease. Serialize native builds/tests/UI; oneCargojob and existingD:cache/temp. Use release dependencies/Folioopt0 for library correctness, optimized integration for the broad native gate; no profiling-enabled GUI.

## Review Focus

- A combo marker containing unknown artwork or selected-row paint must not be mistaken for a plain static border.
- Different actual answer clips and text crossing graphics-state boundaries must remain refusals.
- Empty initial values, explicit text values and indexed choices must retain native value/default/option semantics.
- Inherited resources, font context, nonidentity appearance transforms and unusual widget bounds must retain existing guards.
- Repeated fill/save/reopen and failed/stale requests must preserve original bytes, border pixels, ownership and bounded appearance graphs.

### Task 1: producer regressions and bounded combo normalization

**Files:** Modify `src-tauri/src/form_answer_compound.rs`, `src-tauri/src/form_answer_appearance_tests.rs`, `src-tauri/tests/form_compound_answer.rs`; add four authored PDFs and provenance under `src-tauri/tests/fixtures/`; evidence under `artifacts/compound-field-answer-20261110`.
**Interfaces:** Existing `normalize(&[Operation], &Range<usize>, AnswerKind, Option<[f64;4]>) -> EngineResult<Option<(Content, Range<usize>)>>`; no interface change. Existing `PdfEngine::fill_form_field` and native export/open remain the integration boundary.

- [x] Preserve d327product/native inputs and before copies of all modified existing files. Copy exact authored combo, editable-combo and both empty variants from the accepted-CLI reproductions; record hashes and generator provenance.
- [x] Add `compound_combo_frame_and_inset_answer_are_isolated_without_border_loss`: parse the original border/inset-clip program as Combo, splice an answer and reparse; assert one original border/clip/marker and answer after border. Extend existing unknown-art/interleaved/separate-clip/graphics-escape refusal tests to Combo. Keep a compound List refusal regression.
- [x] Add four `compound_combo_*` public-engine regressions for ordinary/editable, populated/empty producer fixtures. Fill through `FillFormFieldRequest`, save/reopen and repeat supported values. Assert actual answers, original source bytes, unchanged options/flags/defaults, exact pixels outside source inset clip `[101,66,349,134]` on the420x300fixtures, and stable saved graph count after repeated identical states.
- [x] Run focused native tests against unchanged production logic; expect real compound-combo parse/fill failures, no compile error. Retain actualREDlogs/inputs.
- [x] Extend only the permitted kind admission when the existing geometric/static-paint proof succeeds. Run focused compound tests GREEN, including old text cases and negative guards. If deeper behavior differs, diagnose and ledger the change before expanding implementation.

### Task 2: native correctness and fresh local binaries

**Files:** Evidence/scripts within the feature artifact directory; no new product behavior unless a reproduced regression requires correction.
**Interfaces:** Task1source/input fingerprint binds test results and normalGUI/CLI candidate; exact resource tree and unchanged frontend authority come from accepted039.

- [x] Run affected form fill/fidelity/choice/appearance/resource/geometry/physical-unit suites, then full native correctness under original separate optimized-integration and Folioopt0-library meanings. Zero failures required; skips explicit. No repeat fullsuite after passing without new changes/failures.
- [x] Qualify identical frontend inputs against accepted1517tests/226files; do not claim a fresh frontend run. Build fresh normal optimized GUI and CLI with complete unchanged25resource tree and strict pre/post source guards.

### Task 3: actual saved-output acceptance and final review

**Files:** Producer CLI bundles/saves, independent audits, owned desktop runs and source/evidence review snapshot in the feature artifact directory.
**Interfaces:** Consume exact Task2binary/source/resource identities; produce a bounded reviewed compatibility milestone, not an automatic combined-portable promotion.

- [x] Rerun all four accepted-CLI failure cases on the new CLI, save candidates, inspect with another PDF reader, compare complete source-bound outside-clip RGB at72/144dpi, preserve options/defaults/geometry and verify repeated editable saves. Include wrong value, erased frame, outside-paint and identity negative controls.
- [x] Exercise ordinary and editable dropdown answers through actual desktop controls, Apply/Undo/Redo/save/reopen and empty-value entry; bind original files, exact process birth/executable/resources and strict owned process/profile cleanup. No direct native invocation as a substitute for desktop entry.
- [x] One fresh final reviewer against the concrete manifest-defined delta and five ReviewFocus classes. Resolve consequential findings inline with RED/GREEN and appropriate suites; record all minors/declined judgments/rulings/costs.
- [x] Record exact accepted scope/limits, frozen evidence and deferred list compatibility. Preserve the existing worktree and all unsuccessful attempts; accepted039launcher unchanged. Do not mark the broad Adobe/Foxit-quality goal complete for this slice.
