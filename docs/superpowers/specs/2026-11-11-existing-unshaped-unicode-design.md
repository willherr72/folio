# Existing PDF text: the supported unshaped repertoire

Extend original-font editing to the same bounded BMP repertoire already supported
by imported fonts, including Greek, Cyrillic and covered symbols. The present
Latin-only guard blocks these documents even when exact character/glyph/width
proof exists. This advances the user's standing goal of practical commercial
editor usefulness, particularly editing existing documents.

The user explicitly opted into autonomous execution. Use the existing isolated
worktree and Rust/PDFium implementation. Do not ask routine design/approval
questions, commit, publish, alter accepted artifacts or start the user's editor.

## Approach

Reuse `fonts::allowed_character` as the common unshaped classifier. Retain the
old Latin classifier for nominal standard-font encoding, which has a separate
repertoire proof. Embedded TrueType CMap collection and eligible editing routes
use the common classifier. Every requested character must still have a verified
source code, actual TrueType glyph and matching width. Explicit substitutes must
pass their existing font coverage/embedding checks. No implicit substitution.

A separate shaping implementation would be appropriate for Arabic/Indic, bidi,
combining clusters or ligatures; it does not solve this avoidable admission gap.
A second list of Unicode ranges would drift from existing added-text behavior.
The chosen approach is the implementer's call under standing autonomy.

## Requirements

- Single, grouped, uniform/styled paragraph and reusable Form text routes use the
  same supported unshaped repertoire; exact source ownership/context stays intact.
- Greek, Cyrillic, mixed Latin/Greek/Cyrillic and covered symbols can be inspected,
  changed, saved, reopened and edited again with their verified embedded font.
- Keep existing 4096 UTF-8 byte limits, finite geometry, collision, permission,
  Unicode/glyph/width, source operand, resource and publication checks.
- Standard-font encoding admission stays unchanged. Unsupported combining marks,
  bidi controls, shaping scripts, emoji, ambiguous CMaps and missing glyphs refuse
  atomically. No script may gain a font from its name alone.
- Preserve font programs/maps, baseline/style, surrounding glyphs and pixels;
  no-op edits must preserve pixels. Cover rotations 0/90/180/270 and UserUnit1/1.7.
- Diagnostics accurately describe the expanded unshaped scope. Frontend, CLI/MCP
  and saved paragraph memory must not retain a hidden Latin-only gate.
- Actual native tests establish changes; fresh full source-bound native regression,
  normal builds, independent saved-PDF audits, targeted automation/desktop checks
  and one final scoped review are required before component acceptance.

The accepted0.43 binaries/ZIP/checkpoint/launcher remain unchanged. This component
does not claim complex-script shaping, CFF/variable font support, broad producer
parity or a new portable release. These remain part of the broader active goal.
