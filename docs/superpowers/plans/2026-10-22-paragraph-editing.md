# Uniform paragraph editing implementation

Spec: ../specs/2026-10-22-paragraph-editing-design.md

Execute inline in the existing isolated worktree. Preserve pre-existing edits.
No commit, push, or restart of the user's running editor is needed.

## Task 1: Native paragraph selection and preview

Add generated PDF regression tests first: split words across operators, multiple
lines, insertion with wrapping, unchanged neighbors and source bytes, overflow,
mixed formatting, stale text, and unsafe selections. Observe failure.

Implement inspect_text_paragraph and preview_text_paragraph through the engine
worker. Use verified font metrics, bounded word wrapping, original cursor
advances for removed text, reopened-object checks and outside-area raster checks.

Interface: TextParagraphSelection contains objectIndices, text, fontName,
fontSize, bounds. Preview takes selection indices, expected text, replacement,
and bounds and returns an immutable DocumentInfo. Bounds use displayed points.

Verification: cargo test -j 1 --test text_paragraph. Expected: all tests pass.

## Task 2: Dialog and application integration

Write tests for selection checking, native preview, invalidation, overflow draft
retention, cancellation and late response cleanup, and Apply ownership transfer.
Expose Tauri commands and optional adapter methods. Add Edit paragraph entry to
the existing text dialog. Apply through commitTextReplacement with version checks.

Verification: targeted Vitest tests, full frontend suite and production build.
Expected: all pass, one undo step and no leaked temporary sources.

## Task 3: Acceptance, review and documentation

Exercise uniform quiz text with insertion, export/reopen and independent reader
checks. Run native related regressions. Document scope and limitations. Request
one fresh review focused on source preservation, stream state, stale responses,
and ownership. Fix material findings and verify their regressions.

Review Focus: malformed content, resource aliases, generated spaces, implicit
state, unrelated text/graphics changes, bounded input, cancellation and preview
ownership. Running app may contain unsaved user edits; do not terminate it.
