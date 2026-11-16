# Cancellable document opening implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL superpowers:executing-plans inline.
> One final scoped review, no per-task delegates. Standing autonomy; preserve the
> managed branch, worktree and evidence.

**Goal:** Return control promptly after cancellation while retaining existing work and disposing late open/unlock results.
**Architecture:** A pure Promise/AbortSignal boundary handles cancellation and late disposal. The existing password coordinator owns sessions and preparation; App presents cancellation. The native open command receives its invoking Tauri window and parents the file picker to it, without changing client arguments.
**Tech Stack:** React/TypeScript, Vitest, Rust/Tauri candidate and owned Windows/WebView acceptance.
**Spec:** docs/superpowers/specs/2026-11-12-cancellable-document-open-design.md

## Global Constraints

Selected046 source665a061b13cd7c0942efe7b0e0dd33569553604fc5c14ba660b69b582fbba3df is the baseline. The only permitted Rust delta is invoking-window injection and picker parenting in `begin_open_pdf` in `src-tauri/src/app.rs`. No engine, dependency, renderer, client argument, version metadata or password format changes. Run fresh native validation and a new guarded build for this extension; preserve the earlier frontend-only evidence as earlier evidence. Frontend default timeout5,000ms/two workers. One Cargo job and target/temp on D:, serialize builds/native/UI. Preserve selected046 and every failed attempt. No commits/push/merge/publication/cleanup/normal app auto-launch. Original OCR timeout stays unresolved unless separately reproduced and fixed.

**Scope extension from actual acceptance:** `picker-ownership-1` proved that the unparented Windows picker leaves Folio enabled and survives app cancellation. Parent the real picker, then require its owner to match Folio and Folio to be disabled during the modal picker. Native Cancel must restore the UI. Cancellation after picker closure remains covered by the held-response cases. Do not interact through an OS-disabled parent.

## Review Focus

- Cancel during preparation or a native reply must release the UI without accidentally starting a late picker or swallowing normal errors.
- Late opened sources/imported fonts and owner replacement sources must be released exactly through existing ownership rules, without touching a newer session.
- Cancel and Escape must remain accessible in empty and dirty workspaces, restore useful focus, and never mutate existing histories.
- Real native response interception must be calibrated; a fake response or zero-event trace cannot prove real source cleanup.
- Preserve all26 resource bytes, native test inputs and original profile meanings; the native ownership extension requires fresh execution, while selected046 and the unresolved OCR limitation remain honest.

### Task 1: cancellation-aware session lifetime

**Files:** create `src/editor/await-or-cancel.ts`; modify `src/editor/usePasswordWorkflow.ts`; test `tests/await-or-cancel.test.ts`, `tests/password-workflow.test.tsx`, `tests/password-app.test.tsx`.
**Interfaces:** `awaitOrCancel<T>(work:Promise<T>,signal:AbortSignal,dispose:(value:T)=>void|Promise<void>)` returns a Promise of `{cancelled:true}` or `{cancelled:false,value:T}`. Coordinator `open(adapter,prepare?:()=>Promise<void>)` exposes `opening:boolean`; `cancel()` reports whether a live session was cancelled. Existing `unlock/submit/prompt` interfaces remain compatible.

- [ ] Capture exact046 source/native/shared baselines and assert selected package identities. Expected: unchanged authority and isolated worktree.
- [ ] Add a regression that cancels password verification and expects the open callback/UI to settle **before** resolving the native reply. Run it first. Expected: FAIL on current waiting behavior, not import/config errors.
- [ ] Add pure cancellation/normal-error/late-value/late-error/abort-race tests, then implement the helper and coordinator. Expected: prompt cancellation, no unhandled late rejection, late cleanup, no stale prompt dismissal.
- [ ] Cover optional preparation cancellation, immediate subsequent sessions, native adapter self-cleanup, ignored signals and late owner replacements. Expected: no late picker, no double disposal or workspace installation.
- [ ] Run focused helper/workflow/password-app tests. Expected: all pass with original timeout; record exact command/output. Keep all work uncommitted under standing instructions.

### Task 2: accessible opening recovery controls

**Files:** modify `src/App.tsx`, `src/styles.css`; create `tests/open-cancellation-app.test.tsx` and extend relevant app regressions.
**Interfaces:** consume Task1 `opening`, cancellable preparation and `cancel()` result. Render one **Cancel opening** action outside password modals; no adapter/native signature changes.

- [ ] Add failing empty-workspace and dirty-workspace tests with pending open/preparation. Expected: absent Cancel/action or blocked state proves current gap.
- [ ] Move existing preparation into coordinator calls, add Cancel/Escape and focus restoration, with compact status-bar styling. Expected: existing document history is intact and current controls re-enable before the native reply.
- [ ] Cover Add PDF cancellation, native errors, stale completions after a newer prompt, normal picker cancellation, password-modal ownership of Escape, and unmount. Expected: no unwanted tabs/pages and cleanup follows existing resource rules.
- [ ] Run focused application/password/OCR/workspace-source suites and build TypeScript/frontend. Expected: all pass; UI controls fit both empty and existing views.

### Task 3: verify the actual candidate and review

**Files:** create guarded source/input/test/build/runtime evidence under `artifacts/cancellable-document-open-20261112`; scoped review and final component record.
**Interfaces:** changed frontend source -> fresh full frontend/build -> exact candidate-owned native response delay/cancellation/cleanup + public OCR acceptance.

- [ ] Run fresh full frontend suite (default5,000ms/two workers), source/native/shared input guards and exact change scope. Expected: all pass; no native code/resource drift or false fresh-native claim.
- [ ] Build normal optimized GUI/custom protocol and default CLI using the established candidate command/resource-copy workflow, one job/D: caches. Expected: guarded new candidate, all26 renderer resources, source fingerprint and helper pins stable; selected046 unchanged.
- [ ] Calibrate CDP response interception against an actual native open; cancel while a real opened response is held, release it and verify native close plus absence from workspace. Test empty/current dirty workspace, preparation/prompt cancellation and 900x600/1280x820 usability. Expected: immediate UI recovery, preserved Undo/Redo, exact native ownership cleanup; no mock/native-preemption claims.
- [ ] Run public observed OCR acceptance on the exact candidate and independently audit saved outputs. Expected: behavior retained; the earlier unreproduced timeout remains explicit.
- [ ] Freeze scoped evidence, dispatch one final review, disposition findings and apply at most one consequential correction pass with RED-to-GREEN/appropriate suites. Expected: honest source/input/runtime coverage, no re-review.
- [ ] Record exact verified component state/rulings, preserve branch/worktree and selected046, continue broader goal. Expected: no portable/parity/goal-completion claim.
