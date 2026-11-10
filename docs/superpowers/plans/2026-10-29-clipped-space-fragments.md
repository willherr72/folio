# Clipped paragraph space fragments implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, task by task.

**Goal:** Permit proved nonpainting space fragments in clipped paragraph editing.
**Architecture:** Keep ordinary native clipping checks; add exact decoded-source
and native nonpainting evidence for the blank-fragment case only.
**Tech stack:** Rust, lopdf, existing PDFium bindings.
**Spec:** `docs/superpowers/specs/2026-10-29-clipped-space-fragments.md`.

## Global constraints

Preserve original PDFs and the inherited worktree. No new renderer, dependencies,
public API, packages or budgets. One Cargo gate, locked/offline/jobs1. No commits.
No product assertion without a completed check. Partial highlight ownership stays
unchanged. Source decoded from saved operands, never supplied request metadata.

## Review focus

Missing glyph extraction with visible ink must not be treated as a blank. Empty
operands, tabs and custom painted spaces need conservative treatment. Saved
re-edit must apply the same ownership proof. Finite origin must remain in the clip.
Tags and outside pixels must survive preview/publication.

## Task 1: Reproduce and correct space-fragment clipping

- [x] Add regression in `src-tauri/tests/word_text_context.rs`, covering a clipped
  paragraph with a separately styled embedded-font space; capture the expected
  clipping failure before changing product source.
- [x] Trace native space bounds/glyphs in that failure; refine the minimal private
  proof in `src-tauri/src/text_context_edit.rs` from observed evidence.
- [x] Add negative native proof tests in `src-tauri/src/text_context_edit_tests.rs`
  for unproven blank/visible glyphs, malformed geometry and outside-clip origin.
- [x] Verify preview, save/reopen/re-edit and context/structure preservation in the
  integration regression; run affected context/highlight suites serially.
- [x] Build a fresh local CLI candidate and rerun the unchanged Word geometry
  controls. Keep unsupported partial highlight and resource failure explicit.
- [x] Review the scoped delta, reconcile evidence and checkpoint next work.

Evidence: `artifacts/word-highlighted-reflow-20261029/checkpoint.json`.
Product scope additionally includes `src-tauri/src/source_text.rs`: exact embedded
space operands require the same nominal-space admission already used for standard
fonts. Diagnosis and rationale are recorded in the execution ledger. Review is
scoped to this correction; no desktop, portable, resource or parity approval.
