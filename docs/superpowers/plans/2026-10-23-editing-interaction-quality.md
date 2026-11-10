# Editing interaction quality implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, then one fresh whole-pass review. Preserve unrelated changes and the verified portable checkpoint.

**Goal:** Make editing limits understandable and verify delayed results cannot alter closed or newly focused workflows.

**Architecture:** Share the existing paragraph limit between the dialog and preview; retain generation-based candidate ownership and existing image insertion flow.

**Tech stack:** React/TypeScript, Vitest/Testing Library, Vite and browser acceptance.

**Spec:** `docs/superpowers/specs/2026-10-23-editing-interaction-quality-design.md`.

## Constraints and review focus

- Exactly64 paragraph pieces; selected pieces stay removable at the limit.
- Keyboard and pointer activation agree with announced disabled state.
- Cancelled annotated rendering releases its URL/source once, without adoption.
- Pending image picker and Find results cannot resurrect a closed flow or retarget insertion.
- Keep0.15.0-dev.1 package intact; do not claim a native or packaged change from UI-only proof.

## Task1: Paragraph limit interaction

Files: `src/editor/paragraph-preview.ts`, `src/components/ParagraphPagePreview.tsx`,
`TextParagraphDialog.tsx`, `paragraph-page-preview.css`; tests `paragraph-page-preview.test.tsx`,
`text-paragraph-dialog.test.tsx`.

Interface: export `MAX_PARAGRAPH_PIECES=64`; use selected membership and count to
derive disabled state, tabIndex and event guards. Dialog uses the same constant for
checkboxes, its toggle guard, count and status guidance.

- [x] Write and observe RED for disabled unselected targets at64, all activation types,
  selected deselection and reenabled targets after count drops.
- [x] Implement minimal behavior and visible status/style; run focused tests GREEN.

## Task2: Delayed-result proof and image guidance

Files: `tests/text-paragraph-dialog.test.tsx`, `tests/image-pages.test.tsx`,
`src/components/ImagePagesDialog.tsx`; existing lifecycle modules only if tests fail.

- [x] Add controlled deferred composition test after candidate creation, cancel/unmount,
  then resolve blob URL: no adoption and exactly-once source/URL cleanup.
- [x] Add pending-picker unmount and late-search image-insertion regressions.
  Picker unmount is lifecycle smoke coverage; React ignores unmounted state writes,
  so it does not directly prove the mounted guard. Code inspection verifies the guard.
- [x] State32,768 pixels per dimension in import guidance; run related suites.

## Task3: Browser, review and evidence

- [x] Add isolated real-component fixture; verify limit keyboard/mouse behavior and
  image guidance/footer layout at900x600 in a real browser.
- [x] Run full frontend suite and production build; review this bounded pass once.
- [x] Resolve material findings with regressions; update quality ledger and remaining gaps.
