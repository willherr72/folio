# Readable paragraph number controls implementation plan

> Use superpowers:executing-plans inline in the existing isolated worktree.

**Goal:** Compact idle number presentation with exact untouched paragraph values.
**Architecture:** Shared PreciseNumberInput wraps native number inputs; existing dialog draft/model ownership stays unchanged.
**Tech stack:** React/TypeScript, Vitest/Testing Library, current Tauri desktop and Windows dialog harness.
**Spec:** docs/superpowers/specs/2026-11-03-paragraph-number-display-design.md

## Constraints
No model rounding or unsolicited native formatting changes. Exact active drafts, valid tiny values, empty invalid drafts and reset/undo behavior must survive. Preserve private originals and accepted package. No native source changes planned.

## Task1 - numeric presentation and regression
Files: new src/components/PreciseNumberInput.tsx, tests/precise-number-input.test.tsx; existing StyledParagraphDialog.tsx, TextParagraphDialog.tsx and their tests.
- [x] Write and observe failing compact display/focus/no-emission controls and dialog request precision tests.
- [x] Implement shared display input and adopt it for both paragraph dialogs; retain exact spacing draft.
- [x] Verify focused tests including high-precision edits, blur, bounds/reset/undo and untouched native request identity.

## Task2 - integration evidence
- [x] Full frontend tests and fresh build; compare product delta and unchanged native test inputs.
- [x] Build current desktop and run quiz workflows with explicit idle/focus precision checks and independent saved-output audit.
- [x] One fresh integrated review; correct Important findings once, retain limits/minors and exact evidence.
- [x] Update roadmap/checkpoint and leave broader goal active.

## Review focus
- Focus/blur must not emit onChange, round request geometry or introduce history entries.
- User-entered precision, invalid/empty drafts and resets must survive active/idle transitions.
- Small nonzero values and numeric bounds must not display zero or change native validation semantics.
- Native number-field focus/caret/browser behavior requires actual desktop evidence, not only jsdom.
- Uniform and styled paths must retain original size/spacing omission and baseline behavior.
