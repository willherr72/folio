# Paragraph justification implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Add justified uniform paragraphs with adjustable native PDF text, useful
save/reopen/re-edit behavior, and the existing surrounding-content fidelity checks.

**Architecture:** Give paragraphs their own alignment enum (Left, Center, Right,
Justify), leaving the existing text-area enum unchanged. Record soft wrap boundaries
in paragraph layout. Expand only interior ASCII word-space gaps in soft-wrapped
lines, using positioned TJ text with the original or substitute font encoding.
Hard-break endings, the final line, blank lines, and one-word lines remain left
aligned. Preserve encoded whitespace and searchable text. Infer justification only
from multiple compatible rows with measured expanded interword gaps; ambiguity
still defaults to Left. No stored paragraph semantics are invented.

**Tech stack:** existing Rust/PDFium/lopdf native editing and React/TypeScript UI.
**Spec:** This document. Decisions are made under the user's autonomous-work request.

## Constraints

- Preserve all existing original-font and substitute-font admission and resource
  checks, source bytes, comments, neighbors, collision and pixel proof.
- Expanded gaps must not create multiple source objects per line or lose Unicode.
- Gap calculation uses glyph advance and ink (including negative bearings).
- Last lines before explicit newlines are not stretched. Source semantic hard
  breaks remain unrecoverable on re-inspection, as documented for existing reflow.
- A paragraph-specific enum avoids changing added text-area behavior. Rust callers
  of ParagraphFormatting use ParagraphAlignment; JSON Left/Center/Right unchanged.
- No version bump, publication, commit, or modification of accepted executables.

## Tasks and review focus

- [x] Native RED tests: justify JSON admission; full width after soft wrapping,
  natural final/hard-break lines, repeated/leading/trailing spaces and blank lines,
  one-word lines, original standard/embedded/substitute fonts, all rotations,
  size/spacing, save/re-edit, preserved notes/neighbors and overflow rejection.
- [x] Implement paragraph enum and layout/encoding/inference. Keep helpers focused
  in a new paragraph_layout.rs module. Update native paragraph tests' enum imports.
  Run justification and all six existing paragraph/comment test binaries.
- [x] UI/adapter RED tests: choose Justify, preserve after canceled selection,
  invalidate candidates, forward with font/size/spacing, infer saved choice.
  Implement option and explanatory copy; run focused then full frontend/build.
- [x] One scoped code review, handle actionable findings with reproductions.
- [x] Copy debug build; real Windows original/substitute selection, wrapping,
  hard breaks, narrow layout, save/reopen/re-edit, rotation and undo/redo. Verify
  extraction/geometry/font bytes/comments and untouched pixels independently.
- [x] Record fingerprint, source/package preservation, process cleanup and gaps.

Review focus: expanding multiple spaces, trailing cursor positions, glyph bearings,
Unicode code boundaries, inferred justification versus ordinary aligned text, and
selection/re-edit after PDFium normalizes positioned text.
