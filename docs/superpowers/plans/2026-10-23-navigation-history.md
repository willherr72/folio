# Reading history implementation plan

> Use superpowers:executing-plans inline, with one fresh final focused reviewer.

**Goal:** Return to reading positions after deliberate navigation, without changing
PDF content or editing history.
**Architecture:** Pure bounded navigation stacks on each DocumentSession;
DocumentViewport captures/restores page-relative PDF points; App records successful
jumps and exposes controls and shortcuts.
**Tech Stack:** React/TypeScript, Vitest, Playwright acceptance.
**Spec:** ../specs/2026-10-23-navigation-history-design.md

## Constraints and review focus

Preserve source ownership, all previous uncommitted work, portable0.17 and recovery
format.100 entries per direction. No commits or publication. Existing isolated
worktree retained. Review rapid navigation/layout effects, delayed search, missing
pages after undo/delete, zoom/rotation coordinate math, and tab remount behavior.

## Task1: History model and viewport

- [x] Add failing tests in tests/navigation-history.test.ts for stack branching,
  limits, same views, deleted destinations and zoom; implement src/editor/navigation-history.ts.
- [x] Add tests in tests/viewport.test.tsx for synchronous capture and restored point
  after zoom/reorder/rotation/single-mode; implement optional view capture handle and
  point-based NavigationRequest in DocumentViewport. Report successful jump completion
  once, with captured origin/destination, to avoid duplicate/no-op entries.
- [x] Add navigationHistory to workspace sessions and initialize empty in recovery.
  Verify workspace/recovery tests and no dirty/undo impact.

## Task2: Application navigation

- [x] Add failing real-App tests for Back/Forward, tabs, search dismissal, edit-undo
  independence and guards. Centralize explicit navigation; record origin before jump
  and successful result after layout, revision guard against stale callbacks.
- [x] Add accessible buttons near page-nav and Alt+Left/Right with typing/modal guards.
  Keep deliberate edit-triggered page selections separate from reading jumps.
- [x] Run focused tests, create synthetic real-App browser fixture and exercise
  bookmark/search/page-jump/history at900x600, scrolling/zoom/rotation/reorder and tabs.

## Task3: Verification and review

- [x] Full frontend suite and production build. Recheck prior toolbar acceptance.
- [x] One fresh read-only focused review; reproduce/fix material findings once with
  regression evidence; no second review round.
- [x] Record evidence/limitations and remaining parity gaps in quality ledger.

## Completion evidence

603 frontend tests/93files pass; production and Windows debug builds pass. Real
Chromium and actual Windows WebView workflows pass, including all review fixes.
Three synthetic PDF hashes unchanged; owned GUI stopped. One fresh review and one
fix pass; no deferred findings. Evidence manifest artifacts/navigation-history-verification.json.
Native writer unchanged; readyportable0.17 preserved. Browser900x600 layout checks
and CDP900x600 desktop emulation do not establish an OS display-scaling matrix.
Navigation history intentionally resets on recovery and does not change its schema.
