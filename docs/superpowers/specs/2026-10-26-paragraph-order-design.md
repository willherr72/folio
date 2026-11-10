# Source-order paragraph publication

Coordinator decision under standing autonomous authorization. Previous goal turn
made verified progress: baseline/fit corrections,405native passes, fresh Windows
build and supplied-quiz placement acceptance. Reading-order failure is confirmed
in Folio selectable text and independent unsorted MuPDF extraction.

## Intent and approach

Keep an edited paragraph in the original selected text span's content-stream
position, through save/reopen and repeated edits. Preserve source originals,
neighbor text/cursors, graphics, comments, fonts, highlights and saved drafts.

Considered post-extraction geometry sorting, native page regeneration, and source
operator splicing. Sorting can reorder columns/tables and does not repair the PDF.
Regeneration introduces unrelated font/graphics changes. Choose source splicing,
building on existing checked show/removal bindings and canonical fragment blocks.
This preserves existing source order; it does not infer missing semantic/tagged
reading order or promise that arbitrary moved text has a new semantic position.

## State isolation

Insert at the first selected show, or before its verified self-contained block on
saved re-edit. Preserve cursor-only removals for the original selected shows.
Place new canonical text/highlight blocks in default page graphics coordinates.
When insertion interrupts source state, end its open text object, clear only the
pending path that will be reconstructed, and unwind saved graphics scopes.
Original root-level state is protected by one balanced outer scope when needed.

After the replacement, reconstruct the original graphics stack, text parameters,
line matrix and pending path from the bounded prefix without painting or copying
text. Paint operations become end-path operations; form/image/shading paints are
omitted; text shows are omitted while quote operators retain their state/line
effects. Restore the live text cursor with a numeric advance independently checked
against the first selected native text origin, line matrix, CTM and horizontal
scale. Then continue the original operators. Reject unprovable state without
publishing. Do not fall back to appending a paragraph or weaken pixel checks.

Re-edit insertion before the canonical fragment block occurs at default root
state and must not repeat the reconstruction wrapper. Keep operation/content
limits, graphics-depth bounds and bounded allocation. Test repeat size stability.

## Ownership and verification

Return the generated fragment operation range and text ordinal from emission;
these are internal transaction evidence, never persisted/client authority.
Highlight measurement and paint insertion use that range. Final text verification
checks neighbors before and after the replacement, exact fragment fonts/glyphs,
and full independent saved-memory replay. Existing metadata remains source-bound
by font/program/origin/digest rather than final-object positions. No schema change
is expected. Existing source restrictions and appearance gates remain mandatory.

## Acceptance

Synthetic native RED then GREEN: before/selected/after order, same-BT neighboring
shows and relative line motion, transformed/nested state, pending paths, highlight
paint order, multiple edited paragraphs and repeated saved re-edit. Cover crop,
rotation, physical units, embedded fonts, blank fragments and bounded output.
Run affected publication/memory/highlight/occurrence suites. One final review,
inline corrections. Fresh Windows quiz insertion/undo/redo/save/reopen/copy and
independent unsorted extraction, baseline and surrounding pixels must pass.
Retain previous EXEs and failures. No commits/publishing or parity claim.
