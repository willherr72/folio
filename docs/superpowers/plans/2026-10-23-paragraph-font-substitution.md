# Paragraph font substitution implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Let a supported uniform paragraph use an explicitly chosen font when
the original PDF font lacks replacement characters.

**Architecture:** Extend ParagraphFormatting with optional font_id and the native
adapter with an optional tenth argument. Preserve existing admission and original
font paths. Embed a verified full TrueType resource using the existing persistence
helper, measure unshaped glyph advance/ink from that font, and prove the reopened
candidate's program hash, text, geometry, color and surrounding content. Reuse
FontPicker with independent dialog and in-flight font leases.

**Tech stack:** Rust/PDFium/lopdf, React/TypeScript, existing Windows acceptance.

**Spec:** This document plus artifacts/portable-021-next-editing-audit.json.
Builder decision under the user's autonomous-work authorization: explicit font
choice, not automatic substitution. Reuse the existing full-font format rather
than creating a second encoding. Keep current printable Latin paragraph scope;
broader scripts, shaping and mixed/tagged text need separate work.

## Constraints and review focus

- Default and Reset retain the original PDF font; no silent substitution.
- Choosing/canceling fonts preserves text, size, alignment, spacing and area.
- Canceling the picker retains an existing candidate. Committing a different
  font invalidates it. Different paragraph/source selection clears the choice.
- A separate operation lease retains fonts until native previews settle, even
  if the dialog/source changes. Embedded candidate PDFs own their font bytes.
- Keep existing source admission, collision, comment and pixel-preservation checks.
- No release version bump or mutation of the verified 0.21 package. No commit,
  publication, dependency addition or parity claim.

## Tasks

- [x] Add src-tauri/tests/paragraph_font.rs: JSON fontId fails before support;
  original subset refuses an absent letter, explicit font succeeds; embedded
  program identity/Unicode extraction, size/alignment/spacing, all rotations,
  save/reopen/re-edit, unknown ID/unsupported glyph/overflow refusal and source
  preservation. Run focused tests and retain RED evidence.
- [x] Add optional font_id in types.rs and app.rs; support it in text_paragraph.rs
  with existing font registry/resource embedding and bounded glyph metrics.
  Run paragraph_font, text_paragraph, paragraph_alignment, paragraph_spacing,
  paragraph_suggestion and comment_fidelity sequentially under one Cargo job.
- [x] Add adapter and dialog regressions for omission/explicit ID, canceled font
  picker, invalidation, source/selection reset, unmount during preview and stale
  candidate disposal. Implement picker state and leases in TextParagraphDialog.
  Run focused RED/GREEN then full frontend and TypeScript/Vite checks.
- [x] Request one scoped fresh review; reproduce/fix meaningful findings once.
- [x] Copy a new debug executable with source/hash evidence. Run real Windows
  font selection, draft retention, missing-subset replacement, narrow controls,
  rotation, undo/redo, save/reopen and independent PDF/pixel checks.
- [x] Record evidence, cleanup, preserved package/input hashes and open gaps.

Review attention: stale font leases, inherited font resources and alias collisions,
trailing spaces/negative glyph bearings, substituted program proof, re-edit after
releasing the imported font, and preserving relative-positioned neighboring text.
