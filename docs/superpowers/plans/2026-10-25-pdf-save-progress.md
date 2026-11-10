# PDF Save Progress Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. One final scoped correctness review.

**Goal:** Give ordinary editable/flattened PDF saving honest progress and safe cancellation.

**Architecture:** Window-owned native registration plus atomic control and a progress channel; monitored composition; temporary-file cancellation before atomic commit; frontend task with progress modal and existing save-result semantics.

**Tech Stack:** Existing Rust/PDFium/lopdf/Tauri, React/TypeScript, Vitest and packaged WebView2 workflow.

**Spec:** docs/superpowers/specs/2026-10-25-pdf-save-progress-design.md

## Constraints and review focus

- One job per owner, pending expiry 120 s, no duplicate start/foreign cancellation.
- Cancellation never authorizes publication, overwrites inputs, or clears dirty state.
- Native calls remain cooperative; events contain only stage/page counts and job ID.
- Keep existing atomic final publication and 512 MiB output limits; destination lock
  retries remain bounded. Error/cancel paths clean only owned temporary files.
- Existing uncontrolled engine exports remain compatible. Other export dialogs are
  not nested with the new modal. No dependencies, unrelated cleanup or commits.

## Task 1: Native controlled save

Create src-tauri/src/pdf_save.rs and pdf_save_commands.rs. Modify engine.rs,
physical_units.rs, pdf_output.rs, lib.rs and app.rs. Add tests/pdf_save.rs.

- [ ] Red tests for cancellation/commit/destination preservation and controlled export stages.
- [ ] Implement PdfSaveControl alias, progress/monitor, controlled engine export and physical normalization checkpoints.
- [ ] Add monitored composition boundaries and cancellable temporary writes with atomic commit.
- [ ] Add window-bound jobs/commands, expiry/duplicate/foreign/abandonment tests and application registration.
- [ ] Verify focused native lifecycle, publication, preservation and protected-save checks.

## Task 2: Editor integration

Create src/editor/pdf-save.ts and src/components/PdfSaveProgressDialog.tsx. Extend
adapter.ts and App.tsx. Add tests/pdf-save.test.ts, pdf-save-dialog.test.tsx and App save cases.

- [ ] Red task-race, progress, cancellation retry and save-result dirty-state tests.
- [ ] Implement channel task; optional adapter capability for native save; preserve fallback/demo behavior.
- [ ] Integrate main save actions and accessible progress modal, unmount cleanup and late-result handling.
- [ ] Run focused tests and TypeScript, then full frontend suite.

## Task 3: Review and packaged verification

- [ ] Freeze exact delta and obtain one final correctness review; resolve demonstrated issues.
- [ ] Build fingerprinted executable and test Windows save-dialog cancel, mid-composition cancel and successful retry.
- [ ] Independently verify saved content/features and unchanged input/existing destination; inspect screenshot and cleanup.
- [ ] Update quality ledger, latest checkpoint evidence and remaining limitations; keep broad goal active.
