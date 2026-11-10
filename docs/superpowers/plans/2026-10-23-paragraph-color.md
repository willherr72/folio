# Paragraph color implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Let a person change a supported uniform paragraph's text color with a
real PDF preview, reset, undo and faithful save/reopen.

**Architecture:** Add an optional `color` (#RRGGBB) to ParagraphFormatting and the
native preview command, and an optional current color to TextParagraphSelection.
The existing shared color parser validates requests before editing. Emit the
chosen RGB for only the replacement and validate reopened replacement color against
that value. Keep surrounding-style and outside-area pixel checks unchanged. The
existing paragraph memory already binds actual color and needs no new metadata.

**Tech stack:** Existing Rust/PDFium/lopdf engine and React/TypeScript controls.
No dependencies. This scoped plan is the short design/specification.

**Decision:** The user authorized autonomous implementation without permission
gates. Use explicit optional color with original-color fallback. Replacing the
whole formatting API would create unnecessary caller migration; recoloring source
objects separately would bypass the current paragraph preview/fidelity checks.
Keep single-run and mixed-format editing unchanged in this cycle.

## Constraints and acceptance

- Preserve source PDFs, other text/graphics/comments, previous test executables,
  portable packages and the private quiz. Work in the existing isolated worktree.
- Omitted color retains original behavior, including old frontend fixtures. Color
  changes invalidate candidates; stale asynchronous previews must be released.
- The color picker shows the selected paragraph's current color, with an explicit
  reset to that original value. A missing optional color never becomes an implicit
  black change. Check selection resets color; cancel adjustment keeps the draft.
- Support all opaque RGB values accepted by the existing #RRGGBB parser, including
  white and lowercase hex. Reject malformed values before creating output.
- Compose with font substitution, size, spacing, alignment, blank lines and rotation.
  Do not broaden mixed-format, transparency, form or font admission.

## Tasks

- [x] Native RED: current-color inspection, explicit color across rotations and
  formatting/substitute fonts, save/reopen/no-op, unchanged neighbor/source,
  malformed colors and unchanged omitted-color behavior.
- [x] UI/IPC RED: picker initialization, explicit request/reset, preview invalidation,
  late response cleanup and selection reset/cancel behavior; omitted fields stay omitted.
- [x] Native GREEN: DTO/command optional color, strict existing parser, explicit
  replacement color emission and reopened validation. Keep source identity checks.
- [x] UI GREEN: accessible color input/value/reset, optional argument plumbing and
  updated help text; preserve draft/font lease/candidate ownership conventions.
- [x] Run focused native and frontend gates; one scoped fresh review and resolve
  findings with regression evidence. Record limits.
- [x] Build fresh isolated Windows executable; edit/recolor original and substitute
  paragraphs, undo/redo, save/reopen, verify color controls and no-op behavior.
  Independently inspect text color, glyph geometry, neighboring pixels and font
  bytes. Inspect screenshots, clean up owned processes and update quality record.

## File map and review focus

Native: `types.rs`, `app.rs`, `text_paragraph.rs`; regression
`src-tauri/tests/paragraph_color.rs`. Frontend: `editor/types.ts`, `editor/adapter.ts`,
`components/TextParagraphDialog.tsx`; tests `paragraph-color-dialog.test.tsx` and
`text-edit-adapter.test.ts`. Desktop acceptance extends the established paragraph
workflow with opt-in color assertions and independent verifier checks.

Review missing versus explicit black; invalid color decoding; retained graphics
state and untouched neighbors; combined substitution/formatting; stale previews
after change/reset and piece-selection changes. Source and output checks must
cover actual PDF bytes, not only DTOs.

## Ledger

Previous goal turn: progress, readable-font-label checkpoint completed and verified.
Current baseline: `artifacts/font-label-20261023/verification.json`.

Native RED: three expected failures for missing color DTO/formatting; GREEN: three
native tests pass, including all four rotations and no-op raster equality.
UI RED: five new assertions fail; focused GREEN: 62 tests, with a supplemental
font/color composition case added afterward.

Ruling: fix an existing copy test's asynchronous readiness race. The first full
frontend run had 738 passes and one failure in page-interactions.test.tsx: selecting
the already-mounted but still-empty text container creates a collapsed range.
PdfTextLayer mounts its label before asynchronous text extraction. The same file
passes in isolation; its adjacent copy test already waits for actual text. Add
that same readiness condition before creating the range; product copy code is
unchanged. Preserve the failed full log and rerun the full suite.

Full rerun: 739 frontend tests / 104 files pass. Native broader gate: 57 tests /
seven suites pass. One scoped review found no actionable color defect; its recorded
clipboard failure predates the readiness correction and passing full rerun.
The first build then caught a test-only TypeScript error: Testing Library's
getByRole has no Playwright-style `exact` option. Removed it (string names already
match exactly), reran the four color tests and typecheck successfully. Failed job
results are preserved with `first-` prefixes; both jobs confirmed exited before
rebuilding. Product source fingerprint unchanged.

First completed desktop flow and independent PDF check passed. Screenshot inspection
showed the shared input padding compressed the native color swatch. A scoped CSS
rule gives it 40px height/4px padding. Final desktop flow adds a minimum swatch-height
assertion and repeats all prior checks in `artifacts/paragraph-color-verified-20261023`.
The earlier binary/acceptance remain intact. No native or frontend behavior change
after the passing gates; final typecheck/build and desktop acceptance cover the CSS.

Complete checkpoint: final Windows and independent color/undo/no-op checks pass;
four final screenshots inspected; all eight owned jobs/apps exited. Original
inputs, prior executables and portable packages verified unchanged. Manifest:
`artifacts/paragraph-color-verified-20261023/verification.json`. The overall goal
remains active, including mixed formatting, tagged content, shaping and broader
font/document compatibility.
