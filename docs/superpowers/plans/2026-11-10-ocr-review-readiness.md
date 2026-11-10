# OCR review readiness implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one fresh final review. Standing user autonomy replaces routine design/permission questions.

**Goal:** Prevent enabled OCR review actions from being silently ignored while the initial preview owns the workflow.
**Architecture:** Reuse previewLoading in dialog busy state; preserve the hook, native protocol/data and original actual OCR acceptance. Isolated implementation candidate, then a new combined integration.
**Tech stack:** React/TypeScript/Vitest, existing Rust/Tauri/PDFium GUI, Python/PowerShell/Node evidence helpers.
**Spec:** docs/superpowers/specs/2026-11-10-ocr-review-readiness-design.md

## Global constraints

Preserve failed038/037, accepted036/launcher, all source/receipts/outputs. No native/cache/dependency/renderer changes; no relaxed masks/history/timeout/worker/security checks. Fresh frontend under5000ms/two workers; source/input/native-byte guards, reused2680native evidence explicit. Normal optimized GUI with complete038resources. Serialize suites/builds/UI/resource sessions. One fresh final scoped review. No commit/push/public release or auto user app launch.

## Review focus

- Multi-page review exposed before execute pending clears: user must not receive an enabled ignored correction/reset/navigation action.
- Cancel, preview failure and late preview replies: readiness must not trap the dialog or publish canceled native state.
- Existing owned OCR and fresh recognition, repeated copies and source changes: correction/source/native request identity must remain exact.
- The same real OCR workflow must pass; a harness wait/retry must not stand in for the product fix.
- Old package/source/test results and reduced resources must not qualify the changed UI; preserve actual identities and failed attempts.

### Task 1: controlled reproduction and minimal readiness fix

**Files:** tests/ocr-dialog.test.tsx; src/components/OcrDialog.tsx; artifacts/ocr-review-readiness-20261110/{baseline/,before.json,regression-red.log,regression-green.log}; own ledger.
**Interfaces:** Consume actual failed038 sourcea3f2/result/screenshot and hook execute pending/previewLoading contract. Produce one changed dialog busy predicate and meaningful deferred-preview regression; preserve before inputs/identities.

- [ ] Verify source/native baseline and exact failed OCR point; confirm owned roots absent and preserve all failures. Copy before source/changed files and immutable package/native authority bindings before editing.
- [ ] Add the two-owned-target/deferred-initial-preview test. Require disabled editor/page/reset controls and enabled Cancel, then exact correction/adoption after resolve. Run against old dialog and see the intended enabled-control assertion fail.
- [ ] Include previewLoading in busy, run focused OCR tests GREEN, full1517-or-actual frontend suite with original settings, and source/native/shared-input guards. Preserve RED/GREEN and actual counts; no broad native rerun when native bytes are unchanged.

### Task 2: actual corrected GUI and unchanged OCR acceptance

**Files:** new normal GUI build/source/resource manifests, captured OCR/session/stop/auditor helpers, actual OCR run/independent outputs/absence receipts.
**Interfaces:** Consume Task1 source/frontend and exact unchanged native/profile/input qualification. Produce identity-bound actual candidate/output/audit/cleanup evidence for review.

- [ ] Build optimized profiling-disabled GUI with source/frontend guards and complete immutable038resource hashes. No overwritten038package or old pass receipts.
- [ ] Run the unchanged original OCR script against the candidate and independently verify both saved PDFs, all12offline assets, shared-copy/multi-source/Undo/Redo/owned-review behavior. Native request must contain the correction; original scanned ink/glyph exclusions preserved. Failed attempt retained; no timeout/wait changes.
- [ ] Strictly verify owned root/profile absence and retain saved reports. If evidence contradicts the hypothesis, investigate before accepting the fix.

### Task 3: reviewed historical implementation and next integration

**Files:** frozen public review package/report/disposition, checkpoint helper/receipt, next integration notes.
**Interfaces:** Consume actual Task1/2 terminal evidence. Produce historically verifiable reviewed fix; accepted036/failed038 unchanged.

- [ ] One fresh scoped review of source/test/helper evidence and readiness/cancellation/ownership boundaries; grade effects, fix consequential issues inlineRED/GREEN/affected checks; record minors/declined/rulings.
- [ ] Seal source/native/frontend/helper/binary/resource/output facts independently of future ROOT. Continue a new separately versioned combined portable with current native authority and fresh extracted acceptance; do not relabel038 as qualified.

Self-review: Task1owns real enabled-control regression and UI contract, Task2owns unchanged actual OCR/native/source/resource evidence, Task3owns fresh review/historical independence/new integration. One-line UI fix stays separate from failed metadata-only038; no extra pre-plan reviewer or per-task agents.
