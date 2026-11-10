# History-bound text removal implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Let users mark existing text, inspect its removal, Apply once with Undo/Redo, and save a copy without bypassing unsaved document changes.

**Architecture:** A pure document transaction plus an owned async controller wrap the verified native session. A dedicated modal paints source/candidate rasters and measured glyph bounds. App adopts only against the captured active tab/history and uses existing Save.

**Tech Stack:** TypeScript/React/Vitest; existing Tauri/Rust native commands; Windows desktop acceptance and local independent PDF readers.

**Spec:** docs/superpowers/specs/2026-10-30-text-removal-workflow-design.md

## Global constraints

- Exact adapter/tab/history identity,1..1000pages,1..64marks,≤1000boxes per bounds request.
- Prepared geometry tolerance0.01pt; reject invalid source IDs, protection, unreadable bookmarks and overlay DTOs.
- Keep ALL original sources through undo history; clear baked transforms/overlays after adoption.
- Cancel may abandon any await; cleanup never closes borrowed/unknown result IDs.
- Source glyph masks are streamed to bounded canvas, no giant SVG/geometry cache.
- No extra dependencies, original file writes, release/secure-area/parity claims or routine permission requests.
- Preserve inherited dirty work, use frozen baseline/evidence instead of commits/cleanup. One native build at a time.

## Review focus

1. Late Apply after Cancel or stale history must close only the known candidate and leave every tab intact.
2. Malformed adapter IDs/DTOs or later mutation must not corrupt history or close another tab's source.
3. Baked crop/rotation/overlays must not be applied twice; bookmarks omitted by native output must not reappear.
4. Bounds pagination/render errors or a preview from previous marks must never enable Apply.
5. React StrictMode, pointer cancellation, numeric empty input and narrow layout must preserve cleanup and keyboard operation.

### Task 1: Transaction and owned async controller

**Files:** create src/editor/text-removal-workflow.ts; tests/text-removal-workflow.test.ts.

**Interfaces:** RemovalRequest captures {tabId,adapter,history,document}; captureTextRemoval(session:DocumentSession):RemovalRequest; supportsTextRemoval(adapter):boolean; commitTextRemoval(session,request,info:DocumentInfo):DocumentSession. TextRemovalWorkflow(request,{isCurrent():boolean,isBorrowed(id):boolean}) exposes prepare():Promise<TextRemovalSource>, preview(marks):Promise<TextRemovalPreview>, invalidate():void, bounds(preview,page,offset):Promise<TextRemovalBounds>, apply(preview,adopt:(info,preview)=>boolean):Promise<void>, dispose():void. Store owned copied DTOs privately; returned values must not be mutable authorization state.

- [x] RED: prepared/current page mapping, one commit, exact Undo/Redo and original source retention; reject stale history/adapter/tab and malformed output.
- [x] Implement capture/validation/commit with returned physical sizes and output bookmarks mapped to preserved page IDs. Keep savedDigest/name; clear baked state.
- [x] RED: Cancel at Begin/Prepare/Preview/Apply awaits, stale request, wrong/borrowed IDs, settled DTO mutation, double Apply, failed Apply cleanup, pagination mismatch. Implement controller with serialized operation guard, queued discard and idempotent release.
- [x] Run npx vitest run tests/text-removal-workflow.test.ts --maxWorkers=2 --minWorkers=1; require green. Record RED/GREEN evidence.

### Task 2: Accessible marking and preview modal

**Files:** create src/components/TextRemovalDialog.tsx, text-removal.css, TextRemovalGlyphMask.tsx, src/editor/use-source-raster.ts; tests/text-removal-dialog.test.tsx.

**Interfaces:** TextRemovalDialog({request,isCurrent,isBorrowed,onApply(info,preview):boolean,onCancel():void}); TextRemovalGlyphMask({workflow,preview,pageIndex,width,height,onReady(ready):void}); useSourceRaster(adapter,info|null,pageIndex,width=900) returns {url?,loading,error?}. Mark generation belongs to dialog; workflow.invalidate clears native preview authority.

- [x] RED: preparation/cancel, keyboard rectangle creation, pointer clamping/cancel,64mark bound, navigation, preview invalidation, refusal, metadata acknowledgement and successful Apply.
- [x] Implement latest-only raster lifecycle with blob cleanup; source/candidate side by side. Stream validated bounds in1000box chunks to canvas with maximum dimensions4096 and nominal source width900; dispose outdated paints.
- [x] RED: render/bounds errors, expiry with explicit working-copy refresh, late returns, StrictMode ownership, acknowledgement reset and empty numeric fields. Implement disabled states and explanatory copy; keep Cancel available.
- [x] Run focused dialog/controller tests and TypeScript build; require pass. Use keyboard and pointer assertions rather than testing implementation details.

### Task 3: App integration and save-copy history

**Files:** modify src/App.tsx; create tests/text-removal-app.test.tsx.

**Interfaces:** App captures request on native "Remove text…" entry, includes dialog in modalOpen, queries latest workspace in callbacks, adopts synchronously via commitTextRemoval then closes. Full capability gate and all-page authorization. Existing Save a copy remains publication route.

- [x] RED: edited/duplicated page plans supplied, candidate pages adopt once, Undo/Redo exact, sources retained, save-copy gets candidate plans and output bookmarks. Add second-tab stale-return case and Apply failure.
- [x] Implement entry/dialog callbacks without global adapter fallback; original creating adapter owns cleanup. Clear search on entry and selections on adoption; notice directs Undo/save-copy.
- [x] Run all focused tests; then full frontend with two workers and TypeScript/Vite build under stable product fingerprint.

### Task 4: Native desktop acceptance and final review

- [x] Build fingerprinted local debug candidate using established scripts; no running user app restart. Exercise synthetic PDF Preview/Cancel/Apply/Undo/Redo/save/reopen plus refusal and second-tab checks in a separate process.
- [x] Independently audit saved PDF text/glyph positions/raster and original bytes. Verify final source fingerprint before accepting evidence.
- [x] Freeze scope/baseline/evidence; one independent final review. Correct material findings with RED/GREEN and appropriate full suite; no re-review.
- [x] Update plan/ledger/roadmap/checkpoint, retain limits, leave overall goal active.

Acceptance limits and retained coverage gaps are recorded in artifacts/text-removal-workflow-20261030/checkpoint.json; checked tasks mean scoped implementation/gates, not exhaustive combinations.
