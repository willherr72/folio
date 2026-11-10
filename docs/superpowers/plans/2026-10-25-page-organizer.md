# Multi-page Organizer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans, inline. One final scoped correctness reviewer.

**Goal:** Select and arrange multiple pages in a thumbnail grid, extract the selected draft, and Apply as one document transaction.

**Architecture:** Pure organizer frames retain a draft EditorDocument plus selection. Existing generic history provides local Undo/Redo; Modal and lazy Thumbnail provide the workspace. App owns original sources and validates the frozen document before committing.

**Tech Stack:** React/TypeScript, Vitest, existing native editable export, packaged Windows workflow.

**Spec:** docs/superpowers/specs/2026-10-25-page-organizer-design.md

## Global constraints

- Preserve originals, parent history/dirty state until Apply, existing source registrations and protection policy.
- Maximum 10,000 pages, 100 local undo entries, at least one retained page.
- One history step per local action; selection/no-op moves create none; one parent Undo per Apply.
- No commits, release or parity claim. Keep prior runs and binaries.

## Review focus

- Modifier/keyboard selection must not accidentally mutate pages or parent shortcuts (task 2).
- Noncontiguous moves/drop-inside-selection and duplicate limits must preserve exact order (task 1).
- Deletion must retain valid bookmark ancestors and select a surviving page (task 1/3).
- Pending extraction, modal close/unmount and stale Apply cannot publish another tab's draft (task 2/3).
- Narrow grids, focus after deletion/history and lazy thumbnail lifetimes must remain usable (task 2/3).

### Task 1: Pure selection and batch operations

Create src/editor/page-organizer.ts, tests/page-organizer.test.ts.
Interfaces: OrganizerFrame {document,selection:{ids,anchorId,focusId}}; createOrganizer(document), selectOrganizer(frame,id,{toggle,range,focusOnly}), organizerKeyTarget(frame,key,columns), organizePages(frame,action) where action rotate(direction), duplicate(ids), delete, move(boundary). commitOrganizer(history,action) bounds local history. applyOrganizerDocument(session,expected,draft) validates page/bookmark references, protection and draft pages before parent commit.

- [x] Write failing selection/order/history/deletion/cap/protection tests; observe missing module failure.
- [x] Implement pure functions with immutable pages, cloneOverlay for duplicates, filterBookmarks for delete, stable move filtering and no-op identity.
- [x] Verify focused tests, including returned selection and planDigest invariants.

### Task 2: Grid and App integration

Create src/components/PageOrganizerDialog.tsx, page-organizer.css, tests/page-organizer-dialog.test.tsx, tests/page-organizer-app.test.tsx. Modify App.tsx only for opening/state/modal/apply/extract wiring.
Props: document,adapter,onApply(draft):void,onClose():void,onExtract(draft,selectedPages):Promise<string|null>. Dialog snapshots initial document; export is pending-ref guarded. App uses frozen tab/document/adapter; applyOrganizerDocument commits exactly once and selects a valid page.

- [x] Write failing component tests for Ctrl/Shift/arrows/Space/Select all, local Undo/Redo, no-op Apply, grouped drag, external drag rejection and busy/error retry.
- [x] Implement responsive multi-select listbox with roving focus, current-column arrow movement, pointer drag insertion, stable controls, bounded history and visible status. Local Ctrl+Z/Y and Delete stop propagation; typing retains normal editing shortcuts.
- [x] Add Organize pages actions and frozen parent transaction, preserve global modal/mutation gating. Add App tests proving Cancel, local changes, one parent Undo and extraction snapshot/bookmark/error behavior.
- [x] Verify focused, full bounded frontend and TypeScript. Native code unchanged.

### Task 3: Acceptance

- [x] One scoped review; correct demonstrated findings once with red/green coverage.
- [x] Build fresh fingerprinted executable; run relevant native page/form/bookmark export regression.
- [x] Packaged desktop: discontiguous group rotate/move/duplicate/delete, local Undo/Redo, drag, keyboard, narrow controls, extract draft, Cancel/Apply/parent Undo/Redo, save/reopen.
- [x] Independent MuPDF/pypdf page order/pixels/rotation/crop/forms/notes/bookmarks/links and original hash checks; stop owned processes.
- [x] Record source/binary hashes, failed runs, coverage limits; update quality checklist/latest checkpoint. Keep broad goal active.

Evidence and precise limits: artifacts/page-organizer-20261025/checkpoint-corrected/verification.json and execution.md. Full frontend precedes the final one-line entry guard; focused/TypeScript/build/desktop are post-correction.
