# Complete-source PDF save implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Preserve document structure in whole-source saves and refuse lossy structured composition.
**Architecture:** Add an internal complete-source save route after authorization,
before page import; retain the common security/publication tail. Keep admission,
graph edits and fallback guards in a focused engine child module.
**Tech Stack:** Existing Rust, lopdf, PDFium and native Windows acceptance tools.
**Spec:** `docs/superpowers/specs/2026-10-26-complete-source-save-design.md`.

## Global constraints

Save intent only; complete source in original order; no requested/imported overlays.
No relaxed permissions, byte limits, cancellation or source overwrite guards.
No new dependencies, frontend changes, commits or releases. Preserve evidence.

## Review focus

1. Deleted imported annotation plus empty requested overlays must not resurrect.
2. UI bookmark IDs differ from PDF outline IDs; semantic equality preserves the tree.
3. Protected/generated source provenance must not bypass re-encryption or permissions.
4. Crop/rotation in inherited or physical page units must retain original content.
5. Structured fallback refusal must preserve an existing destination and avoid rejecting temporary previews.

### Task 1: Retain complete source graphs in authorized saves

**Files:** Create `src-tauri/src/complete_source_save.rs`, `src-tauri/tests/complete_source_save.rs`;
modify `src-tauri/src/engine.rs`. Baseline engine before edits in milestone artifacts.
**Interfaces:** `WorkerRuntime::complete_source_save(&self, request: &ExportRequest,
bookmarks: &[DocumentBookmark], monitor: Option<&PdfSaveMonitor>) -> EngineResult<Option<Vec<u8>>>`;
`complete_source_save::guard_composition(sources: &[SourceForm]) -> EngineResult<()>`.
Existing compose final protection/publication remains shared, not duplicated.

- [x] Write public-API regression with tagged two-page source, metadata, page Group,
  structure ParentTree, outline unknown property and shared references. Require
  byte-identical ordinary full save, then exact graph retention except explicit
  crop/rotation/bookmark changes. Watch the original composer fail these assertions.
- [x] Add boundary cases for removed imported annotations, requested overlays,
  reordered/subset/duplicated/mixed sources, and structured refusal preserving a
  sentinel destination. Include unchanged bookmarks with different UI IDs.
- [x] Implement route and fallback guard, using original bounded source graph and
  existing crop/bookmark helpers; preserve shared security finalization.
- [x] Add cancellation at the source-preservation stage and protected save/denial
  assertions; run `cargo test --manifest-path src-tauri/Cargo.toml --test complete_source_save`.
  Expected all pass. Run existing `pdf_save`, `protected_engine`, `protected_copy`,
  `bookmarks`, `page_cropping`, `annotations` and representative form/text targets.
- [x] Record terminal counts, exact scope and rulings in the ledger; no commit.

### Task 2: Verify real producer saves and desktop acceptance

**Files:** Existing producer probes; `artifacts/complete-source-save-20261026/`;
README, `docs/product-quality-loop.md`, milestone checkpoint.
**Interfaces:** Public `export_pdf` from Task 1; no new product API.

- [x] Rebuild producer-save-probe; run all six original producer PDFs to a fresh
  output directory. Independently compare whole bytes for unchanged saves plus
  page text, pixels, structure references and input hashes.
- [x] Build native desktop executable and verify Windows Word Save/Reopen, crop/
  rotation/bookmark behavior and structured-overlay refusal with retained draft.
- [x] Obtain one fresh scoped final review; reproduce material findings and fix
  with red/green evidence. Record limitations separately from demonstrated defects.
- [x] Record fingerprints, test/build/desktop results and remaining Word editing
  blockers. Update roadmap/latest verified checkpoint only when gates pass.
