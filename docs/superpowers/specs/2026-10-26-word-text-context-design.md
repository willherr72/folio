# Editing original Word text with retained page context

## Intent and evidence

Deliver an actual insert/edit/save/reopen workflow on both original locally
generated Word PDFs in `artifacts/producer-text-20261026/inputs-2`, including the
first paragraph's single-run and styled-paragraph edits. Font proof now succeeds;
clipping, page transparency groups and tagged marked content are the next blockers.
The objective remains everyday Adobe/Foxit usefulness, not merely removing an error.

Observed inputs: each block uses `q 0.00000912 0 612 792 re W* n ... Q`.
The page Group is `/Type /Group /S /Transparency /CS /DeviceRGB`. Tagged output
also has StructTreeRoot/MarkInfo, page StructParents and inline
`/Span << /MCID n /Lang (en-US) >> BDC ... EMC`. Inspection found no ActualText,
Alt, expansion text or layout attributes in this sample's structure subtree.
The source has whitespace-only native objects; they remain unselected neighbors.
The original accented CID subset still lacks h; no implicit substitute is allowed.

## Approach and alternatives

Extend the existing isolated edit + exact complete-source publication pipeline.
It already proves fonts, glyphs, cursor placement, collisions, neighboring objects,
pixels, immutable ownership and permissions. Add bounded content-context proofs
and preserve the original group/tag graph in the exact same-source publication
route. Arbitrary page replacement keeps its existing exclusions.

A separate full-source text editor could patch the original graph directly, but
would duplicate existing paragraph, font, comment and transaction verification.
Relying only on unchanged page pixels would miss clipped replacement glyphs and
broken accessibility references. Removing clips or normalizing diagnostic copies
changes source semantics and is not an implementation option.

## Content-context proof

Add a focused Rust module under text_edit for bounded parsing of top-level source
operators. Use strict content decoding, existing 16 MiB / 100,000-operation limits,
finite bounded operands and at most 64 graphics/marked scopes. Record the active
clip and marked-content ownership for each non-cursor-only text show, in the same
order as the existing source operand/native snapshot mapping. Require unique
mapping before using a context to authorize a selected run.

Clips are expressed in raw page coordinates, separately from displayed geometry
and UserUnit conversion. Initial clip is unbounded in this model; the existing
visible-page containment remains independently required. Track CTM, active clip
and text rendering mode in graphics state. Track current path and pending W/W*
outside q/Q: a path is not restored by Q. Path coordinates are transformed when
constructed, not when later consumed. A clip takes effect at its path-ending
operator, then intersects the current clip. Nested clips and empty intersections
must be represented accurately. Support a single rectangle transformed to an
axis-aligned rectangle, including translation, scaling, reflection and quarter
turns. Multiple subpaths, curves or arbitrary-angle clip edges are not proved.
Unknown active clipping must refuse the affected edit, rather than becoming an
unbounded region. Text clipping modes must not be mistaken for ordinary fill.

Before and after a single edit, the selected glyph geometry must lie inside its
proved clip; do not authorize clipping away part of the replacement. Preserve
the clip operators and force the existing stream-preserving route when a clip is
present. Do not feed such pages through native content regeneration. Keep no-op
glyph/pixel proofs, replacement-text checks and neighbor proofs unchanged.

For a paragraph, all selected shows must share the same proved clip and compatible
marked ownership. Constrain the proposed paragraph area/generated glyphs to that
clip. The styled splicer currently unwinds q/Q before emitting in raw page space;
wrap the new fragment block with the effective rectangle clip, then restore the
original prefix state as before. Preserve the original clipping operators for
the remaining content. Account for this wrapper in block_start/replay hints;
do not change the canonical fragment format. Uniform publication needs the same
clip constraint/wrapper. Existing group-merge editing must retain its clipping
refusal until it has its own context proof; sharing preflight must not silently
enable that separate path.

## Marked content and tags

Admit paragraph selection only within one compatible source marked scope; do not
merge separate semantic scopes. Initially prove inline BDC dictionaries containing
only MCID and optional Lang, with normal named tags; preserve operands verbatim.
Reject active ActualText/Alt/E or unsupported named property dictionaries. Preserve
balanced nesting and unique MCID ownership. No new MCIDs are invented.

Styled insertion happens inside the existing marked scope. Prefix reconstruction
must not replay BDC/BMC/EMC, because marked content is not part of q/Q graphics
state. Do not duplicate MCIDs or move generated text outside its original tag.
Uniform append-at-end publication must either use the same checked insertion
strategy or retain a refusal for tagged requests; the desktop styled workflow
and single-run Word edits are required acceptance paths.

Exact publication keeps the destination page dictionary identity, StructParents,
catalog StructTreeRoot/MarkInfo, ParentTree and their referenced objects. Admit
only verified text-only tag ownership whose semantics survive the edit. Bounded
structure inspection must reject alternate/expansion text, layout attributes that
would become stale, stream-bound MCR references, or unsupported ambiguous ownership.
Original page references must still resolve to the same page. Keep arbitrary page
grafting, cross-source structure import and tagged page duplication unsupported;
reject selected duplicate occurrences before publication rather than corrupting
ParentTree ownership. Reordered workspace references must not change source page
identity; unsupported later composition still gets its existing clear save error.

## Page-group context

Allow the exact text-publication route to retain a page transparency group only
when original and candidate contexts agree. Prove bounded group dictionaries with
Transparency subtype, a device blend color space (RGB/Gray/CMYK), and valid
optional isolation/knockout booleans. Preserve the original Group value/graph.
Resource-dependent blend spaces, unknown group fields, malformed values and
context mismatch retain a refusal. Do not merely exempt Group from the generic
guard. All other existing catalog/page restrictions remain unless specifically
covered by the same-source tag proof above.

## Integration and ownership

Relevant modules: text_edit.rs/preflight and replace_transaction_text;
positioned_text.rs/source_text.rs; text_paragraph.rs preparation/publication;
styled_paragraph_splice.rs/publish.rs/memory.rs; occurrence_text_edit.rs and
page_content_replace.rs. Prefer new focused context/tag/group helpers over growing
an unrelated module. Share invariants, not permissive booleans that callers can
forget to validate. No API caller supplies trusted clip bounds or tag identity.
All context comes from immutable source bytes and native snapshots.

Use existing font proofs, byte/resource budgets, protected-document policies,
comment/form preservation, abandoned-request cleanup and atomic save publication.
No new dependencies, private-data upload, source rewriting, commits or release.
Keep every earlier executable and app. The user's autonomous-work authorization
covers implementation choices; do not request routine approvals.

## Required evidence

- Unit tests for nested clip intersections, q/Q versus pending path ownership,
  transformed rectangles, empty/unknown clips, malformed state, text clipping,
  marked-scope ownership and bounded parsing. Observe meaningful red tests first.
- Public synthetic fixtures with source fonts, neighboring text/graphics, a second
  page, Group and tags. Check edit/reflow, save/reopen/re-edit, unchanged source,
  font bytes, neighbor pixels, page/catalog/tag relationships and independent
  reader output. Exercise crop/rotation/UserUnit and overlapping/shared resources.
- Negative cases: partly clipped source/replacement, rectangle crossing clip,
  nonrectangular/unknown active clip, mismatched group, changed marked ownership,
  ActualText/Alt/layout/stream-bound tags, tagged duplicate occurrence, stale text,
  permission failure and abandoned candidate cleanup. Existing generic page
  replacement exclusions must stay covered.
- Original Word false/true tagging inputs: first sentence insert this, styled
  paragraph growth/reflow, save/reopen/re-edit; preserve neighbors, second page,
  original hashes, metadata and original structure references. Do not replace the
  input with diagnostic copies. Keep missing subset glyph refusal explicit.
- Affected native targets plus fresh frontend/native build, one final independent
  source review, and owned Windows UI acceptance of the original Word workflow.
  Independent MuPDF/pypdf checks supplement native proofs. No PDF/UA or parity claim.

## Builder-check resolutions

The three questions in
`artifacts/word-text-context-design-review-20261026/review.md` are resolved below.
These are implementer decisions under the standing autonomous-work authorization.
No scope reduction: original Word single/styled editing remains required.

### 1. Source-bound semantic ownership

The original complete source is the authority, never the projection's incomplete
catalog. Accept an indirect StructElem owner whose selected MCID has reciprocal
ownership through page StructParents, the unique ParentTree key/array slot, and
the owner's K. Accept integer K, arrays, and page-bound MCR dictionaries without
Stm/StmOwn. Inherit Pg through StructElem ancestors when absent; require the same
original page object. Require a unique reachable owner from StructTreeRoot/K and
a reciprocal, cycle-free P chain back to that root. A bounded K-tree walk must
rule out duplicate owners for the selected page/MCID. ParentTree number-tree
Kids/Nums forms are allowed with unique keys, bounded nodes/entries and no cycles.
Use 65,536 structure nodes, 100,000 number-tree entries, MCIDs 0..65,535 and depth
64 as explicit ceilings. Reject unsupported K encodings; annotation OBJR nodes
can remain untouched but cannot own the selected text. Stream-bound MCRs remain
unsupported throughout the inspected tree because replacing Contents could
strand their stream relationship even outside the selected paragraph.

Reject ActualText/Alt/E, A, C (class-based attributes), Ref and AF on the selected
owner or any ancestor. Do not guess which inherited/class semantics need updating.
Unrelated owners' alternate text is preserved unchanged. No unknown properties
may authorize selected content. Source inspection now records the actual Word
topology in `artifacts/word-text-context-20261026/source-inspection.json`: the
first selected paragraph belongs to MCID 0, owner 21 0 R, page 3 0 R, under the
Document owner 20 0 R and root 16 0 R. Unselected whitespace may remain in this
same owner; it must retain its original scope and native glyph/state evidence.

Assign marked-scope identities by opening-event ordinal, not operation index.
All selected shows must occupy the same exact full marked stack, with one admitted
MCID owner; sharing a tag name or ancestor alone is insufficient. Across editing,
the entire ordered BMC/BDC/EMC event sequence and operands stay unchanged. Preserve
each untouched show's clip/marked context; every generated replacement show gets
the selected context. Use the existing proven native run mapping plus an internal
placement mode (single, source-anchor styled fragments, or uniform append) to map
source selection to candidate shows. Do not take a trusted context from the UI,
metadata, or an arbitrary incoming PDF. Legacy form/group publishers that lack
this top-level selection proof keep their existing metadata exclusions.

### 2. Projection and Group agreement

Use a shared supported-Group descriptor before and after every text page-copy
boundary (including paragraph projections). Compare resolved semantic values,
not cross-document object IDs. Absence must match absence. Present groups require
S=Transparency, a present device CS, optional Type=Group, and optional boolean I/K;
false is the default for I/K. Other keys/types/spaces refuse. Source-to-baseline,
baseline-to-candidate, and source-to-final descriptors must agree. If copying loses
or changes the group, refuse with a concrete message in this implementation;
never silently validate under a different group. Existing saved-copy evidence
shows PDFium retains both sample Word groups and StructParents. Final publication
retains the original Group object/reference and graph, not the copied descriptor.

The exact-publication relaxation requires the source-bound context proof above;
generic page replacement remains unchanged. Source and final page IDs,
StructParents, root/ParentTree and selected owner/ancestor objects remain identical.
Tagged selected-page duplication is refused before creating a candidate. Protected
policies and existing annotation appearance preservation remain mandatory.

### 3. Clip recipes, envelope ownership and hints

Do not flatten transformed clip boundaries to newly rounded coordinates. Retain
bounded source rectangle/CTM recipes for the active intersections. At the raw-page
insertion point, a recipe can construct its original path inside q/CM/re/Q and
then apply W/n outside that inner q, since the path survives Q. This restores the
identity CTM without discarding the new clip and preserves the original rectangle
and transform operands. Bound retained/emitted recipe operations by the same
content operation/byte ceilings; shared histories must not cause unbounded copies.
Reparse emitted context and require the same effective clip before publication.

Wrap fragments and any generated backgrounds in one balanced clip envelope.
block_start points after its opening operators at the canonical 13-operator
fragments. On remembered re-edit, recognize an envelope only from actual source
operators and independently verified selected fragment/background ownership. Its
entire interior must consist of those owned blocks, with no unrelated operations
or marked events; effective context must agree. Remove that complete envelope
before replacing it. Otherwise do not claim wrapper ownership. This prevents
obsolete envelopes from accumulating until re-edit limits fail.

Hints remain disposable. Add a versioned optional binding digest for clipped or
marked paragraphs, derived from the currently proved clip and full marked stack.
An altered clip, MCID, language or scope must invalidate the old hint even if
glyphs/pixels are unchanged. Check it against freshly parsed context; the digest
cannot authorize a clip or owner. Old hint versions are eligible only in their
previously supported unmarked/unclipped context. New clipped/marked records need
the new version/binding. Native fragment, glyph, resource and exact replay-pixel
proofs remain required. Uniform clipped hints need the same binding; uniform
tagged reflow retains a clear refusal until it uses source-scope insertion.

Tests must include bounded repeated envelope replacement, several actual saved
re-edits, and altered-context hints with otherwise identical glyphs/pixels.
