# Group boundary editing implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Edit raster-sensitive adjacent pieces while preserving exact no-op output.
**Architecture:** Keep original and privately merged page bytes, guard raster-only
admission with glyph/region/overlap proofs, reuse existing changed-text publication.
**Tech Stack:** Rust, PDFium, lopdf, Tauri, Python independent PDF readers.
**Spec:** `docs/superpowers/specs/2026-10-24-group-boundary-editing-design.md`

## Constraints and review focus

No renderer change, no original document writes, no weaker surrounding-pixel proof.
Preserve permissions/comments/page identity; private merge must never become an
unchanged edit's output. Test rotated/cropped geometry, overlapping artwork/text,
resource identity, failure cleanup and save/reopen, including nonzero source pages.

## Task 1: Exact no-op publication and guarded admission

Files: `src-tauri/src/text_group.rs`, `src-tauri/tests/text_group.rs`.
Interface: private prepared group carries preview, original bytes and merged bytes;
public inspect/replace signatures remain unchanged.

- [x] Change the seven-boundary regression to require successful inspection/editing
  and exact original pixels/separate pieces for unchanged replacement; observe RED.
- [x] Retain original bytes, add bounded raster-only admission for non-overlapping
  selections, and register an unchanged copy with the source page's policy.
- [x] Add overlapping-artwork/text negative controls and rotated/nonzero-page
  no-op save/reopen checks; run full group tests and library cleanup regression.

## Task 2: Independent, review and desktop verification

- [x] Verify generated no-op/changed exports independently with MuPDF/pypdf and
  source hashes; run relevant text/paragraph/comment/protection tests.
- [x] One fresh scoped review of final delta; one important-fix TDD pass if needed.
- [x] Build and exercise real Windows group edit/no-op/undo/save/reopen; check loaded
  renderer and preserve previous checkpoint. Record evidence and remaining limits.

No commit/merge while the broad pre-existing dirty workspace is being developed.

Completed evidence: `artifacts/group-boundary-editing-20261024/final/verification.json`.
Corrected relevant native suite:250 passed,3 ignored,7 groups, exit0. Independent
checks:48 standard cases and7 embedded pairs. Corrected Windows workflow:3 checks;
independent desktop checks:2. One P2 review finding reproduced and corrected;
no rereview. Exact no-op preserves source extraction order, including rotated text.
