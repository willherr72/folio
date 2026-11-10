# Paragraph canvas implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, task by task. No implementation delegation or commits.

**Goal:** Edit and reflow existing paragraphs beside the main PDF viewport with movable, resizable areas and native comparison.

**Architecture:** Retain the uniform/styled controllers and native transactions. A shared workspace presents their controls and source-axis raster; PageView hosts accessible area controls. Candidate publication remains controller-owned, while raster presentation owns only its URLs.

**Tech Stack:** React, TypeScript, Vitest, Tauri/PDFium; existing Windows acceptance harness.

**Spec:** docs/superpowers/specs/2026-10-25-paragraph-canvas-design.md

## Global constraints

- Existing dirty worktree and inputs preserved; no commits, release or parity claim.
- Bounds use full source physical points, minimum width/height 0.01; crop clips interactions.
- Pointer threshold 3 CSS pixels; hit targets 24 CSS pixels; arrows 1 pt, Shift 10 pt.
- Preview after completed geometry changes only, debounced 350 ms. Text/format changes retain explicit Preview to avoid native work while typing.
- One native publication request at a time; stale completions release their source. Zoom rerenders the same source at source-width zoom/DPR, bounded 600..2400.
- Standalone modal presentation remains supported. Nested font picker remains modal.
- One design review (completed, no unanswered questions) and one final correctness review/correction pass.

## Review focus

- Deferred native replies after invalid numeric edits or cancellation must never enable Apply (task 2).
- Zoom during pending raster work must not release a candidate still being rendered (task 2).
- Cropped/rotated pages must map screen gestures once and hide excluded focus targets (tasks 1/3).
- Page navigation and single-page virtualization must retain the editing draft (task 3).
- Font picker Escape and local undo must not close/mutate the document session (tasks 2/3).

### Task 1: Area geometry and accessible SVG controls

Files: new src/editor/paragraph-canvas.ts, src/components/ParagraphCanvasControls.tsx and focused tests.
Interface: ParagraphHandle = move|n|ne|e|se|s|sw|w|nw; changeParagraphArea(bounds,handle,delta,pageWidth,pageHeight): AnnotationRect. CanvasParagraphView carries page/session identity, area/pieces/selection, source-axis raster, enabled/choosing state and onChange/onGestureActive/onToggle callbacks.

- [x] Write and run failing geometry tests: clamp all edges/corners without crossing fixed edges; move clamps; minimum .01; nonfinite input retained.
- [x] Implement geometry and test all rotations through existing screenArrowDelta/imagePointerPoint.
- [x] Write controls tests for release-only commits, grab offset/threshold, competing pointer, capture loss/Escape/zoom cancellation, keyboard and crop-excluded handles.
- [x] Implement source-axis controls and verify focused tests. No automatic focus stealing.

### Task 2: Shared workspace and controller presentation

Files: new ParagraphWorkspace.tsx/CSS and paragraph workspace tests; modify both paragraph dialogs and paragraph-preview.ts.
Interface: optional canvas prop {pageId,sessionKey,zoom,renderCanvas(view):ReactNode,onLocate,onModalChange}; shared workspace accepts original/candidate identity, bounds and callbacks, fields/footer and picker. Source-axis raster hook owns original/candidate URLs and reports candidate readiness. Modal default keeps ParagraphPagePreview.

- [x] Write failing controller tests for nonmodal controls, full bounds updates, stale invalid field rejection and nested picker preservation.
- [x] Extract a shared shell without duplicating editor state. Keep picker mounted beside workspace, use its Modal to block the background.
- [x] Serialize native requests independently of invalidation generation; on stale completion release then clear busy state. Debounce only geometry revisions, coalesce latest request. Keep custom-font leases through pending native work.
- [x] Add source-axis raster lifecycle with exact URL disposal, bounded zoom rendering, candidate render readiness, retry and original/edited comparison. Retain source until any dependent raster settles.
- [x] Verify candidate zoom/late reply/unmount/invalid field/selection-only tests and existing modal tests.

### Task 3: Main viewport integration

Files: App.tsx, DocumentViewport.tsx, PageView.tsx and integration tests.

- [x] Write failing tests for pinned target mount, source-axis raster under rotation/crop, no overlay double paint, session draft across reading navigation and blocked competing mutations.
- [x] Render active paragraph controller in the main workspace instead of modal root; separate paragraph editing from readingBlocked while keeping mutations blocked. Preserve tool during history travel and focus Edit text on close.
- [x] Pass CanvasParagraphView through viewport; pin target mounted, hide away in single-page mode, suppress existing-text hit targets while active. Render borrowed source-axis raster at source dimensions before one SVG page transform; composite overlays only once.
- [x] Wire Locate, nested modal blocking, Escape precedence and local typing Undo. Verify frontend suite and TypeScript build.

### Task 4: Acceptance and checkpoint

- [x] One final scoped review; reproduce and correct demonstrated findings once, retaining all dispositions.
- [x] Run full frontend and relevant native paragraph regressions; build fresh fingerprinted Windows checkpoint.
- [x] Desktop acceptance: mixed/uniform text insertion, formatting, pointer/keyboard area changes, reflow, native comparison, invalid draft, zoom/navigation, picker, narrow controls, Cancel/Apply/document Undo/Redo, save/reopen/re-edit.
- [x] Authored rotated/cropped/UserUnit duplicate fixture with annotations/highlights plus local-only private quiz insertion. Independent reader checks text/style/linewrap/geometry/outside pixels/siblings and unchanged originals.
- [x] Record exact hashes, outcomes and limitations; stop only owned test process; update quality ledger and checkpoint link. Keep broad goal active.

## Completion evidence and limits

See artifacts/paragraph-canvas-20261025/checkpoint-corrected/verification.json and execution.md. Native APIs unchanged. All review findings corrected with red/green evidence; no second review. Desktop uniform fallback, broader accessibility/printing/recovery/producer matrices remain unclaimed. First desktop/quiz/verifier harness failures are retained with corrections.
