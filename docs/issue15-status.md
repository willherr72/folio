# Issue 15 acceptance status

v0.13.0 implements bounded positioned single runs, explicitly selected adjacent groups and isolated nested-form occurrences.

- B1: Same-font single-run edits preserve explicit spacing, affine transforms and
  inherited text state through a proven native or uniquely mapped stream path.
  Original encoding, font ownership, neighboring text, page geometry and visible
  output are checked before publishing a new immutable source.
- B2: Explicitly select 2–8 compatible adjacent pieces, verify their lossless
  merge, then apply one replacement through the immutable editing pipeline.
  No grouping is inferred from proximity or a shared font name. Exact font
  identity, encoded characters, glyph geometry and page raster are checked.
- B3: Select one supported nested-form text occurrence by its complete invocation
  path. Clone only its resource chain, preserve other shared placements, and
  verify the resulting page before publishing an immutable source. This first
  subset supports standard Latin Type1 fonts, printable ASCII and positive
  scale/translation in conservative text-only content; it does not extend all
  page-level font and text-state support into forms.

The page-level positioned route is intentionally conservative. Ambiguous operator
mapping and relative neighbor movement remain refusal cases. Font substitution
on explicitly spaced pages is not enabled. Existing shaped-text restrictions in
#14 also remain; this stage extends the verified unshaped Latin path.

Grouping additionally refuses tagged/marked content, explicit spacing, generated
or omitted characters and incompatible reading order. Even tiny raster changes
from fractional glyph advances can prevent an otherwise plausible group from
passing. This is a conservative supported subset, not arbitrary line editing.

The [adjacent-group reader matrix](issue15-group-reader-verification.json)
accepts 32 reference cases: within-word splits at all
four page rotations, and leading/trailing boundary spaces at 0° and 180°, with
shorter/equal/longer/no-op edits. The fixture’s space-boundary groups at 90° and
270° are explicit appearance-refusal cases. This describes the tested fixture
outcomes, not a blanket restriction based only on page rotation.

See [editing behavior](existing-text-editing.md) and the
[independent reader report](issue15-reader-verification.json). Manual Foxit/IME
acceptance still belongs to the outstanding #14 work and is not inferred here.

The [repeated-edit resource check](issue15-resource-verification.json) performed
20 same-font positioned edits: each resulting PDF contained seven objects; byte
size stabilized at 947 after the initial 938-byte output. Closed source IDs became
unavailable and the original file stayed unchanged. These are fixture-level
ownership/object-growth checks, not a process working-set benchmark or an 8 GB
hardware acceptance result.

The [form reader matrix](issue15-form-reader-verification.json) compares five
export/reference pairs with exact MuPDF text, glyph geometry and raster, plus
pypdf text. The explicit-resource case positively copies the changed string in
both readers. Four inherited-leaf cases preserve pypdf’s existing empty extraction;
that equality is not evidence of successful pypdf copying. Eight repeated form
edits retain ten PDF objects each and make closed source IDs unavailable; this is
a fixture resource check, not a process-memory benchmark.
