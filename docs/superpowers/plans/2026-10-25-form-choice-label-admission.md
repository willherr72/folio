# Original-font admission for every choice label

Execute inline after the font repertoire checkpoint passes. Preserve the worktree,
inputs and failed evidence; no commits. One Cargo/build at a time. Whole-feature
review remains at the end of the existing choice editor, not this private layer.

## Design and interface check

The native generator already inspects its staged Context and resolves the exact
original widget font, preferring that appearance's resources over form defaults.
ChoiceFontProof now proves bounded resource Unicode/code/glyph/width mappings.
Connect these interfaces before constructing a native probe. Each call proves
every option display label, including unselected/offscreen rows, plus free-text
current/default answers. A stored export value is not automatically visible text:
matching defaults use their option's display label. Do not demand glyph coverage
for opaque export keys when their rendered display labels are supported.

The inspected model bounds strings and counts before collecting borrowed slices.
No original strings are cloned for the proof. Check the exact resolved font for
each widget; never accept the same font name as evidence of another widget's
resource. The caller's future all-widget transaction must execute all these proofs
before publishing any candidate.

This is a necessary admission check, not native encoding/layout proof. Preserve
the existing generated font-graph comparison and all empty/no-op/artwork checks.
Do not enable a public endpoint or infer supplementary/shaping support from a
positive nominal glyph map. Later native generated-code comparisons and geometry,
global NeedAppearances and source no-op checks are still required.

## Task 1 — wire the resource proof before native generation

Files: form_choice_probe.rs, engine_choice_probe_tests.rs. Snapshot both files
against the previous verified checkpoint before adding tests.

- [x] RED: unsupported unselected dropdown label and offscreen list label must
  fail original-font admission, even when the visible selection is representable.
  Unsupported free-text default/current answers must also fail. Assert the font
  error so an unrelated parser/generator failure cannot satisfy the regression.
- [x] GREEN: collect all display labels and free text from the inspected model;
  call ChoiceFontProof::for_texts with the original resolved widget resource before
  Copier/native construction. Matching defaults need no duplicate export proof.
- [x] Positive regressions: accented embedded label; Unicode opaque export key
  with representable display; linked widget resource admission; original fixtures
  and clear/restore/artwork checks. Preserve bytes and expected no-op pixels.
- [x] Run affected library and choice/embedded integration gates. Record exact
  source/evidence fingerprints and pending native-codec/publication/UI limits.

Expected RED: at least the unselected dropdown case currently succeeds and fails
the new assertion. Expected GREEN: focused and affected gates pass, with no source
mutation or new public endpoint. Follow any genuine native behavior discrepancy
to its source rather than assuming the missing proof is the only possible cause.

Observed RED: unselected dropdown generation succeeded; offscreen list and current
free text failed at the later native-substitution gate, not original-font admission.
The corrected run passed16probe tests. Expanded final gate adds a same-name/different
encoding linked-widget regression. Independent readers will verify opaque export
pixels and exact accented native codes on both widgets, plus prior generation and
empty-transition cases. Existing public publication/UI gates remain pending.

The first expanded linked-widget test assumed an absent Encoding meant WinAnsi;
it correctly failed. The fixture now explicitly creates WinAnsi and Standard
resources under the same F1 name. Failed test/log/output/fingerprint preserved.
Corrected final gate339passed,3ignored;19independent generation/transition/label
cases passed. Product source was unchanged by the test correction. Fingerprint
b291446103a260678302eacf3efcbb7da61d68bb3559003b9c13f6b6bff759ee.
Evidence: artifacts/form-choice-label-admission-20261025/verification.json.
