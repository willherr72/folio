# Page Image Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline in the existing isolated worktree. Preserve all prior work; no commits or publication.

**Goal:** Export selected finished PDF pages to PNG/JPEG at explicit physical resolution, safely and without changing the document.
**Architecture:** Native per-page composition/rendering, an image-export permission intent, sequential bounded encoding in a new owned folder, cancellable IPC and a selection/progress dialog.
**Tech Stack:** Rust/PDFium/image/tempfile, Tauri channels, React/TypeScript/Vitest.
**Spec:** docs/superpowers/specs/2026-10-24-page-image-export-design.md

## Global constraints

Copy-content permission is native source authority; never use preview/print to bypass it. DPI72/150/300/600; JPEG quality1..100. Maximum24M pixels,12000pixels per dimension,128MiB encoded per image,1GiB batch. Current crop, rotation, native comments/forms and overlays must appear. No overwrite or PDF saved-state change. Cancellation token bypasses the serial PDF worker queue. Keep prior tested executable and private quiz unchanged.

## Review focus

- Decimal UserUnit, crop offsets and rotations must not silently reduce or stretch resolution.
- Print-enabled/copy-disabled sources and forged UI rights must never authorize exported pixels.
- Cancel during render/write, receiver loss, unmount and repeat actions must release job state and owned incomplete output.
- Oversize/malformed later pages must fail preflight before any output folder is created.
- Completed images must retain form/annotation appearance and have a white opaque background in both formats.

## Task 1: Native rendering and publication

Files: new src-tauri/src/page_image_export.rs (types/options/dimensions/publication helper), new src-tauri/src/page_image_render.rs (worker implementation), engine.rs (request/public entry), lib.rs exports, document_policy.rs intent. Tests: src-tauri/tests/page_image_export.rs and focused internal policy tests.
- [x] Add failing generated-fixture tests for real pixel dimensions/content, current overlays/crop/rotation/UserUnit, PNG/JPEG decode and source preservation.
- [x] Add policy/cancel/invalid-options/oversize/label/destination preservation tests.
- [x] Implement validation and native copy intent; render one composed page at a time with exact dimensions and bounded encoding.
- [x] Implement new-folder ownership, completion marker, ordinary-error/cancel cleanup, progress; run focused native tests.
- [x] Capture independent raster corpus and verify geometry/pixels/JPEG error and metadata externally.

## Task 2: Desktop command and user flow

Files: app.rs job state/commands; src/editor/page-image-export.ts API/options helpers; new ExportImagesDialog.tsx; App.tsx action/modal integration; targeted tests.
- [x] Add failing tests for selection/options/pixel bounds/progress/cancel/duplicate-submit and unchanged dirty/history state.
- [x] Register job IDs before started progress, cancel with atomic flag outside worker, remove jobs on all terminal paths. Native folder picker cancel is a no-op.
- [x] Add modal with native Channel progress, original page-number labels, resolution/format/quality controls and export result folder. Keep controls keyboard-accessible and narrow layout usable.
- [x] Run targeted tests then full frontend suite and TypeScript build.

## Task 3: Acceptance/review

- [x] Build fresh executable with before/after product fingerprints.
- [x] Windows export selected edited/cropped/rotated pages in both formats; verify outputs independently, cancellation/cleanup, keyboard/narrow layout, unchanged document saved state/source bytes.
- [x] Scoped fresh review; resolve concrete findings with regression evidence.
- [x] Update quality loop, ledger, plan and final verification record; retain compatibility/performance/hardware limits and active overarching goal.
