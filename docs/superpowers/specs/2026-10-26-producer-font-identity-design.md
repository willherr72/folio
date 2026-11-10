# Producer font identity and sparse widths

Folio must let users edit existing text with its actual embedded font when the
source character mapping and metrics are provable. Original Word exports expose
two false refusals: sparse WinAnsi widths without ToUnicode, and simple/CID
resources sharing a font program but using distinct renderer identities.

## Approach

Keep resource-specific mapping proofs, keyed by program hash and the renderer's
font name. PDFium build 8044 strips the six-uppercase subset prefix for embedded
simple fonts and exposes the CID descendant BaseFont unchanged. Use this same
identity when matching source operands for single, positioned, substituted and
paragraph edits. Do not prefer a first match; conflicting proofs/errors under
the same identity still refuse. Parent and descendant CID names may differ.

For the editing-only implicit WinAnsi repertoire, a zero PDF width that disagrees
with a positive program advance is an unavailable code, not a reason to reject
every other code. Explicit ToUnicode, nonzero contradictory widths, malformed
maps, glyph agreement, and the separate all-scalar form repertoire remain strict.
Genuinely matching zero advances retain existing behavior. Source text using an
unavailable code must still fail its decoding/support proof.

Alternatives: universal subset-prefix stripping conflates mappings; first-match
or font-program-only lookup can use the wrong encoding; synthesizing widths or
rewriting producer PDFs changes source semantics. None is appropriate.

## Scope and validation

Existing Rust/PDFium/lopdf architecture, no dependencies or UI changes. Regression
fixtures use the licensed existing DejaVu font. Test sparse unavailable codes,
explicit contradictory maps, different and colliding renderer identities, and
CID parent/descendant names through real public editing APIs. Verify original
producer inputs are unchanged and record later refusals honestly. Word page
clipping, transparency groups and tags remain subsequent compatibility work.
No font substitution is implicit; raster/neighborhood and publication guards stay.

Run focused red/green tests, library and affected embedded/form/paragraph native
targets, fresh build and producer probe. One final independent review. No commits,
release, cleanup or broad parity claim. Preserve the prior executable checkpoint.
User-authorized autonomous continuation supplies execution approval; choices above
are implementer decisions, not newly elicited user preferences.
