# Desktop workspace Replace implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. Use test-driven-development for each behavior and one final scoped review after integration.

**Goal:** Replace literal text in the current workspace with validated preview and one reversible Apply.

**Architecture:** Freeze the editor model, stage complete-source edits per occurrence, and preserve model metadata and local transforms during adoption. A window-owned native session retains candidates until Apply or cleanup.

**Tech stack:** TypeScript/React/Vitest; existing Rust engine, Tauri commands and native test harness.

**Spec:** `docs/superpowers/specs/2026-11-14-desktop-replacement-design.md`

## Global constraints

Use the managed dirty isolated worktree inline. Preserve originals/artifacts and prior packages/launcher. No commit/cleanup/version changes, clipboard probe or new private input. Frontend two workers/default timeout; native serialized with one Cargo job and the existing D: target/temp. Evidence before completion claims.

## Review focus

- A candidate belonging to another tab or older Undo state must never be adopted/closed as new.
- Multiple duplicate occurrences must preserve independent native content and page-qualified overlay identity.
- Metadata/bookmarks/local crops/rotations survive preview, Apply and Undo exactly.
- Late invalid mapping or stale overlay rejects all changes, including otherwise valid earlier sources.
- Input changes, tab/history changes and destroyed windows cannot apply obsolete candidates.

### Task 1: Atomic workspace transaction

**Files:** Create `src/editor/workspace-replacement.ts`, `tests/workspace-replacement.test.ts`.

**Interfaces:** Capture immutable `WorkspaceReplacementRequest`; validate `WorkspaceReplacementResult` containing final per-source `PageContentReplacement` mappings and page-qualified text-overlay patches; commit once to `DocumentSession`.

- [x] Write focused tests: frozen detached capture, multiple imported/duplicate sources, overlay styling and deletion, metadata/bookmarks/crop/rotation preservation, one Undo/Redo, stale tab/adapter/history/mutation, global borrowed IDs, malformed mapping/protection and atomic late refusal.
- [x] Run focused Vitest and record expected runtime RED with a deliberately unimplemented module.
- [x] Implement capture/validation/commit without native calls or source cleanup.
- [x] Run focused tests and relevant existing model tests; typecheck. Preserve evidence and mark task complete without committing.

### Task 2: Native workspace planning and staging

**Files:** New workspace replacement engine/host/commands modules; extend `engine.rs`, `lib.rs`, `app.rs` and adapter capabilities only as required.

**Interfaces:** Consume Task 1 frozen page plans and literal intent; produce source mappings/overlay patches bound to an opaque native session and preview identity.

- [x] Write failing native tests for duplicate/imported occurrences, crop filtering, selected blockers, bounds, stale sources and late-stage rollback.
- [x] Implement reuse of source-bound planning/occurrence editing, preserving original graph and source policy. Validate overlay matching/layout independently.
- [x] Add registered-window candidate ownership, cancel/stale/reply-abandonment cleanup tests; run focused and affected native gates.

### Task 3: Desktop Replace workflow

**Files:** New replacement workflow/dialog/tests, SearchBar/App integration and targeted styles.

**Interfaces:** Consume native session capabilities and Task 1 transaction. Apply only the exact displayed candidate to the same tab/history; retain original/candidate resources for Undo.

- [x] Test entry, literal options, blockers/explicit selection, preview-before-Apply, cancellation/races, source guards, narrow focus and one Undo.
- [x] Implement controls and lifecycle with existing modal patterns, then run focused and complete frontend tests and normal build.

### Task 4: Integrated acceptance

- [x] Run authored mixed-workspace desktop workflows, save/reopen and independent text/graph/raster checks with negative controls.
- [x] Request one fresh scoped review; disposition findings, perform at most one correction pass and verify the corrected scope.
- [x] Seal source/evidence checkpoint and truthful limits; keep broad goal active and leave package/launcher unchanged.
