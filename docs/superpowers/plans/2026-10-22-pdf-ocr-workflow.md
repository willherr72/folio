# PDF OCR workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Turn scans into reviewable, correctable, searchable PDFs with preserved appearance and history.

**Architecture:** Native preparation and immutable transformation surround the reviewed
local recognition worker. A verified invisible Form owns the searchable text; full
source replacement preserves forms and protection, and one workspace commit owns Apply.

**Tech Stack:** Rust/PDFium/lopdf/semantic TrueType, TypeScript/React/Tesseract worker,
native and Vitest tests, independent Python readers and Windows acceptance.

**Spec:** `docs/superpowers/specs/2026-10-22-pdf-ocr-workflow-design.md`

## Global Constraints

- Edit-content and copy-content rights required natively; no prepared print/signed/XFA mutation.
- nominal300DPI, <=6000pixels/side,20millionpixels,32MiB PNG; hints72–600DPI.
- <=100pages,500,000words/2,000,000scalars per batch; per page20,000words/100,000scalars.
- Existing source files, histories and font ownership remain unchanged until atomic adoption.
- Semantic OCR font <=65,534definitions; original shaped-text font <=4096definitions unchanged.
- Original page content decode <=16MiB/1million operations; output <=512MiB.
- Keep existing worktree/uncommitted work and portable checkpoint; one final workflow review.

## Review Focus

- Shared original page occurrences must stay consistent without breaking cross-page form identity.
- A forged OCR marker must not authorize deletion of visible content or unrelated invisible text.
- Page rotation and CropBox must affect glyph placement, exclusion boxes and reopened review equally.
- Mixed-page native headers must remain selectable exactly once while the scanned body gains text.
- Cancel/stale tab/partial multi-source failure must release candidates without history or font leaks.

### Task1: Bounded native OCR model and semantic transport

**Files:** create `src-tauri/src/ocr_types.rs`, `src-tauri/src/ocr_pdf.rs`,
`src-tauri/src/ocr_font.rs`; modify `semantic_font.rs`, `lib.rs`; tests
`src-tauri/tests/ocr_pdf.rs` plus private validation tests.
**Interfaces:** exported `OcrBounds`, `OcrWord`, `OcrLine`, `OcrParagraph`, `OcrBlock`,
`OcrPage`, `OcrUpdate{page_index,rotation,page}` mirror camelCase JS model.
`ocr_pdf::replace(document:&mut Document,page_id:ObjectId,mapping:OcrMapping,
page:&OcrPage)->EngineResult<()>`; `read_owned(document,page_id)` returns verified
record or None, never infers ownership from a marker alone. Internal mapping provides
PDF point origin/right/up vectors and original raster dimensions.

- [x] Write RED validation/transport tests: Unicode/geometry/budgets, deterministic
  font, scan pixels unchanged, repeated correction without duplicate text/resources,
  corrupted marker/font/content remains opaque, inherited/shared resources isolated.
- [x] Run focused Cargo tests and observe expected missing-code/behavior failures.
- [x] Implement semantic font entry point, bounded data, deterministic Form, ownership
  regeneration comparison, balanced-content isolation and safe replacement.
- [x] Run focused tests; independently inspect resulting PDFs and record fidelity evidence.

### Task2: Native preparation, permissions and immutable publication

**Files:** create `src-tauri/src/ocr_engine.rs`; modify `engine.rs`, `app.rs`,
`lib.rs`; tests `src-tauri/tests/ocr_engine.rs`, protected/recovery regressions.
**Interfaces:** `PdfEngine::prepare_ocr_page(&str,usize,u16)->EngineResult<OcrPreparedPage>`
with PNG, dimensions, effectiveDpi, exclusion bounds, owned record;
`PdfEngine::apply_ocr(&str,Vec<OcrUpdate>)->EngineResult<DocumentInfo>`.
IPC `prepare_ocr_page` and `apply_ocr` forward validated native calls.

- [x] Write RED tests for native and extra rotations/CropBox, mixed text filtering,
  restricted-reader rejection/owner success, original immutability, shared forms and
  annotations/bookmarks, output/recovery policy, dropped-reply cleanup, malformed/stale input.
- [x] Implement raster/mapping/exclusions, full-source mutation and pixel/text verification,
  then `open_bytes` with inherited SourcePolicy, no new global font registrations.
- [x] Run targeted native suites and independent reader/pixel verification at all rotations.

### Task3: Recognition review and atomic workspace adoption

**Files:** `src/editor/adapter.ts`, `types.ts`; create `ocr/workflow.ts`,
`ocr/useOcrWorkflow.ts`, `components/OcrDialog.tsx`; integrate `App.tsx`, styles and
existing scan-text guidance. Tests `tests/ocr-workflow.test.ts`, `ocr-dialog.test.tsx`,
`ocr-app.test.tsx`.
**Interfaces:** adapter `prepareOcrPage(sourceId,pageIndex,rotation)` and
`applyOcr(sourceId,updates)` mirror Task2; OcrDialog selects ranges/languages and owns
the draft worker/results, parent adopts only verified candidates once.

- [x] Write RED tests for range validation, language/progress/cancel, uncertain-word
  selection/correction/removal, mixed exclusions, existing owned text, repeated copies,
  stale state and partial failure cleanup, one-step undo/redo and dirty state.
- [x] Implement accessible dialog and hook with one raster/session, page navigation,
  visible scan/word boxes, precise errors, and no Apply for an unchanged/empty batch.
- [x] Connect toolbar/menu and keyboard/busy state; keep source-text editing protection
  for invisible OCR, and explain that recognition corrections preserve scanned ink.
- [x] Run relevant UI suites, production build and complete frontend suite.

### Task4: Desktop workflow, review and checkpoint

**Files:** acceptance scripts/fixtures, OCR documentation, progress ledger and release notes.

- [x] Verify actual desktop scan→recognize→correct→apply→Find→undo/redo→protected save/reopen;
  cancel setup, recognition and review, and force-exit recovery. Applying-stage cancellation
  and late candidate disposal are verified in hook/native tests, not a timed desktop race.
  Independently compare pixels/text/permissions
  and original hashes. Exercise the user's quiz as an already-native-text no-duplication case.
- [x] Run full native/frontend/build checks appropriate to the final integration.
- [x] Request one fresh whole-workflow review; resolve material findings with RED/GREEN.
- [x] Increment versions and build a new local portable checkpoint, verify its embedded
  assets and desktop flow, record remaining gaps and keep the broad goal active.
