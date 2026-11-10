# Native area selection implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans.

**Goal:** Derive complete admitted source text selections from native area marks and publish combined text/drawing removal.

**Architecture:** Explicit complete-mode reuse of source/native glyph binding; source-owned worker call; existing strict combined publisher; no candidate registration.

**Tech Stack:** Existing Rust/PDFium/lopdf; independent saved-output Python audits.

**Spec:** `docs/superpowers/specs/2026-11-01-area-native-selection-design.md`

## Constraints and review focus

64 marks,4096 ranges,250,000 selected glyphs across the request. Existing text-only behavior unchanged. Private API only. Review whitespace completeness, generated separators, stale identities, overlapping marks and global allocation bounds. No commits/cleanup/package replacement.

## Task 1 — Native complete measurement and source-owned publication

Files: `redaction_selection.rs`; new `redaction_area_selection.rs`, `redaction_area_selection_tests.rs`; private module declaration in `engine.rs`.

Interface: `WorkerRuntime::redact_content_areas(&self, source: &str, marks: &[TextRedactionMark]) -> EngineResult<AreaContentResult>`. Return immutable source identity/hash, RemovedContentPdf and page-indexed selected glyph boxes. Measurement helper and area-ranges method remain engine-private.

- [x] Write failing native tests against a stub for complete text/path selection, whitespace, duplicate marks, source/registration invariants and refusals.
- [x] Implement complete measurement/range derivation and worker publication, preserving existing text-only path; run the whole redaction_ library suite serially.

## Task 2 — Independent output and review

- [x] Save owned fixtures from the new native entry point and verify exact retained source glyphs, removed source operands, drawing/graph state and complete expected rasters. Add whitespace-specific structural and geometry checks.
- [x] Freeze scope and request one final fresh review. Correct demonstrated issues with regressions; preserve failed evidence. A substantial evidence-driven measurement correction required one additional fresh scoped review, as documented in the spec appendix.
- [x] Reconcile source/native/audit hashes, record remaining image/host/UI limits and checkpoint; keep the full goal active.

Self-review: the API derives ranges itself; geometry-only glyph-less marks remain permitted until the combined publisher confirms a drawing removal. No area feature is exposed before all accepted content types are covered.

Outcome: 141 native tests passed, zero failed, one explicit skip. Three saved native outputs passed independent structural/geometry/raster audits with six effective corruption controls. See `artifacts/area-native-selection-20261101/review-disposition.md` for evidence and retained limits. Source/native origin matching retains its existing tolerance; exact retained-output glyph positions and pixels are separate independently verified fixture claims.
