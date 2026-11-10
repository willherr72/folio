# Empty choice appearance transitions

Execute inline under the choice-editor design and TDD. The previous native probe
checkpoint is d1694d49...64cae. Preserve source PDFs and failed evidence; no commits
or overlapping Cargo jobs. This closes a publication prerequisite, not the feature.

## Decisions

PDFium produces genuinely empty dropdown/list appearances without a Tx block.
Add an explicit ChoiceTextBlock::parse_empty_background factory for bounded,
balanced vector-only content: no text, marked content, external state or resource
invocations. A caller must separately prove the current value is empty. Ordinary
parse remains strict and never interprets malformed text as an empty background.

An originally empty background has a zero-length insertion slot at its end.
Inserting generated Tx retains all original background operations. Clearing an
existing Tx must retain `/Tx BMC q Q EMC` at that exact position, rather than remove
the marker: otherwise a later edit can paint above artwork that originally followed
the text. Empty-to-empty background is an exact operator no-op. All existing
operation/state/font/output budgets remain enforced for inserted or cleared slots.

## Task1 — transitions and native proof

Files: form_choice_text.rs, form_choice_text_tests.rs, engine_choice_probe_tests.rs.
API: private parse_empty_background(bytes); transplant handles zero-length
generated regions without losing the source insertion slot.

- [x] Preserve baseline and observe failing tests for vector background admission,
  invalid/hidden text refusal, clear/restore ordering and empty no-op.
- [x] Implement constrained background parsing and stable empty Tx slots.
- [x] Native clear/restore cycles for selected dropdown/list/artwork cases, plus
  an initially native-empty appearance. Verify current values and exact restored
  pixels; independently check artwork and restored pixels in saved test copies.
- [x] Run affected library/choice/fill/reflow regressions, verify source/fixture
  hashes and record remaining glyph/publication/UI requirements.

Review focus: q/Q does not reset a current path; Do/gs/sh can hide state/text and
are not accepted as proven empty. Empty input must not bypass malformed-mark
rejection. Preserve the exact text layer position after clear, including artwork
that overlaps text. Empty factories do not establish source value/font/geometry
or global NeedAppearances compatibility; those remain publication gates.
