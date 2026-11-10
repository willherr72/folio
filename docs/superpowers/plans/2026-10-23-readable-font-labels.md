# Readable embedded-font labels

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Existing-text dialogs show a readable font name after save/reopen without
changing the PDF font identity, encoding or edit eligibility.

**Architecture:** Add an optional display-only label to the run/group/paragraph DTOs.
Read a bounded usable full name from the verified embedded TrueType program, using
the same name selection as the font picker. Keep raw `fontName`, proof lookup,
resource names, rendering, formatting and persistence bindings unchanged. Dialogs
prefer the label and fall back to the raw name when absent.

**Spec and decisions:** This scoped plan is the design. The user authorized routine
implementation choices without approval gates. Registry lookup alone fails after
reopen/release; renaming PDF resources would touch encoding identities needlessly.
An additive optional label uses authoritative embedded bytes and retains compatibility
with existing frontend fixtures and callers. Standard PDF fonts keep their familiar
names. Names are informational, never evidence of font support or authenticity.

**Tech stack:** Rust/ttf-parser, existing font proof path, React/TypeScript dialogs.
No dependencies, source font changes or installed-font access are added.

## Tasks

- [x] Native RED: substitute DejaVu Serif, release registry ownership, save/reopen,
  require readable run/paragraph labels while raw font identity remains unchanged.
- [x] UI RED: existing run, group and paragraph dialogs prefer the optional label;
  fixtures without labels retain the current fallback and editing behavior.
- [x] Bound/sanitize usable full-name records (length, control/directional characters,
  blank names); prefer English then other decodable names. Share selection with the
  font picker without changing font admission. Test malformed/missing-name fallback.
- [x] Add optional DTO fields and populate through verified font proofs. Wire all
  existing-text dialogs, including nested-form runs, without mutating font identity.
- [x] Run relevant native/UI tests and request one scoped review. Fix findings with
  reproductions. Check label parsing does not broaden support or impose new failures.
- [x] Build fresh executable; repeat real Windows saved substitute-font re-edit with
  readable label assertions and independent no-op pixel/glyph comparisons. Inspect
  screenshots and record exact evidence/hashes/limits; preserve earlier packages.

## Review focus

Untrusted font strings; bounded decoding; missing or localized names; font identity
versus display-only labels; subset and standard fonts; shared form occurrence paths;
optional-field compatibility; no changes to edited PDF content or source files.

## Outcome

Complete source/debug checkpoint. Full frontend: 733 tests / 103 files. Broad
native gate: 89 tests across eight suites before review correction. One P3 odd
UTF-16 name issue reproduced and fixed; final helper passes two unit tests and
ten font/paragraph integration executions. No second review round. Typecheck,
Vite and debug executable build pass. Real Windows save/reopen checks readable
run/paragraph labels; MuPDF/pypdf confirm both no-op edits preserve all pixels
and glyph geometry. Four screenshots inspected; all three owned processes closed.

Manifest: `artifacts/font-label-20261023/verification.json`.
Review and response: `artifacts/font-label-review/`.
Earlier binaries/packages and private input hashes are preserved. This does not
broaden font/form admission or establish overall product parity.
