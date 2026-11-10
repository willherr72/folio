# Faithful Form Filling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Execution is inline under the user's standing autonomous authorization; one scoped final review follows implementation. Do not commit or publish.

**Goal:** Change form answers without erasing surrounding artwork or disturbing unrelated document state.

**Architecture:** Keep the original PDF graph as the publication source. A privately marked native form supplies validated values and generated answer paint; bounded correspondence, appearance isolation and saved-value checks govern the transplant. Existing worker registration and UI ownership remain in place.

**Tech Stack:** Rust, lopdf, pinned PDFium/pdfium-render, existing React/Tauri UI, Windows WebView acceptance harness, independent pypdf/MuPDF audits.

**Spec:** `docs/superpowers/specs/2026-10-26-faithful-form-filling-design.md`

## Global constraints

- Do not change product/runtime or start another Cargo/build while binding regression session 17116 is live. Record its terminal exit, fingerprint and test totals before beginning implementation. A timeout is not terminal.
- Preserve user PDFs, prior executables, dirty worktree changes and failed evidence. No commits, pushes or releases.
- Keep the public `FillFormFieldRequest`/worker/UI transaction contract and existing permission, validation, normalization and field-type behavior.
- Keep 512 MiB source/output, 4000 form-node/placement, 100000 traversal-value and 64-level depth bounds. Keep 32768 UTF-16 units per field answer.
- Bound each appearance to 4 MiB decoded bytes and 262144 operations, and aggregate retained appearance/resource data to 64 MiB. Charge before allocation and repeated expansion. Existing other-editor limits remain unchanged.
- Raw native fill stays private for isolated generators. A failed preservation proof must never fall back to publishing its whole result.
- No-op answers return original bytes after validation. Temporary probe markers never enter publication or recovery.

## Review focus

1. Linked widgets may use different local font bindings or share appearance dictionaries; changing one answer must retain each widget's artwork and correct font resolution. Task 3/4 tests.
2. Ordinary producer streams may have a single unmarked text object with state affecting later paint; preserving only `/Tx` inputs would break existing workflows. Task 3 tests, including existing `form_filling.rs`.
3. `/NeedAppearances true` and missing appearances require a complete effective baseline; clearing the flag must not hide unrelated fields. Task 4 tests.
4. Native serialization can change object numbers or expose duplicate names; identity must use validated per-operation correspondence and original ownership. Task 2 tests.
5. Large compressed appearance/resource graphs and repeated aliases can amplify memory across linked widgets or successive edits. Task 2/3/4 budget tests.

## Task 1: Preserve request semantics and establish failing public regressions

**Files:**
- Modify: `src-tauri/src/form_fill.rs`
- Create: `src-tauri/tests/form_fill_fidelity.rs`
- Reuse: `src-tauri/tests/form_filling.rs` and `src-tauri/tests/support/protected_fixture.rs`

**Interfaces:** Add private `ValidatedFill { field: FormField, wanted: FormFieldValue }` and `NativeForm::validate_fill(&self, request: &FillFormFieldRequest) -> EngineResult<ValidatedFill>`. Raw `NativeForm::fill` reuses it and retains its signature/semantics.

- [x] Write synthetic public-API regressions for text, dropdown and list artwork preservation, using the original 0.5-point frame reproduction. Assert the requested native value, original defaults, red-square pixels and static appearance operations after `fill_form_field`, `export_pdf`, reopen and a second edit.
- [x] Add shared-field variants on one and multiple pages with distinct artwork, plus answer clear/refill and an unchanged-value request. Do not use whole-page pixel equality where the answer is expected to change; isolate invariant artwork and page regions.
- [x] Run `cargo test --locked --manifest-path src-tauri/Cargo.toml --test form_fill_fidelity`. Confirm failures identify lost artwork, not fixture/permission/import errors. Preserve the red log.
- [x] Extract validation without changing it: expected-value comparison, native eligibility, NUL/length/single-line checks, choice index ordering/cardinality/type checks and radio rules. Keep focus/edit operations in raw `fill`.
- [x] Run the validation-focused library tests and `--test form_filling`. Existing accepted fills remain green; fidelity regressions remain red until publication is integrated.

## Task 2: Bind native results to the original field graph

**Files:**
- Create: `src-tauri/src/form_fill_identity.rs`
- Create: `src-tauri/src/form_fill_identity_tests.rs`
- Modify: `src-tauri/src/lib.rs` (private module declaration)
- Reuse: `src-tauri/src/acroforms.rs`, `src-tauri/src/form_fill_actions.rs`, `src-tauri/src/form_graph.rs`

**Interfaces:** Private `FillIdentity::prepare(bytes: &[u8], request: &FillFormFieldRequest) -> EngineResult<Self>` owns the original graph and selected logical-field/placement records. `probe_bytes(&mut self) -> EngineResult<Vec<u8>>` returns a bounded marked private copy. `bind_native(&self, native: &Document) -> EngineResult<BoundFill>` returns validated original/native node pairs and staged answer entries. `BoundFill` exposes only selected owner/widget correspondence and bounded answer entries to Task 4, not arbitrary native graph mutations.

- [x] Write tests for a merged field/widget, nested owner, linked cross-page widgets, aliased appearances, and repeated page placements. Renumber a simulated native result and assert that binding still follows markers and topology.
- [x] Add refusal tests for missing/repeated/detached markers, forged roles, mismatched parent/child/placement relationships, unexpected unrelated value changes, reference cycles and excess graph/answer data. Verify refusals precede large copies and leave the original graph unchanged.
- [x] Implement source ownership enumeration using existing filling validation. Use fresh collision-checked operation keys and bounded unique values. Mark all additionally materialized widgets when global regeneration requires their correspondence.
- [x] Stage only `/V`, `/I`, `/TI` and applicable `/AS` values from their verified native owners. Follow bounded references into owned primitives/arrays; never retain native object IDs as original references. Preserve defaults, option rows, field flags and all other graph entries.
- [x] Run `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib form_fill_identity`. Confirm marker removal, source integrity and all adversarial cases before integrating publication.

## Task 3: Isolate and transplant answer appearances

**Files:**
- Create: `src-tauri/src/form_answer_appearance.rs`
- Create: `src-tauri/src/form_answer_appearance_tests.rs`
- Modify only as required: `src-tauri/src/form_choice_text.rs`, `src-tauri/src/form_appearance.rs`, `src-tauri/src/form_graph.rs`
- Reuse: `src-tauri/src/form_choice_appearance.rs`

**Interfaces:** `AnswerKind { Text, Combo, List }`; `AnswerSlot::parse(bytes: &[u8], kind: AnswerKind) -> EngineResult<Self>`; `transplant_answer(source: &Document, original: &Stream, native: &Document, generated: &Stream, kind: AnswerKind, budget: &mut FillAppearanceBudget) -> EngineResult<StagedAnswerAppearance>`. The staged result contains a resource-isolated appearance ready for original-graph publication; it cannot register a source or write a file.

- [x] Test marked answer islands, empty marked slots, plain single-text-object fields, captions outside a marked island, static vector paint after text, source text-state changes, list selection paint, balanced nested graphics state, and deliberate ambiguity. Include the unmarked `form_filling.rs` shapes.
- [x] Test nonzero BBox origins, quarter-turn matrices, scale/UserUnit, source clipping, conflicting font resource names and identical names bound to different fonts. Assert unchanged static pixels and valid new answer paint, not just matching text operators.
- [x] Implement the spec's marked and unmarked admission rules. Preserve drawing order and state needed by later source paint; reject text clipping or ambiguous captions. Keep an empty insertion location for refill.
- [x] Import generated resources through one shared bounded copier or proven equivalent source bindings. If extending the copier's configurable budget, keep all existing callers' defaults and tests unchanged. Initialize generated answer state explicitly and preserve the original mapping/clip contract.
- [x] Add compressed-expansion, oversized operand, excessive operation, shared-font amplification and aggregate budget regressions. Verify preallocation checks and shared import memoization.
- [x] Run `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib form_answer_appearance` and the existing `form_choice_text`/appearance-copy tests. Preserve passing behavior of the choice-settings editor.

## Task 4: Publish complete faithful fills through the worker

**Files:**
- Create: `src-tauri/src/form_fill_publish.rs`
- Modify: `src-tauri/src/engine.rs`, `src-tauri/src/lib.rs`
- Extend: `src-tauri/tests/form_fill_fidelity.rs`

**Interface:** `form_fill_publish::fill(pdfium: &Pdfium, bindings: &dyn PdfiumLibraryBindings, source: &[u8], request: &FillFormFieldRequest) -> EngineResult<Vec<u8>>`. Worker ordinary filling calls this instead of publishing `NativeForm::fill` directly. Existing raw generation callers remain unchanged.

- [x] Write integration cases for dropdown export/display pairs and duplicate labels, editable dropdown text, multiselect clear/select, multiline/long Unicode text at existing limits, custom check/radio states, and inherited defaults. Validate against native values after reopen.
- [x] Add missing-normal-appearance and `/NeedAppearances true` cases with unrelated fields. Capture the initial native effective baseline; assert complete materialization before any flag normalization. Test an unprovable baseline refusal without changing the source.
- [x] Add preservation cases for field actions, bookmarks, comments, protected sources, recovery, linked local fonts, reused resources and repeated edits. Require a bounded resource/object count across repeated fill/clear/refill cycles.
- [x] Implement private validation/no-op, identity marking, native baseline/fill, staged value and appearance changes, resource pruning, bounded serialization and native saved-value/unrelated-field verification. Publish from the original graph only after every linked widget succeeds.
- [x] Keep worker registration, original path, source policy, imported annotation ownership and image leases unchanged. Failed staging or saved verification must not leave a registered candidate.
- [x] Run the fidelity and existing form-filling suites, then the relevant choice, appearance, position, protection and ownership suites. Do not weaken an existing expectation to hide a compatibility regression.

## Task 5: Review and Windows acceptance

**Files:**
- Create evidence: `artifacts/form-fill-fidelity-20261026/`
- Update: `docs/product-quality-loop.md` and the milestone progress/checkpoint records

- [x] Freeze the final changed sources and request one scoped final review through the available skill-prescribed review mechanism. Include the red reproductions, spec, plan, exact file hashes and current verification limits. Apply corrections inline; no automatic repeat review round.
- [x] Complete appropriate frontend/typecheck/build gates, stage a fresh owned executable/profile, and record source/runtime/binary hashes. Keep older apps and original PDFs intact.
- [x] Repeat the three Windows reproductions on fresh copies, adding shared widgets, Undo/Redo, clear/refill and recovery. The expected result is retained artwork and correct values through save/reopen, not merely a successful Apply.
- [x] Independently inspect saved PDFs and render artwork/page invariant regions with MuPDF. Verify temporary markers are absent, defaults/actions survive and resource references remain bound.
- [x] Record completed evidence and remaining limits without a parity claim. Resume the border/background controls design only after this prerequisite's gates are terminal and green.

## Execution note

The preceding binding correction has its own final review and full regression; do not conflate its approval or passing tests with this new fill correction. The current artifacts prove a defect and a promising publication approach. They do not satisfy the production tasks above.
