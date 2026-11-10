# Usable embedded-font repertoire

Status: implementation in progress. The standard-font Latin milestone is closed;
parser and source-editing regressions pass, with wider consumer acceptance pending.
Standing user direction authorizes autonomous implementation without repeated
design questions, with one final scoped reviewer and no commits or publication.

## Problem and evidence

Folio refuses all three first-page text runs in the untouched public producer
`artifacts/producer-text-20261026/inputs-2/mupdf-True.pdf`. They use an embedded
TrueType font with an Identity-H composite encoding. Font-wide ToUnicode parsing
fails on unused destinations that do not encode one UTF-16 scalar. A later check
also requires an explicit CIDToGIDMap, which this file omits.

Resource-bound diagnosis in `artifacts/embedded-cmap-diagnosis-20261028/round-2`
identifies 3,444 declared codes, 108 unusable destination codes and 29 actually
used codes, with no intersection. An independent fontTools/pypdf probe confirms
that all 3,336 other mappings match the program's best Unicode cmap and advances.
This is evidence for a candidate design, not proof of safe engine admission.
Other Unicode subtables, native mapping, pixels and publication still need checks.

Three derived diagnostic copies isolate the two refusals on the existing candidate:
adding Identity alone still fails on the Unicode map; removing only the ten unused
malformed entries fails on the missing mapping; combining those diagnostic changes
admits all three runs. Both pages retain exact independent-reader text/pixels and
the same font program. These are diagnostic copies, not support for the untouched
original or permission to rewrite its mapping. Evidence is in
`artifacts/embedded-cmap-diagnosis-20261028/isolated-controls-1/result.json`.

The PDF 1.7 reference gives Identity as the omitted CIDToGIDMap default for Type 2
CIDFonts ([Adobe, table 5.14, page 437](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.7old.pdf)).
Explicit malformed values must not be treated as omission.

## Intended result

Users can edit and reflow ordinary supported Latin text in these embedded-font
PDFs while retaining the original font program, mapping streams, placement and
surrounding document. Save, reopen, and edit again work through desktop, CLI and
both MCP transports. A code with no usable verified mapping remains ineligible
in source or replacement text. This adds no OCR, ligature shaping or font repair.

## Approaches

1. Reinterpret malformed destinations as Unicode scalar numbers: rejected. It
   invents meaning absent from valid UTF-16 and can change glyph identity.
2. Require font substitution for every such file: retains the demonstrated
   original-font workflow gap, so it does not meet this milestone's objective.
3. Prove only usable entries while validating every declared assignment:
   selected. Keep structural and work limits global; keep the proof for each
   admitted character independent of native extraction alone.

## Parser and proof boundaries

Use an explicit internal proof policy to distinguish Latin text editing from
complete form-repertoire verification. Preserve the latter's strict contract;
do not silently relax `choice_font_codes` or shared form appearance generation.

The bounded CMap representation must retain every assigned source code, including
an explicit unusable marker when its string destination is not one scalar. One
bounded ordered map is sufficient; a second unbounded reverse map is not needed.
All assignments count toward the existing 65,536-code limit, overlap rejection,
codespace membership and section-count checks. Duplicate definitions are errors
even when both entries are unusable or assign the same character.

Malformed structure remains a global failure: wrong counts/types, reversed code
ranges, mixed source lengths, overlapping codespaces, inherited maps, unexpected
section operators, incomplete arrays and out-of-codespace assignments. Destination
arrays belong only to range declarations. Unusable string destinations may be
excluded in editing mode; a non-string destination is not a repairable character.

For range destinations, increment only the final encoded UTF-16 byte, with no
carry into preceding bytes, as required by PDF ToUnicode rules (see PDF Association
[clarification277](https://github.com/pdf-association/pdf-issues/issues/277)).
Never replace that with Unicode scalar addition across surrogate boundaries.
Odd-length destinations, invalid surrogate sequences and multi-scalar strings
receive no character authority. Overflow must not wrap. Each source code is still
accounted for; complete-repertoire mode rejects an unusable destination.

Keep the current 2 MiB CMap, 16 MiB font, bounded character-code and width limits.
The independent font program, glyph index and PDF advance checks remain mandatory
for admitted entries. Existing valid-map ambiguity, same-program/different-resource
collisions and source-byte roundtrip checks remain. Do not skip a valid entry's
failed glyph/width proof merely because it is unused by the selected text.

In editing mode, absence of CIDToGIDMap may use Identity for an embedded Type 2
font. Present names other than Identity, null/malformed entries and invalid
streams remain errors. Complete form-repertoire behavior remains unchanged in
this milestone. No PDF object or stream is rewritten to supply a missing default.

Source decoding must fail if it encounters an excluded code. Native extraction
that omits or normalizes that code cannot stand in for exact source proof. New
replacement text may use only the verified unique repertoire. Preserve existing
single-line/control-character, positioning, collision and raster constraints.
Partial ToUnicode proofs must explicitly require source-operand verification at
admission; a missing source mapping cannot fall back to native extracted text.
Paragraph preparation must obtain that evidence even when the raw snapshot has
no prior error. Reusable-form parsing already decodes exact original operands.

## Integration and acceptance

The engine's existing embedded-font proof is shared by single runs, groups,
uniform/styled paragraphs, explicit substitutions and reusable-form text. Verify
each affected route; do not infer coverage from a direct parser test. Existing
form-field all-scalar proof must retain negative coverage for unusable entries.

First observe the untouched original refusal with exact source hashes. Separate
synthetic regressions must isolate malformed unused destinations from the missing
Identity default. Include valid/excluded overlap in both orders, count mismatch,
array and range forms, surrogate boundaries, explicit malformed CID maps, selected
excluded codes, replacement characters outside the repertoire, invalid metrics,
ambiguous valid mappings and resource collisions. Original buffers remain intact
after refusal. Save/reopen/re-edit must reproving the retained resource.

For the public producer, test original-font insertion and paragraph growth,
consecutive saved edits, explicit substitute controls and group/form consumers.
Desktop acceptance includes Preview/Cancel/Apply, Undo/Redo, save/reopen and search.
Independent readers compare complete text, exact font/CMap bytes, nonselected
glyph positions, unchanged page pixels and other pages; include corrupted-output
negative controls. Preserve failed attempts and accepted prior packages.

Run meaningful affected native regression, fresh GUI/CLI build and both MCP
protocols with one Cargo at a time and frozen inputs. Obtain one final scoped
review. Record all remaining refusals and verification gaps without claiming
general PDF compatibility or Adobe/Foxit parity.

## Implementation discretion and remaining questions

The implementer may choose the private enum/struct layout and focused test file
organization. Do not widen public formats or add a dependency. If another
independent proof fails after the two identified boundaries are addressed,
diagnose it before extending scope or changing an admission rule. Error text
should distinguish unavailable character mappings from malformed structure.
The independent diagnostic probe is not reusable production parsing code.
