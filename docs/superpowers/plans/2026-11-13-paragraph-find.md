# Paragraph find and replace implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. User opted out of questions; preserve managed worktree and artifacts. One final review; one Important fix pass; no re-review/commit/cleanup.

**Goal:** Navigate and replace repeated text within the current styled paragraph, preserving its source-bound formatting and history.
**Architecture:** Pure grapheme-aware match/navigation helpers and an atomic editor transaction, then a local input/panel wrapper in the styled dialog. Native publication remains unchanged.
**Tech Stack:** TypeScript/React/Vitest, existing Windows owned-app harness.
**Spec:** docs/superpowers/specs/2026-11-13-paragraph-find-design.md

## Global Constraints

4096UTF8bytes/64styles; LF normalization; immutable current source; final atomic capacity/style checks; literal replacement. No version/dependency/native/renderer/permission change. Keep selected050 and prior paste evidence/OSclipboard gate. Public authored fixtures only; no private corpus. Two frontend workers/default5000ms; one Cargo job/D:cache/temp. No commit/merge/push/promotion/deletion/public release.

## Review Focus

- Unicode simple folding/word boundaries cannot authorize inside-grapheme edits.
- Replacement spanning derived fonts/highlights preserves outside styles and original source identity.
- Rapid query/draft/history changes cannot use stale ranges or partially apply a batch.
- Keyboard Find/Escape ownership cannot cancel the workspace or steal composition.
- Selection-only navigation preserves preview; real replace invalidates it and remains oneUndo.

### Task1: match model and atomic replacement

Files: new src/editor/paragraph-find.ts and tests/paragraph-find.test.ts; src/editor/styled-paragraph.ts, src/editor/styled-paragraph-editor.ts; tests/styled-paragraph-editor.test.ts.
Interfaces: findParagraphMatches(text,query,{matchCase,wholeWords})->readonly{start,end}[]; nextParagraphMatch(matches,selection,direction)->range|null; editor replace-many transaction consumes checked sorted nonoverlapping nonempty ranges and literal text, commits once.
- [x] Write failing tests for every pure/model behavior and listed failure modes, observe RED, implement, require focused GREEN.

### Task2: local Find panel

Files: new src/components/ParagraphTextEditor.tsx and tests/paragraph-text-editor.test.tsx; StyledParagraphDialog.tsx; existing-text.css; styled-paragraph-dialog tests.
Consumes Task1 helpers/transaction; produces currentparagraph-only panel/shortcuts and existing StyledParagraphInput callbacks.
- [x] Add RED tests for visible/shortcut panel, case/word/wrap, exact selection, Replace/Replaceall, immutable source reset, busy, preview/history, Escape/composition. Implement and run focused tests.

### Task3: full acceptance

Files: artifacts/paragraph-find-20261113; this plan/ledger/livequalitydoc after evidence.
- [x] Capture exact source/native/shared/resource/test inputs; fresh full frontend and normal optimized GUI.
- [x] Actual compiled keyboard Find/Replaceall, UndoRedo/PreviewApply/save/reopen publicstyledPDF; independent fulltext/font/style/outsidepixels and owned cleanup. Preserve previous paste transport gate.
- [x] One final scoped review; Important RED/GREEN fix pass/fullsuite; exhaustive declined-behavior rulings/minor deferrals; record bounded completion/nextwork while broadgoal active.

Completed component evidence: source31eb0aebd1021d20721b0e4abe6bdedd7be7c3aa1c343d7396b6ea0e69e68954;
frontend-full-5:1630passed/234files/default5000ms/two workers/guards0;
review/fix-red:5behavioral failures then fix-green1:148passed/8files;
normal optimized build5 and desktop5 completed/owned stopped, including both
review findings/nested font modal/first and second Escape, plus independent
mixed-style saved PDF and three rejected corruptions. Earlier failed harness
attempts remain preserved; desktop4 assumed reset Find options after Close,
but options intentionally persist. Desktop5 explicitly sets options before
asserting counts and reuses the same exact corrected binary. No re-review.
See artifacts/paragraph-find-20261113/component-completion.json. Whole-document,
installed IME/accessibility and portable integration are separate scope. Prior
paste actual OS Ctrl+V gate stays OPEN; the broad quality goal stays active.
