# Paragraph positioning implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, one fresh whole-milestone review after integration.

**Goal:** Move an existing uniform paragraph precisely through numeric position or an original-page drag, with native preview/fidelity gates.

**Architecture:** Existing native rectangle-based replacement plus UI draft position and a local SVG drag component. Candidate ownership remains in TextParagraphDialog.

**Tech stack:** React/TypeScript, Rust/PDFium, Vitest, native fixtures, desktop browser automation and MuPDF/pypdf.

**Spec:** `docs/superpowers/specs/2026-10-23-paragraph-position-design.md`.

## Constraints and review focus

- Native uniform formatting/source-rotation rules remain in force; original axes in preview.
- Complete rectangle stays inside page; typed invalid drafts remain editable.
- Candidate invalidates on committed position changes, never on cancelled/no-op gestures.
- Pointer cancel, lost capture, Escape and unmount cannot apply a move or leak a candidate.
- Editing text/size, comparison/zoom and Apply cannot race an active drag.

## Task1: Numeric position and native fidelity proof

Files: `src/editor/paragraph-preview.ts`, `src/components/TextParagraphDialog.tsx`,
tests `text-paragraph-dialog.test.tsx`, `src-tauri/tests/text_paragraph.rs`.

Interface: `validParagraphBounds(rect,pageWidth,pageHeight):boolean` and
`moveParagraphBounds(rect,x,y,pageWidth,pageHeight):AnnotationRect|null` in the
paragraph helper; Left/Top draft strings are included in the existing adapter call.

- [x] Observe RED for numeric position/candidate invalidation/invalid drafts/reset.
- [x] Implement position fields, validity guidance and source-axis explanation.
- [x] Add native moved/cropped/protected/refusal fixtures, run targeted native suite and
  emit independent-reader artifacts only to an explicitly selected artifact directory.

## Task2: Original-page drag

Files: new `src/components/ParagraphAreaMove.tsx`, modify `ParagraphPagePreview.tsx`,
`paragraph-page-preview.css`, `TextParagraphDialog.tsx`; tests new `paragraph-area-move.test.tsx`.

Interface: move component consumes bounds/page size and `onChange(bounds)`,
`onDraggingChange(boolean)`; preview forwards these optional callbacks, enables
movement only in Original view, and disables comparison/zoom while dragging.

- [x] Write RED movement/clamp/cancel/no-op/gesture-busy tests.
- [x] Implement local draft/capture, valid coordinate inversion and cancellation cleanup.
- [x] Integrate with dialog ownership and disabled controls; run paragraph suites.

## Task3: Acceptance, review and evidence

- [x] Extend isolated fixture/browser workflow for pointer movement, zoom, cancel and numeric controls.
- [x] Run complete frontend/build plus relevant native tests; build isolated Windows test app.
- [x] Verify actual move→preview→apply→undo/redo→rotated save/reopen; independently compare output placement, text, neighbor pixels and original hashes.
- [x] Fresh review, regression fixes if needed, final checks and quality ledger. Keep broader goal active.
