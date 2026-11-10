# Paragraph alignment implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Support left, centered, and right-aligned uniform paragraph edits while retaining font, size, color, direction and surrounding content.

**Architecture:** Reuse TextAreaAlignment and introduce ParagraphFormatting for optional native font size/alignment. Preserve existing public wrappers. Infer alignment from raw PDF row geometry during inspection; report unknown when insufficient or ambiguous. Reflow offsets lines within the raw paragraph area, independent of page rotation. Reopened validation checks the intended baseline origins as well as existing style/ink/pixel gates. Dialog selection initializes alignment, and changes invalidate the candidate.

Inference and layout use compatible origin/ink/verified-advance extents. PDFium object rectangles include whitespace cursor positions, so visible containment is checked per nonspace glyph. Trailing spaces remain encoded but do not change wrapping/alignment width. These rules resolve the two scoped review findings; red regressions cover both before the correction.

The automatic paragraph finder also uses these extents for shared centers/right edges. Inspection estimates the horizontal edit area from the same extents so trailing whitespace cursor positions cannot enlarge it. Actual desktop acceptance exposed this integration requirement; native regressions now check suggestions from every saved line and bound the inferred area after re-editing.

**Spec:** Group selected fragments on the same baseline. At least two rows are needed for inference. Use bounded geometric tolerance, require meaningful row-width variation, and recognize a shared left edge, right edge, or center; otherwise return unknown. Single-line/unknown selections default left. Expose a select with Left/Center/Right. Retain alignment through text/size edits and canceled adjustments; a newly checked selection initializes its inferred value. Trim trailing spaces for alignment measurement while retaining encoded text. Left/right are relative to text direction, including intrinsic rotation. Explicit size and alignment must compose. Do not introduce justification, mixed formatting, font substitution, or automatic shrinking. Existing overlap, overflow, permissions and publication gates remain.

## Review focus

- Split fragments, nearly equal widths, indentation, and ambiguous inference.
- Negative bearings, trailing spaces, and unselected surrounding text.
- Intrinsic rotation/scaled matrices; intended baseline positions after save/reopen.
- Stale candidates after alignment changes and preservation/reset during adjustment.
- Backward compatibility for omitted formatting and old Rust method signatures.

## Task

- [x] Write failing native inference/layout and UI ownership regressions.
- [x] Implement shared formatting, geometric inference, native offsets/validation and UI controls.
- [x] Pass scoped native suites, full frontend tests and Windows build.
- [x] Obtain one scoped review and resolve findings.
- [x] Verify Windows centered/right edits, undo/save/reopen, native notes and independent geometry/pixels.
- [x] Record executable/source fingerprints, evidence and remaining limitations.

Completed evidence: artifacts/paragraph-align-verification.json. Final Windows
acceptance is artifacts/paragraph-align-acceptance-2; the first failed run remains
preserved. The overall product quality goal remains active.
