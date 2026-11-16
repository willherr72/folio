# Compound list frame implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing user autonomy replaces routine approval questions. No per-task implementer/reviewer agents; one final source review.

**Goal:** Fill plain compound list-box appearances while retaining conservative refusal of source row paint and custom artwork.

**Architecture:** Admit List to bounded static-frame normalization only for structurally uncapturable marked slots; preserve already capturable choice slots. Existing frame proof must reject internal row paint. No metadata-bound row-paint repair in this stage.

**Tech Stack:** Rust/lopdf/PDFium/Tauri and existing Python/MuPDF/pypdf acceptance helpers.

**Spec:** docs/superpowers/specs/2026-11-10-compound-list-frame-design.md

## Global Constraints

Start from completed dropdown source68; preserve before maps and completed evidence. Keep 4 MiB bytes/262144 operations/64 MiB copies. No public API/frontend/dependency/version/cache/history change, whole-document reparse, stricter choice-editor admission, commit, push or portable promotion. Serialize native/compiler/UI, one Cargo job and D: cache/temp. Keep accepted039 launcher. Profiles: optimized integration, release dependencies with Folio opt-level 0 library correctness. No performance claim.

## Review Focus

- Already valid marked List selection paint must survive repeated edits without new normalization.
- Source row-highlight rectangles and arbitrary internal artwork must remain refusals, including selected empty values.
- Empty/multi-selection metadata and actual painted rows must agree after clear/refill/save/reopen.
- Distinct clips, graphics escape, resource/font aliases and bounded-copy checks must retain their guards.
- Source bytes, outside-clip artwork, saved graph retention and exact runtime ownership must remain stable.

### Task 1: plain-list regressions and minimal normalization

**Files:** `src-tauri/src/form_answer_appearance_tests.rs`, `src-tauri/tests/form_compound_answer.rs`, `src-tauri/src/form_answer_compound.rs`; four fixtures/provenance under `src-tauri/tests/fixtures`; artifacts under `artifacts/compound-list-frame-20261110`.

**Interfaces:** Existing `normalize(ops, marked, kind, bounds)` and `PdfEngine::fill_form_field`; no signature change. Source68 manifests define the delta independently of wider Git changes.

- [x] Preserve current source/native-input manifests and exact before copies. Copy the four plain corpus PDFs with producer/derivative provenance and hashes; retain the other four as negative controls.
- [x] Replace the misleading old plain-List refusal test with an actual internal selected-row-paint refusal. Add a positive multi-row List isolation/splice/reparse test, asserting one source border/clip/marker and border-before-answer order. Extend existing unknown-art/separate-clip/interleaving/graphics-escape cases to List.
- [x] Add four public-engine cases: original-single, empty-single, multi-one, multi-two. Cycle single `[1]`, empty, `[2]`, or multi `[1,2]`, empty, `[0,2]`, over three cycles. Assert native/saved values and selected indices, unchanged metadata/source/outside-inset RGB, and stable saved object counts on repeated equivalent states. Reuse existing bounded rendering/save helpers.
- [x] Run focused native tests on unchanged source68 production; retain genuine parse/fill RED, not compiler/fixture errors. Expected: positive new cases fail; negative guards pass.
- [x] Admit List to existing static-frame proof and preserve already capturable Combo/List slots. No classifier relaxation. Expected: new positives and all old Text/Combo/generated-choice guard regressions pass. Stop and diagnose if any authored source-painted case becomes admitted.

### Task 2: native correctness and normal candidate

**Files:** Artifact-only scripts/receipts unless a reproduced product regression requires a correction.

**Interfaces:** Current exact source/native inputs bind focused outputs, full native gate and normal candidate. Accepted039 resource tree remains 25 identical files.

- [x] Run full native correctness with explicit ignored tests and zero failures; it includes affected form/resource/geometry/physical suites, so avoid a duplicate subset.
- [x] Qualify exact unchanged frontend inputs against accepted1517/226 evidence, build fresh normal optimized GUI/CLI and enforce source/resource guards. Preserve failed attempts; no source edits during active validation.

### Task 3: independent outputs, desktop and final source review

**Files:** Artifact-only CLI/desktop workflows, row-aware independent oracle, frozen review/evidence report.

**Interfaces:** Exact Task2 binaries/resources/source; actual saved appearances rather than logical metadata are the visual oracle.

- [x] CLI repeat fill/clear/refill/save/reopen on four positive cases. Negative four source-painted/custom-art cases remain refused and unchanged.
- [x] Independently render saved PDFs at72/144dpi; compare complete pixels outside the source-owned inset. Prove visible option labels and selected-row paint correspond to saved indices. Add controls for wrong visible row with correct value, hidden/missing labels, residual selected paint on empty selection, border loss and outside paint. Preserve real oracle RED/GREEN evidence.
- [x] Actual desktop single/multi selection entry, Apply/Undo/Redo/save/reopen, source/binary/resource hashes, trusted input and exact owned cleanup. Do not substitute native invokes or repeat completed CLI on a harness-only retry.
- [x] One fresh final source review of the manifest-defined delta; resolve consequential findings inline with RED/GREEN, record every gap/ruling/cost and preserve snapshots. Capture parent helper evidence separately where omitted from review.
- [x] Close only this bounded local milestone. Keep source-painted/scrolled list compatibility, package promotion and memory quality on the roadmap; broad goal remains active.
