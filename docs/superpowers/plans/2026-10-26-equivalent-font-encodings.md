# Equivalent font encoding implementation plan

> Execute inline with superpowers:executing-plans. User authorized autonomous work;
> preserve dirty files, originals and all logs. No commits or releases.

**Goal:** Admit verified equivalent font encoding dictionaries for existing edits.
**Architecture:** A bounded read-only helper shared by text preflight and embedded
WinAnsi proof, with all downstream fidelity gates retained.
**Tech stack:** Rust, lopdf, PDFium; existing Windows/independent-reader harnesses.
**Spec:** docs/superpowers/specs/2026-10-26-equivalent-font-encodings-design.md

## Global constraints

Explicit WinAnsi/MacRoman base for dictionaries, exact ASCII glyph names only,
512 array items maximum, no duplicate assignments or malformed/implicit base.
One Cargo job at a time. Never overwrite input PDFs or earlier build artifacts.

## Review focus

- Glyph aliases with the same Unicode value must not count as identical glyphs.
- Indirect dictionary members and cyclic references must terminate safely.
- New characters absent from the original text still need the full font proof.
- Native save/reopen must not silently rewrite neighboring content or fonts.
- Paragraph/substitution paths sharing preflight must retain their fidelity guards.

### Task 1: Reproduce and implement exact equivalence

**Files:** create src-tauri/src/equivalent_encoding.rs; modify text_edit.rs and
embedded_font.rs; tests in equivalent_encoding.rs and tests/text_edit.rs.
**Interface:** `equivalent_encoding::name<'a>(&'a Document, &'a Object) -> Option<&'a [u8]>`.

- [x] Add a public engine regression showing `[65 /A]` refuses a safe same-font edit;
  retain the old custom-remapping negative with `[65 /Z]`.
- [x] Run the focused regression and retain its expected unsupported-encoding failure.
- [x] Add the bounded helper and positive/negative unit tests, then integrate both
  guards without mutation or other admission changes.
- [x] Run focused native tests; expected all pass, originals unchanged.

### Task 2: Verify editing and saved-output fidelity

**Files:** tests/equivalent_encoding.rs; artifacts/equivalent-encoding-20261026/.
**Consumes:** Task1 recognition. **Produces:** saved original/output pairs and audits.

- [x] Cover direct/indirect, WinAnsi/MacRoman, all rotations, source state and neighbor
  preservation, paragraph/substitution and embedded ToUnicode/no-ToUnicode paths.
- [x] Add malformed/remapped/alias negatives and unchanged source assertions.
- [x] Run library, text, embedded, positioned, paragraph and form compatibility suites.
- [x] Independently verify saved text and unaffected pixels using MuPDF and pypdf.

### Task 3: Review, desktop acceptance and checkpoint

**Files:** scoped frozen review artifacts, fresh build/harness, editing docs/quality log.
**Consumes:** Task2 evidence. **Produces:** runnable checked build and continuation.

- [x] Dispatch one scoped fresh code review including baseline/current source hashes.
- [x] Resolve demonstrated findings inline with regression evidence.
- [x] Build fresh desktop executable; run one standard and one embedded edit workflow,
  paragraph preview/apply, Undo/Redo and saved reopen checks.
- [x] Record actual passes/limits, executable/source identities and next priorities.

Completion evidence: artifacts/equivalent-encoding-20261026/checkpoint.json; all three tasks recorded in the plan ledger.
