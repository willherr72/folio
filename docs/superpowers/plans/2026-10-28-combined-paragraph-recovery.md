# Combined paragraph canvas and recovery plan

> Execute inline using superpowers:executing-plans. One final scoped reviewer.

**Goal:** Verify and correct the combined paragraph editing/recovery workflow.
**Architecture:** Existing isolated Windows candidate, actual UI/native IPC,
copied recovery data, independent saved-PDF comparison.
**Tech stack:** Playwright/WebView2, PowerShell, Python MuPDF/pypdf.
**Spec:** docs/superpowers/specs/2026-10-28-combined-paragraph-recovery-design.md

## Constraints and review focus

Keep source PDFs and all prior evidence unchanged. Stop only exact owned process
identities. No commits, package overwrite, or user-app restart. One Cargo command
at a time if needed. Check false raster acceptance, loss of mixed styles,
recovery from the wrong checkpoint, linked-widget topology, and stale candidates.

## Task 1: Combined acceptance

- [x] Bind candidate/source hashes; implement isolated editing and recovery harness.
- [x] Run keyboard resizing, highlighted replacement, Cancel, Apply, exact Undo/Redo.
- [x] Restore durable unsaved edit without the input copy; save and reopen/re-edit.
- [x] Independently compare baseline, text/style/highlight, outside pixels,
  untouched pages and PDF links/fields; prove comparison rejects corruption.

## Task 2: Resolve findings and record evidence

- [x] Reproduce each product failure before changing code; run affected tests.
  No product failure found. Five corrupted workflows reproduced three audit
  defects before correction; corrected audit rejects all five.
- [x] Obtain one scoped review and correct important findings in one pass.
- [x] Record exact passed checks, failures, limitations and next action in ledger.

Acceptance artifacts: artifacts/combined-paragraph-recovery-20261028.

Final authority: checkpoint-final.json. Four genuine saved PDFs pass; five
negative cases (20 modified PDFs) fail their intended predicates. Product source
and candidate unchanged. This is copied durable-checkpoint recovery, not a
power-loss/Undo-history test. Single-line canvas enlargement is covered;
layout-sensitive narrowing and wider producer desktop/resource work remain.
