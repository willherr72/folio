# Faithful tight paragraph leading

The source-alignment milestone is verified. Its synthetic mixed 12/16 pt Courier
paragraph has 18 pt baseline gaps and native visible clearances of 8.400/7.632 pt,
but styled reflow expands the gaps to 18.112 pt from whole-font metrics. Preserve
the requested/source leading whenever the actual text and marks fit.

## Approach

Keeping whole-font minima preserves this known defect. Removing clearance minima
could collide letters and highlights. Use measured visible ink for new paragraphs'
vertical extents, retaining nominal em-based leading and full-font fallback for
blank rows. This is the implementer's choice under the user's autonomous-work
authorization. Existing Rust/PDFium pipeline and frontend interfaces remain.

For a row with painted glyphs, ascent is max(0, ink top) and descent is
max(0, -ink bottom), including verified highlight padding. Ignore whitespace and
degenerate ink when accumulating these extents. For rows without painted glyphs,
retain the existing full-font extents, including every whitespace style. The em
used for nominal leading still includes all row styles. The baseline gap remains
max(previous descent + current ascent, max adjacent em * requested spacing).
The first row's ascent and last row's descent use the same policy; automatic
initial framing keeps the first source baseline, horizontal anchor and row count.

The existing full-font native probe must include unmarked full-font styles for
this policy, so imported and registry fonts use reproducible native ink. Existing
source-font metrics remain checked through their source resource probes. Native
publication still verifies every fragment, glyph origin, frame containment,
background ownership and surrounding pixels. No tolerances or budgets increase.

## Saved layout compatibility

Add a version5 `ink_vertical` boolean to disposable paragraph hints. Fresh
paragraphs opt in; restored versions1–4 stay on their exact prior policy, including
explicit subsequent edits. Version5 records may coexist with older records.
Reject this flag in versions1–4 and reject true without text_alignment. Do not
trust stored coordinates: retain independent resource/operator/fragment replay.
Preserve a pre-change version4 tight-leading PDF as a golden fixture before edits.

## Scope and evidence

No new UI, font support, shaping, bidi, or uniform legacy layout changes. No
automatic migration of old saved paragraphs. Keep 4096 bytes/spans/fragments,
64 styles, 256 lines, physical 1–300 pt and spacing0.6–3 limits unchanged.

Tests cover fresh18pt gaps, saved/reopened exact replay, v4 golden18.112pt gaps,
mixed whitespace/blank rows, accents/descenders, marks, substitution/released fonts,
rotation/crop/decimal UserUnit, malformed hints and unchanged source bytes.
Independent PDF readers verify baseline positions, reading order, and pixels.
One final scoped review, corrections inline, then final native/build/desktop gates.
Preserve prior accepted EXE and all failed evidence; no commits or publication.
