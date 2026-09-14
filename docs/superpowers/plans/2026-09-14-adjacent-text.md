# Adjacent text implementation plan

> Execute using superpowers:subagent-driven-development with independent native/UI tasks and whole-branch review.

**Goal:** Explicitly edit a losslessly verified group as one single-line replacement.
**Architecture:** Inspect/replace commands share native merge proof; the replacement reuses the existing immutable single-run pipeline through an always-cleaned temporary source. Separate modal preserves the existing single-run interface.
**Tech stack:** Rust/PDFium/lopdf; Tauri; React/TypeScript.
**Spec:** ../specs/2026-09-14-adjacent-text-design.md

## Native group deliverable

Files: new src-tauri/src/text_group.rs, modifications to text_edit.rs, types.rs, lib.rs, engine.rs and app.rs; tests/text_group.rs.
- [x] Write failing tests calling engine.inspect_text_group and engine.replace_text_group for split Hello and Old + words; assert exact logical text and original bytes.
- [x] Implement TextGroupPreview and typed worker/Tauri commands with the spec signatures. Export the type.
- [x] Implement one shared prepare_group routine that checks identity/state, character attribution and exact merged glyph/raster equality; inspect publishes no source.
- [x] Replace by guarded temporary source plus existing replace_text; remove temporary ID on every Result path.
- [x] Run targeted native tests with CARGO_TARGET_DIR set to the existing primary debug target and -j1. Add refusal, rotation, embedding and lifecycle cases from the spec before broadening admission.

## UI deliverable

Files: new src/components/TextGroupDialog.tsx and tests/text-group-dialog.test.tsx; modify ExistingTextDialog.tsx, App.tsx, editor/types.ts, editor/adapter.ts, existing-text.css and focused app/adapter tests.
Interfaces: TextGroupPreview {objectIndices:number[];text:string;fontName:string;fontSize:number}; adapter.inspectTextGroup?(sourceId,pageIndex,objectIndices):Promise<TextGroupPreview>; adapter.replaceTextGroup?(sourceId,pageIndex,objectIndices,expectedText,replacement):Promise<DocumentInfo>.
- [x] Write failing tests for explicit selection then verification, no apply before verification, cancellation/stale response and retained draft after apply errors.
- [x] Implement the two-step accessible modal with bounded selection and per-request invalidation. No automatic group membership.
- [x] Add Edit together… transition, modal blocking and group apply using commitTextReplacement, captured tab/page/source identity and orphan cleanup.
- [x] Run focused frontend tests and TypeScript/Vite build, preserving existing single-run tests.

## Integration and release

- [x] Compare native exported group fixtures with independently authored reference PDFs using MuPDF/pypdf; record exact strings and raster results.
- [x] Review combined implementation, fix blockers and run appropriate full regression suites once final code settles.
- [x] Create a group-edit practice PDF; package the next release and test the actual Windows app's edit/undo/search/save/reopen/recovery workflow.
- [x] Update current behavior docs and prepare the #15 status update; package only verified artifacts and retain B3 open.

Resource rule: one cargo process at a time, -j1 and existing build caches. Other agents must not run cargo without coordination. Read/write synthetic project fixtures only; validate owned desktop PID/path before termination. No unbounded polling or repeated broad test runs without a new concern.
