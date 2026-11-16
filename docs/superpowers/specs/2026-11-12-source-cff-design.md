# Existing CFF source-font editing

Extend Folio's existing-PDF editing advantage to statically verifiable CFF1 fonts.
The user authorized continued autonomous work toward Adobe/Foxit usefulness and
opted out of routine questions. The choices below are implementation decisions,
not a new user approval. Keep the accepted 0.45 package selected throughout this
component; a later combined portable integration has its own acceptance gates.

## Evidence and intended result

Baseline product source is `4113115d1ec3a7677b9281bfa34f3e77493c131b122ddc596523ebcb71bd4022`.
Its accepted checkpoint is `61e574e6060752ce18c52f2d514919ad10624d46e91a40f4714b6d6aa2d7c3bd`.
Stock045 rejects all three independently readable probes in
`artifacts/source-cff-compatibility-20261112/stock-045-probes-2`: a simple Type1
FontFile3/Type1C resource, a name-keyed CFF OpenType CIDFontType0 resource, and a
CID-keyed CFF OpenType CIDFontType0 resource. The stock native data accessor
returns the exact embedded program bytes in all three cases. The first authoring
attempt is preserved: generated NBSP aliases and a CID subsetter error were
fixture problems and do not establish a Folio editing failure.

The existing shared font proof only admits TrueType/Type0-CIDFontType2 and
FontFile2. ttf-parser0.25.1 understands CFF but its public glyph-width accessor
returns None for CID fonts and truncates SID fractional widths. An isolated,
unshipped extension matches 54 independent fontTools widths, including 18 CID
Font DICTs; it is feasibility evidence, not production acceptance or boundedness.

An eligible source CFF run must support original-font edits, grouping, uniform
and styled paragraph reflow, reusable-object occurrence editing and saved
re-editing through the existing shared font-proof interfaces. Mixed CFF/TrueType
paragraphs retain each source program. Original encodings, resource ownership,
clipping, tags, extraction order, neighboring content and history remain guarded.
Native desktop and CLI/MCP entry points must receive the same capability.

## Approaches

Chosen: verify the actual source CFF program, code-to-glyph mapping and advances,
then retain its immutable original resource during the existing edit transaction.
Reuse the existing parser with small documented bounded extensions rather than
introducing a second Type2 interpreter or changing the renderer binary.

Converting CFF to TrueType would change source programs/outlines and introduce
conversion fidelity work. Requiring substitution would keep rejecting ordinary
original-font edits. Those alternatives do not fulfill this component's result.

## Program and mapping contract

Recognize supported raw CFF1 and OpenType-wrapped CFF1 FontFile3 programs and
validate descriptor/stream/descendant combinations. Preserve the TrueType path;
do not confuse CFF CIDFontType0 with CIDFontType2 or honor CIDToGIDMap as CFF
authority. Ambiguous competing program entries, malformed or mismatched formats,
external streams, CFF2/variable programs and unsupported Type1/Type3 programs
retain explicit atomic refusals. A valid OpenType TrueType wrapper can reuse the
existing TrueType proof rather than being mistaken for CFF.

For CID-keyed CFF, resolve CID through the program charset to GID. For name-keyed
CFF used by a CIDFontType0, resolve CID directly as GID. For simple CFF, resolve
the actual PDF encoding (named or explicit Differences) to program glyph names
or its proved built-in encoding. ToUnicode supplies represented Unicode, with
the existing complete/partial policies and exact source-operand checks. Missing,
conflicting, multi-character and duplicate reverse mappings cannot authorize a
replacement. New text must remain in the proved resource repertoire; source
font bytes alone do not invent PDF encoding entries. Keep the existing unshaped
scalar policy. Generated native separators must not become source operands.

Glyph IDs and original widths must be independently justified. OpenType hmtx and
units are checked against its CFF glyph mappings; raw CFF widths come from Type2
charstrings and the correct SID/CID Private DICT default/nominal widths. Preserve
fractional values and account for applicable FontMatrix/units rather than assume
every program uses the same glyph space. Use the existing width-consistency
policy; do not loosen tolerances to admit a fixture. Native font program hashes,
actual preview geometry and outside-area pixel checks remain final authorities.

## Bounded parser extension

Vendor the existing ttf-parser0.25.1 source with upstream MIT/Apache licenses,
documented local patch and a Cargo path patch. No unrelated dependency upgrade.
New CFF metadata iteration must traverse glyph identifiers linearly, expose
unambiguous names/CIDs and applicable matrices, and detect invalid/overflowing
or duplicate identifiers. Validate FDSelect boundaries/order/sentinel and Font
DICT indices; avoid per-glyph linear range searches on large fonts.

Expose fractional SID/CID width reading with an explicit shared work budget.
Charge recursive Type2 interpretation, private dictionaries and required
subroutine work; a recursion-depth bound alone cannot prevent repeated expansion.
Keep the old public APIs' results stable. Proposed interfaces are
`Table::glyph_infos(remaining_work: &mut usize) -> GlyphInfos` and
`Table::glyph_width_with_budget(GlyphId, &mut usize) -> Option<f32>`; metadata
items carry GID, optional name/CID and applicable matrix. A budget/malformed-data
failure must be distinguishable from successful iterator exhaustion.
`Table::glyph_index_exact(u8) -> Option<GlyphId>` exposes the declared built-in
encoding without the existing convenience API's fallback to Standard encoding.

Retain existing 16 MiB/program, 128 MiB total-program, 64-resource, 2 MiB ToUnicode
and 4096-byte edit budgets. Initial CFF work allowance is 2,000,000 charged units
per program verification, including metadata and metric work. Do not silently
increase it to pass a pathological fixture. Normal complete glyph tables up to
the format's 65,535 glyphs must be indexed without quadratic expansion. Any
changed budget needs an evidence-backed ledger ruling.

## Acceptance and invariants

Use commit-pinned OFL Source Sans3 and Source Han Sans references. Generate small
public fixtures with reproducible fontTools transforms and retained licenses;
installed fonts and the first MuPDF failure are diagnostic inputs only. Include
raw/wrapped, SID/CID, nonidentity CID/GID, multiple FD dictionaries/subroutines,
fractional/space metrics, nondefault units, explicit encodings, missing mappings,
malformed structures, resource aliases and wrong widths. Independent fontTools
and PDF readers supply expected glyphs/advances; candidate output cannot define
its own allowed change mask or expected mapping.

Observe RED before production changes. Run affected native tests, fresh complete
native correctness/optimized-integration gates and frontend checks appropriate
to changed inputs. Keep their profile meanings explicit. Build normal optimized
GUI and CLI candidates with the unchanged complete runtime resources. Actual
desktop and automation edits must preview/apply, save/reopen/re-edit, Undo/Redo
and refuse safely while retaining the source font and unrelated glyphs/pixels.
Cover single/group/styled/uniform/reusable-form and mixed-font contexts, tags,
clipping, ownership and missing-character substitution/refusal behavior.

One final fresh scoped review follows actual component acceptance; fix important
findings once with RED/GREEN evidence, defer minors, and do not re-review. Preserve
failed attempts and the managed worktree/ledger. No commits, push, merge, public
release, accepted-package mutation or normal-user-app automatic launch. Broad
producer, arbitrary CFF, complex-script, memory, accessibility, printing/signature
and commercial-parity claims require their own evidence. Source CFF support does
not silently enable importing new CFF assets into the separate font registry.

## Primary references

- [ISO32000-1 PDF font rules](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf), section9.7.4 and tables126-127, printed pages266/285-286 (PDF pages278/297-298): CID selection depends on the CFF program kind; program container and dictionary type matter.
- [Adobe CFF1 specification](https://adobe-type-tools.github.io/font-tech-notes/pdfs/5176.CFF.pdf), sections9,13,15,18-19: charset, matrices and per-FD private data.
- [Source Sans](https://github.com/adobe-fonts/source-sans/tree/87b37a2daaed80fcb8e8ccb0085c4d72ddade12e) and [Source Han Sans](https://github.com/adobe-fonts/source-han-sans/tree/a4f7cf94edfb9d7ffbdfc4841de276358bd7e0f2), retained OFL provenance under the component artifact folder.
