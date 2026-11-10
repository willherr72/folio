# Rotated Latin reading implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Correct copy/search order for supported rotated Latin split text.
**Architecture:** Bounded private-page normalization with per-object and singleton
baseline evidence; source bytes/raw extraction unchanged.
**Tech Stack:** Rust/PDFium/lopdf, Python independent readers, Tauri/WebView.
**Spec:** `docs/superpowers/specs/2026-10-24-rotated-latin-reading-design.md`

## Constraints and review focus

Preserve raw extraction, source/crop/rotation/permissions, OCR, search budgets and
existing fallback. Check singleton evidence across objects, Unicode direction and
control characters, malformed/mixed orientation, import/native lifetimes, and
unintended changes to baseline ASCII eligibility. No renderer modifications.

## Task 1: Reader admission and native regression

Files: engine.rs, new reader_order.rs, tests/text_geometry.rs, reader-order-workaround.md.
- [x] Add seven-split accented reader/search/geometry/source tests; observe RED.
- [x] Extract and extend admission; prove singleton baseline ordering. GREEN.
- [x] Update singleton acceptance and retain negative/stationary/reversed/mixed/
  non-Latin fallback; test search permissions, save/reopen and unchanged source.
- [x] Run focused geometry/search/protection/OCR/text-group/semantic regressions.

## Task 2: Verification and checkpoint

- [x] Verify saved source content independently and actual Windows copy/Find.
- [x] One fresh scoped final review; one important-fix TDD pass if needed.
- [x] Build and record executable/source/loaded renderer hashes, accepted workflow,
  evidence/limits and quality ledger. Preserve previous checkpoints and originals.

No commits or external publication from this ongoing broad dirty worktree.
