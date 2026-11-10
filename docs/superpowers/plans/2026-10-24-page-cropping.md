# Page Cropping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Use the existing isolated worktree; no commits or publication in this ongoing autonomous loop.

**Goal:** Crop a page with a reversible retained-area setting that survives editing, recovery, save, and print.

**Architecture:** Preserve original source coordinates and identity with `PagePlan.crop`. Clip the workspace view using centralized geometry, and set the source-mapped PDF CropBox after native content/annotation composition.

**Tech Stack:** React/TypeScript, Rust, PDFium, lopdf, Vitest, existing Windows acceptance scripts.

**Spec:** `docs/superpowers/specs/2026-10-24-page-cropping-design.md`

## Global Constraints

- Crop is physical source-display geometry before workspace rotation; width/height and stored object coordinates retain their source meaning.
- Minimum retained width/height is 1 pt; source dimensions and UserUnit are native authority.
- Requires editContent permission; never delete hidden contents or claim redaction.
- Recovery writes version 4, reads 1–4; raw-unit migration only below version 3.
- Preserve the quiz, prior builds/evidence, immutable sources, and intentional dirty tree. No new dependencies, commits, or releases.

## Review Focus

- Partial and fully hidden interactive objects must not leave invisible focus targets.
- Source replacement from paragraph/form/OCR edits must retain the same crop frame.
- Decimal UserUnit and rotated nonzero origins must not shift crop boundaries or overlay replay.
- Shared widgets, bookmarks, and duplicate source occurrences must retain ownership after cropping one occurrence.
- Stale preview, document switch, and failed save/recovery must not commit a different crop.

## Task 1: Native crop composition and durable state

Files: `src-tauri/src/types.rs`, `engine.rs`, new `engine/page_crop.rs` or equivalent private crop module, `physical_units.rs`, `document_policy.rs`, `recovery.rs`; literal compatibility updates in existing Rust callers; tests `src-tauri/tests/page_cropping.rs`, `recovery_store.rs`, `physical_units_recovery.rs`, `protected_engine.rs`.

Interfaces: optional `PagePlan.crop: Option<AnnotationRect>`; public physical validation and private raw source validation; crop publication helper consumes original PageGeometry list. No new IPC command required; render/export/print use existing page plan.

- [x] Add serde-based crop requests testing output dimensions, per-occurrence independence, immutable source, and invalid crop rejection. Run before implementation; expect missing crop behavior.
- [x] Add optional native field, conversion, source-bound validation, and edit-content permission enforcement. Update literal callers with `crop: None` only.
- [x] Apply leaf CropBox after AP/form/comment composition and before serialization. Preserve original geometry for overlays and bookmarks.
- [ ] Cover rotations, nonzero origins, decimal UserUnit, retained native comments/widgets, portable overlays and save/reopen, render/print, and protected composition.
- [x] Add recovery version-4 crop validation and legacy tests. Verify invalid state rejected without replacing valid recovery. Run targeted native suite and independent reader checks.

## Task 2: Workspace geometry and history

Files: `src/editor/types.ts`, `geometry.ts`, new `page-crop.ts`, `model.ts`, `recovery.ts`, `src/components/PageView.tsx`, `DocumentViewport.tsx`, `NativeCommentLayer.tsx`, `ExistingTextLayer.tsx`, `PdfTextLayer.tsx`, associated controls, search and navigation utilities. Tests: new `tests/page-crop.test.ts`, targeted existing viewport/interactions/recovery tests.

Interfaces: `pageVisibleRect(page)` in rotated display coordinates; `pageVisibleSize(page)`; `cropPage(document,pageId,crop?)` no-op on identical/invalid crop; crop-aware `clientPointToPage` preserving base coordinates.

- [x] Write failing tests for each rotation, margins roundtrip, pointer mapping, dirty tracking, one-step history, independent duplicates, source replacement retention, and hidden target behavior.
- [x] Implement geometry and model functions; validate restored crop data. Pass crop visibility to canvas layers and remove invisible focus targets without deleting objects.
- [x] Use visible dimensions/viewBox for pages and thumbnails, explicitly render full-source rasters for clipped placement. Update zoom/scroll anchors, search and bookmark navigation.
- [x] Run targeted tests and TypeScript checks. Reproduce a cropped text edit and annotate at a known visible point.

## Task 3: Crop dialog and integration

Files: new `src/components/CropPageDialog.tsx` and styles, `src/App.tsx`, paragraph/form/OCR previews and their callers as needed; new dialog/app tests.

Interfaces: modal takes adapter and page snapshot, calls `onApply(crop?)` once only for current verified preview, `onCancel()` otherwise. Crop draft uses four displayed margins. Parent checks page/session identity and permission before history commit.

- [x] Write failing tests for valid/invalid numeric input, reset, drag, preview failure/staleness, cancel, permission, and exactly one apply.
- [x] Build original-context preview with crop handles/margins and dimensions, exact native retained preview, explanatory text, reset/cancel/apply. Keyboard numeric alternative is required.
- [x] Wire page action/modal locking and one-step undo; clear hidden selection. Mark crop on source-context text/form/OCR previews or render actual cropped previews as appropriate.
- [x] Run focused frontend suite, then full frontend suite and build once at final source state.

## Task 4: Acceptance and review

- [x] Capture synthetic native crop corpus, check with independent reader; preserve inputs and exact source hashes.
- [x] Build fresh executable without replacing prior tested build, record matching before/after product fingerprints.
- [x] Exercise Windows crop preview/apply/undo/redo/rotate/save/reopen/edit workflow; capture screenshots and result PDFs; verify retained content and placement independently.
- [x] Request one scoped fresh implementation review, resolve demonstrated issues with regression evidence, and retain limits.
- [x] Update `docs/product-quality-loop.md`, this checklist, and `.superpowers/sdd/2026-10-24-page-cropping/progress.md`; do not claim overall parity.

Acceptance note: the independent matrix covers source text/graphics and portable
text; shared cropped widgets have native ownership/appearance checks. A full
cropped native-comment/form/overlay reader matrix remains an explicit coverage
gap. Review round1's three P2 interaction defects were corrected; round2 closed all three with no new actionable defects.
Current evidence: artifacts/page-cropping-20261024/verification.json.
