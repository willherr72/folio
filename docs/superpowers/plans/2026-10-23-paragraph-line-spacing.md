# Paragraph line spacing implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Preserve or explicitly adjust uniform paragraph baseline spacing while
retaining font, alignment, color and surrounding content.

**Architecture:** Add optional lineSpacing to selection metadata and formatting.
Infer a font-size multiple from equal baseline gaps among distinct nonempty rows;
use 1.2 when unknown. Apply the effective multiple to height checks and every line
origin, including blank lines. Existing reopened geometry/style/pixel gates remain.

**Tech stack:** Rust/PDFium/lopdf native engine, React/TypeScript desktop UI.

**Spec and choices:** User authorized autonomous implementation. A multiplier
preserves proportional spacing during font-size changes; fixed point leading
would not. Permit 0.6–3, matching existing paragraph selection's baseline limits.
Reject nonfinite/out-of-range native values. Infer only multiple rows with gaps
agreeing within clamp(size*0.02, 0.02, 0.25) pt. Single/irregular rows are unknown.
UI shows Line spacing (× font size), Reset line spacing, estimated/default help;
reset restores inferred spacing or 1.2. Invalid input keeps the draft and disables
preview. Changes invalidate candidates, canceled adjustments retain formatting,
and a newly checked selection restores its own inferred value. Existing optional
size/alignment API calls remain valid; ninth adapter argument is optional spacing.

Automatic finder retains conservative paragraph-break thresholds: widely spaced
rows may require Adjust selection. Do not merge separate paragraphs by increasing
the finder gap globally. Hard breaks/blank lines affect output; original semantic
hard breaks cannot be recovered reliably from PDF geometry. No justification or
mixed formatting in this change. Preserve previous copied builds/packages.

## Review focus

- Scaled source matrices, fragmented lines, irregular/single row inference.
- Minimum/maximum/nonfinite values, overflow, empty lines and font size changes.
- All intrinsic rotations, alignment composition, save/reopen geometry.
- Stale candidate/raster disposal and resetting only spacing.
- Source/neighbor/comment preservation; inference does not promise PDF semantics.

## Task

- [x] Add failing native tests in src-tauri/tests/paragraph_spacing.rs and UI tests
  in tests/text-paragraph-dialog.test.tsx. Assert inference, explicit and default
  output baselines, bounds/refusals, ownership and selection reset.
- [x] Implement optional fields in types.rs, app.rs, engine.rs, editor types and
  adapter; infer spacing in text_paragraph.rs and use it in height/layout. Add
  dialog state/validation/reset/invalidation while preserving default calls.
- [x] Run paragraph_spacing/alignment/text_paragraph/suggestion/comment_fidelity,
  full frontend and TypeScript/Vite/Windows build. Resolve real failures.
- [x] Obtain a scoped review; resolve findings with regressions.
- [x] Run Windows size+spacing+alignment preview/apply/undo/save/reopen, manual
  selection for wide leading, narrow layout, independent saved geometry/pixels.
- [x] Record source/executable fingerprints, evidence and remaining limitations.

Final evidence: artifacts/paragraph-spacing-verification.json. 719 frontend and
56 scoped native tests, build, Windows spacing workflow and independent saved-PDF
checks pass. Also repeated the supplied quiz insertion/undo/save/reopen/Find
workflow with unchanged surrounding pixels. Two review findings resolved: a real
boundary rounding bug, and a selection-reset test that did not change selection.
The existing 0.02 pt geometry tolerance now applies at manual spacing bounds and
the finder minimum, retaining the deliberate finder maximum.

Previous goal turn: progress; completed and verified alignment checkpoint.
