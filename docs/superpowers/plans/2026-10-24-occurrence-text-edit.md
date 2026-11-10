# Occurrence Text Editing Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing user authorization permits implementation without approval questions. No commits or release.

**Goal:** Edit an existing text run while keeping shared fields and complete-source ownership intact.

**Architecture:** Private one-page mutation using existing validation, exact content graft into retained source, existing occurrence mapping. Legacy API remains unchanged.

**Tech Stack:** Rust, PDFium, lopdf; synthetic native integration fixtures and independent PDF readers.

**Spec:** `docs/superpowers/specs/2026-10-24-occurrence-text-edit-design.md`

## Constraints and review focus

Preserve original bytes; no arbitrary protected candidate import; no isolated
candidate leak; exact geometry/UserUnit; retain bounded replay metadata; preserve
native interaction identities and unselected occurrence pixels. No frontend or
desktop-support claim until separate integration is complete.

## Task 1: Native transaction

Files: `src-tauri/src/text_edit.rs`, `acroforms.rs`, `page_content_replace.rs`,
`page_occurrence_replace.rs`, `engine.rs`; new `occurrence_text_edit.rs`.
Tests: extend `src-tauri/tests/page_occurrence_replace.rs` using its shared-field
fixture, plus focused internal publication/lifetime regressions.

- [x] Reproduce existing shared-field refusal and add failing new API tests.
- [x] Implement internal shared-field projection, exact graft and worker transaction.
- [x] Verify unique/repeated/re-edit, field filling, invalid requests and geometry.
- [x] Verify policy, metadata, source lifetime and independent-reader outputs.
- [x] One scoped final review, address important findings, run affected regression.
- [x] Record exact source hashes, results, remaining UI/paragraph work in progress ledger.

Native acceptance: 372 passed / 3 ignored across 20 suites, plus the strengthened
filled-value save/reopen test passed. Independent readers verify 15 cases / 44
pages at two scales and 3 filled-field cases / 8 pages. Reviewed product remains
62440a9f6d1730b32923b69f048008bfcf16f333743803d332fabd8d994c7851.
See artifacts/occurrence-text-edit-20261024/checkpoint-accepted/verification.json.
No desktop integration or release claim; continue with IPC, adapter, selection
and complete-source Apply/undo ownership, followed by Windows acceptance.
