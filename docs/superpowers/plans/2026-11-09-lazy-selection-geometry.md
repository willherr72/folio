# Lazy Selection Geometry Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final fresh-context review. Preserve the existing worktree and all evidence; no commits, publication, cleanup or package promotion in this scoped milestone.

**Goal:** Reduce unnecessary browser layout during continuous existing-text edits without removing reading or selection capabilities.

**Architecture:** Compact semantic runs while nonselectable; existing positioned glyph DOM and measured geometry when selectable. Single/group Apply retain Edit mode, matching paragraph editing.

**Tech Stack:** Existing React/TypeScript, Vitest, native Windows WebView2 diagnostic and acceptance helpers.

**Spec:** docs/superpowers/specs/2026-11-09-lazy-selection-geometry-design.md

## Global constraints

Keep accepted 0.34 and its launcher untouched. No native/dependency changes or history reduction. Preserve exact source/copy policy and crop visibility. No forced GC or injected CSS in candidate resource acceptance. Serialize resource runs and compilation/tests.

## Review focus

- Crop-hidden text must not become accessible or selectable on tool changes.
- Rotation, coincident glyphs and scalar boundaries must retain correct selection/copy behavior.
- New-source asynchronous reads must not restore old text or transforms.
- Apply, Undo/Redo and explicit tool switching must use current source identity and retain all history.
- Post-close memory and scripting costs must be measured separately; no leak or general speed guarantee from a single pair.

## Task 1 — compact inactive text and continuous Apply

Modify src/components/PdfTextLayer.tsx, src/components/page-interactions.css and the two text Apply transitions in src/App.tsx. Add tests/lazy-text-selection.test.tsx and extend tests/text-edit-app.test.tsx.

- [x] Write regressions: inactive layers retain exact semantic text with no glyph nodes/layout reads; activating Select measures once and copies exact ligature/scalar text; returning inactive removes glyphs while reactivation reuses cached transforms; crop visibility and pending new-source isolation remain correct; single/group Apply keep Edit selected through Undo/Redo.
- [x] Run focused tests and retain expected pre-implementation failures.
- [x] Implement minimal conditional geometry, compact semantic runs and the two tool transitions. Do not alter extraction/native ownership.
- [x] Run focused selection/copy/crop/highlight/editing regressions, then full frontend with established 2-worker/5-second limits and TypeScript/Vite build. Preserve failures.

## Task 2 — source-bound desktop qualification

- [x] Capture product/native inputs before builds and require unchanged native implementation and renderer.
- [x] Build one optimized GUI candidate from verified frontend; record exact source, configuration and binary identity.
- [x] Adapt the generated-fixture driver for genuine candidate acceptance: no CSS injection/GC, assert Edit remains active after Apply, compare exact alternating images/text, switch Select/Highlight and verify geometry/copy, traverse Undo/Redo, save/reopen, close and verify owned-process cleanup. Match display environment for baseline/candidate performance observations.
- [x] Reconcile the scoped evidence, request one final review, correct consequential findings with reproducing checks, record checkpoint and remaining limits. Keep broad goal active.

## Ledger

Started 2026-11-09 in existing isolated t3/resume-pdf-editor worktree. Inline execution under user's standing autonomy. Initial normal-1 harness timed out because it omitted reactivating Edit after Apply; corrected normal-2 and hidden-1 passed all 25 edits. Containment spike is deliberately diagnostic and rejected for measured style cost. Original attempts remain preserved. No production change yet.

2026-11-09 execution: corrected red-2 fails six intended cases; focused-2 passes all 33 selection/geometry/editing cases. Focused-1 found a test-oracle mistake (Select uses active class rather than aria-pressed); corrected without changing product behavior. Native product comparison confirms exactly 1,290 unchanged native inputs and only three frontend product changes. Full-1 has no terminal result/report after an observation gap; handles missing and no owned test process remains. Its two unchanged form-text-controls failures pass isolated with the new tests (18/18). Full-2 is running at original 2-worker/5-second settings. Accepted 0.34/launcher unchanged.

Qualified 2026-11-09: full frontend 1489/224, optimized GUI and exact source checks pass. Both builds complete 105 edits, 100 Undo/100 Redo and save/reopen. Independent all-page RGB/glyph checks pass four pages at 72/144 dpi, with three corruption controls. Compiled selection-8 passes crop AX/copy at five display rotations and an actual first pointer Highlight gesture without CSS injection. All 58 observed process identities and private WebView profiles are absent. One final code review found no demonstrated defect; source/test hashes still match. Failed/interrupted attempts and oracle corrections are preserved in failure-dispositions.md. Existing instruction-banner hit obstruction is next, not fixed or hidden by this qualification. Accepted portable/launcher unchanged; broad goal active.
