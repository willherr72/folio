# Paragraph entry focus implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one scoped final review.

**Goal:** Put keyboard focus in the paragraph workspace upon entry and preserve later user focus choices.
**Architecture:** Shared workspace owns initial focus; styled/uniform dialogs supply loading state.
**Tech Stack:** React, TypeScript, Vitest, Windows WebView2 desktop acceptance.
**Spec:** docs/superpowers/specs/2026-10-30-paragraph-entry-focus-design.md

## Constraints

Preserve inherited worktree edits and all previous artifacts. No native editing
changes, dependencies, commits, package overwrites or user-app restarts.

## Review focus

Delayed inspection must not steal focus; selection refusal must remain reachable;
unmount must not later focus a dead editor; picker restoration must still work;
Modal teardown must not override the new workspace owner.

## Task: keyboard entry ownership

- [x] Reproduce both routes on the current desktop candidate.
- [ ] Add failing integration tests in tests/paragraph-entry-focus.test.tsx for both editor types.
- [ ] Implement ownership in src/components/ParagraphWorkspace.tsx and pass loading from StyledParagraphDialog/TextParagraphDialog.
- [ ] Run focused and full frontend tests, then TypeScript/Vite build.
- [ ] Freeze the scoped delta and request one independent review; fix confirmed findings locally.
- [ ] Build a fresh debug candidate and verify real keyboard entry, editing, Cancel/Apply and narrow-window behavior.
- [ ] Record exact evidence and remaining limits in the product ledger and checkpoint.
