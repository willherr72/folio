# Rotated paragraph implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one fresh review after integration.

**Goal:** Reflow and move uniform paragraphs on PDFs with saved quarter-turn rotation.

**Architecture:** Keep displayed-coordinate UI/adapter contracts. Convert rectangles
at the native writer boundary and use raw dimensions for layout. Preserve /Rotate.

**Tech stack:** Rust/PDFium/lopdf, React, native integration tests, Windows acceptance,
MuPDF/pypdf independent validation.

**Spec:** `docs/superpowers/specs/2026-10-23-rotated-paragraph-design.md`.

## Constraints and review focus

- Existing uniform-font, horizontal raw text, permissions and fidelity rules remain.
- Displayed axes include intrinsic rotation but exclude extra workspace rotation.
- Rectangle transformations must handle reversed corners, cropped origins and180/270.
- Wrapped layout uses raw width/height; candidates retain displayed dimensions.
- No original-file mutations, candidate leaks or changes to portable0.15.0-dev.1.

## Task1: Native coordinate support

Files: `src-tauri/src/text_paragraph.rs`, `src-tauri/tests/text_paragraph.rs`.
Internal helpers: displayed_bounds(PageGeometry,[f32;4])->AnnotationRect and
pdf_bounds(PageGeometry,&AnnotationRect)->[f32;4]. Public signatures unchanged.

- [x] Add all-rotation/crop wrap, move, re-edit and refusal regression; observe RED.
- [x] Transform inspected/raw bounds, remove rotation-only refusal, wrap in raw space
  and normalize rotated raster bounds. Observe GREEN including original cases.
- [x] Extend rotated protected/overlay coverage and emit independent fixtures.

## Task2: Independent and actual-app acceptance

Files: new `scripts/verify-rotated-paragraph.py`,
`scripts/smoke-rotated-paragraph-desktop.mjs`, optional desktop reader verifier;
dialog copy and existing text documentation if needed.

- [x] Verify8 native rotation/crop outputs with both readers, direction and pixel masks.
- [x] Run full frontend, relevant native suites and production/Windows builds.
- [x] Actual Windows saved/reopened rotated paragraph edit, movement, preview,
  undo/redo, saved comment/neighbor fidelity and immutable input.
- [x] Fresh review, regression fixes/checks as needed; update docs/ledger and cleanup
  only owned test processes. Keep the broad quality goal active.
