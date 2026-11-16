# Paragraph keyboard scope implementation plan

> **For agentic workers:** superpowers:executing-plans inline. Existing managed worktree; user opted out of routine questions. No commit/cleanup/new checkout. One final fresh review, one Important fix pass, no re-review.

**Goal:** Make paragraph Find shortcuts work after formatting or positioning without disturbing drafts, previews or nested dialogs.
**Architecture:** One render-current native capture owner at the nearest paragraph workspace/modal, wrapper fallback; existing immutable editor/search model and native transactions remain.
**Tech Stack:** React/TypeScript/Vitest, existing Rust/Tauri GUI and Windows WebView2 QA.
**Spec:** docs/superpowers/specs/2026-11-13-paragraph-keyboard-scope-design.md

## Global Constraints

Preserve source051 checkpoint56eadb915adb7cb9f3181878e54dbb48a4b31f47295a39ce22e817859d05c8a5, its executables, selected050 and open systemCtrlV/IME gate. No clipboard writes/private fixtures/commit/merge/push/deletion/portable promotion. Frontend default5000ms/two workers. Normal optimized GUI,26resource bytes unchanged; Cargoonejob/D:cache+temp; serialize frontend/build/desktop. Native equality is qualified reuse only.

## Review Focus

- Repeated renders must use current query, match list, selection and disabled state without duplicate dispatch.
- Nested modal portals must retain Ctrl+F/H/F3/Escape and composition ownership.
- Sibling numeric/select controls must open/navigate Find without consuming ordinary typing or changing paragraph/history/preview.
- Unmount/reopen/new-source transitions must remove old scope listeners.
- Real compiled keyboard results must identify fresh source/executable/resources; CDP events cannot close physical keyboard/IME/system clipboard gates.

### Task1: scope ownership and regressions

**Files:** src/components/ParagraphTextEditor.tsx; tests/paragraph-text-editor.test.tsx; tests/styled-paragraph-dialog.test.tsx if needed.
**Interfaces:** Keep ParagraphTextEditor props and immutable StyledEditorState API. Ownership uses closest `.paragraph-workspace-panel, [role="dialog"]`, falling back to the editor host.
- [ ] RED: Ctrl+F/H from sibling input/select/button; F3/ShiftF3 with latest matches after rerender; modal/fallback and unmount cleanup; nested portal/default-prevented/composition/disabled/modifier negatives.
- [ ] GREEN: consolidate key handling into one capture listener using current closures; remove wrapper duplication; preserve existing first-Escape behavior.
- [ ] Focused suite, then fresh full frontend; retain raw reports/source/input guards.

### Task2: current-source normal build and Windows workflow

**Files:** artifacts/paragraph-keyboard-scope-20261113/{baseline,source,build-1,desktop-1,qualification helpers}.
**Interfaces:** Verified051source is baseline; exact declared frontend delta and unchanged native/shared inputs; current candidate binds actual optimized GUI/source/resources.
- [ ] Capture baseline/inputs and exact source transition; preserve051 originals and all failed attempts.
- [ ] Normal optimized GUI build; source/input guards after build.
- [ ] Compiled public mixed paragraph: sibling control Ctrl+F/H/F3/ShiftF3, current query after rerender, Escape, nested picker isolation, native Preview retention, Apply/history/save/reopen and independent PDF/control audit. No clipboard use.

### Task3: final component review and checkpoint

- [ ] Freeze complete component review inputs. One fresh most-capable native reviewer; no tests/apps/builds by reviewer.
- [ ] Re-grade, one Important RED/GREEN fix pass, full affected suite; ledger all declined judgments and deferred minors.
- [ ] Source/runtime/output-bound component completion and checkpoint; keep selected050 and reviewed051 package unchanged, system paste open and broad goal active.
