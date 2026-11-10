# Complete-source paragraph transactions

## Intent

Extend the accepted complete-source single-run transaction to uniform and styled
paragraphs. Paragraph preview must preserve shared fields, links, bookmarks,
annotations, native page frames, protected provenance and duplicate ownership.
This native milestone supplies the desktop integration that follows; it does not
claim that existing UI preview/Apply already uses the new APIs.

## Approach

Reuse existing paragraph admission, layout, collision, highlight, replay and raster
verification inside a private page projection. Then graft the verified candidate
into the full immutable source through the accepted occurrence publication helper.
This is the coordinator's choice under standing autonomous authorization.

Alternatives: threading a new projection mode through every layout/measurement
helper is efficient but spreads the ownership distinction through many algorithms.
Allowing legacy isolated publication to split fields would violate document
identity. A private source projection keeps the existing layout algorithms intact;
it costs an extra copy and must prove that copy is faithful before inspection.

## Contract

- New occurrence inspection/suggestion/layout/fit APIs are explicit; legacy APIs
  keep their isolation guard. Request, selection and fit rectangles/font sizes
  remain physical points; diagnostic layout output keeps the legacy raw PDF axes.
- Projection checks original edit rights and source restrictions. Projected policy
  and original path remain derived from the selected source page, never unprotected.
- The projection is private to one worker request. No projection ID escapes. Dispose
  it on success and every error; discard final results on abandoned worker replies.
- Preview accepts the complete source's occurrence vector and selected occurrence,
  returns PageContentReplacement, and changes only that occurrence. Duplicate edits
  require assembly rights; preserve all current source pages and mappings.
- The existing paragraph engine proves the private edit. Complete-source publication
  must match candidate text objects, glyphs and raster before returning a result.
  Carry bounded paragraph hints; independently re-inspect/replay published output.
- Fonts, highlights, spacing, alignment, blank lines and physical-unit conversions
  retain existing validation. No-op replay must not change text/pixels. Foreign
  malformed metadata cannot authorize relaxed validation.
- Originals remain unchanged; source ownership never crosses requests/tabs by accident.

## Verification

Use synthetic two-page PDFs with one field shared across pages, native notes, links
and bookmark. Cover uniform and mixed styles, first/later/unique occurrences, reflow,
re-edit/no-op, save/reopen/refill, rotations and UserUnit 1/1.7/2, protected edit versus
assembly rights, stale requests and failure cleanup. Retain independent reader
artifacts and compare selected output to isolated reference plus untouched pages
at two raster scales. Existing paragraph and occurrence regression suites must pass.
One scoped final review, one important correction pass, no automatic rereview.

## Remaining product work

Desktop capability routing, preview ownership/Apply, undo/recovery and actual Windows
acceptance are the following integration milestone. Group and nested-form transactions,
broader producer coverage, performance and release/parity are still open.
