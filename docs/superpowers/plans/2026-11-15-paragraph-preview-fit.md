# Automatic paragraph preview fitting implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline; one fresh final whole-component review, no implementer agents.

**Goal:** preview directly edited paragraphs with a checked fitting proposal and one coherent local edit history.
**Architecture:** reuse native fitting through the existing adapter and extend the shared draft-fit helper; track content-fit intent separately from selection, history and explicit area changes. Recompute the preview request only after adopting a current, owned, validated proposal.
**Tech Stack:** TypeScript/React, existing native Rust fitter and Tauri Windows harness; Python independent PDF audits.
**Spec:** docs/superpowers/specs/2026-11-15-paragraph-preview-fit-design.md

## Global constraints
Preserve existing dirty worktree, documents, source eligibility, fonts and baseline. No Git mutations/dependencies/private inputs/clipboard/version/portable/launcher changes. Serialize native/frontend/build/UI; one Cargo job, existing D-drive cache/TEMP. Frontend min/max2 workers/default5000ms. Exact native/shared equality qualifies prior scopes, not fresh native tests. One final fresh review and one Important correction pass; Minor findings deferred.

## Review Focus
- Keyboard Undo/Redo and manual area moves must not silently refit restored geometry.
- Delayed fit replies must not overwrite a newer draft, owner or caret selection.
- Failed/outside-crop proposals must preserve the draft and allow the original native preview route.
- Request fonts stay alive until fit/preview settlement, including unmount/reset and provider mutation.
- Old rendered candidates cannot authorize Apply after toggling on a needed fit or adopting new geometry.

### Task1: Behavioral RED
Files: create tests/paragraph-preview-fit.test.tsx; baseline the two production files and existing dialog tests.
- [x] Assert default fit-before-preview call ordering and matching fitted request; untouched initial/caret paths perform no fit.
- [x] Assert one Undo/Redo restores text plus fitted area; fixed-area opt-out and manual/history operations retain exact geometry.
- [x] Assert failures/outside-crop/delayed draft/owner/unmount preserve user data and discard stale proposals; current caret survives.
- [x] Run focused Vitest with min/max2workers/default5000ms and preserve terminal behavioral failures before production edits.

### Task2: Implement and verify
Files: src/editor/paragraph-draft-fit.ts, src/components/StyledParagraphDialog.tsx; tests/paragraph-preview-fit.test.tsx, tests/styled-paragraph-dialog.test.tsx.
Interfaces: applyAutomaticParagraphFit(editor:StyledEditorState,bounds:AnnotationRect):StyledEditorState preserves history/selection/baseline; fitEditedParagraphDraft(adapter,sourceId,pageIndex,editor,context):Promise<FittedParagraphDraft> shares bounded context and copy/ownership rules with fitTransferredParagraphDraft.
- [x] Share automatic-area history application and add edited-draft feedback without changing transfer behavior.
- [x] Track fit-needed content intent, default-on capability checkbox, and explicit history/manual exceptions; sequence fitting before native preview and adopt only current owned bounds.
- [x] Extend focused coverage for keyboard history, canvas readiness, source changes, leases and provider mutation; observe GREEN and full frontend/typecheck.
- [x] Verify unchanged native/shared hashes and preserve exact reused profile meanings.

### Task3: Compiled acceptance and review
Files: component-local build/runtime/audit/evidence scripts, docs/existing-text-editing.md, component ledger/status.
- [x] Build normal optimized diagnostic GUI and verify26resources/source/frontend bindings; keep old packages/launcher.
- [x] Run actual Windows direct/shared/CFF/opt-out/refusal/history/save/reopen checks and independent saved-PDF audits with meaningful corruptions.
- [x] One fresh final scoped review, one Important RED/GREEN correction pass if required; reconcile exact evidence, document limits, retain artifacts and leave broad goal active.

Correction scope: adjacent StyledParagraphInput.tsx and ParagraphTextEditor.tsx now notify provisional browser input; all four frontend production files retain exact backend/shared equality. One fresh review and one observed RED/GREEN correction pass completed; no rereview. Runtime harness corrections retain prior failed attempts and never alter native response bytes.
