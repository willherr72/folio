# Replace one page occurrence without splitting its source

The first native content-replacement API retains a complete source but changes a
page index shared by every duplicate. Moving only the selected workspace page to
another source breaks cross-page links and splits shared form fields. The current
goal requires one selected copy to change, preserving interactions and reversible
workspace state. This design is an implementation decision under standing
autonomous authorization; it is not a user-selected architecture.

## Approaches and evidence

Mixed old/new source mappings failed the navigation/field probe. General persisted
source lineage could unify them but would require new ownership semantics across
every editing and recovery path. Composing the whole workspace would risk baking
current crop/rotation/overlays into new source geometry. Instead, keep one full
source and create one additional native page for the selected repeated index.

The feasibility probe `artifacts/page-occurrence-probe-20261024-2` proves first/last
copy replacement, unchanged other pages and shared field filling. Independent
pypdf/MuPDF checks confirm local targets, one field with three widgets, text and
exact two-scale pixels. The first probe failed only because its expected phrase
did not match the fixture; that evidence remains. Production must preserve the
original catalog rather than rebuilding the whole document as the spike did.

## Native operation

`replace_page_content_occurrence(destination_id, occurrences, selected_occurrence,
incoming_id, incoming_page)` accepts the ordered native page indices of every
workspace occurrence using the destination source. Return a new full source plus
one mapped page index per occurrence. Bound occurrences to 100,000; reject empty,
out-of-range or invalid selections. The original files and sources never change.

If the selected index occurs once, use the existing immutable replacement and
keep all indices. If it repeats, append exactly one native copy to a clone of the
full original source. Keep existing page indices and original catalog metadata.
When the selected occurrence is first, replace the original index and map all
other copies of that index to the append. Otherwise replace the appended page and
leave all other occurrences at the original index. Links from other source pages
continue targeting the first workspace occurrence; self-links remain occurrence
specific when exported. Other repeated page indices need no extra native copies.

Copy the extra page with the existing renderer, merge its renumbered objects into
the original lopdf document, append it to the original page-tree root, and repair
page resources, native comment geometry, AcroForm and links using bounded existing
helpers. Preserve catalog/bookmark references to existing page IDs. Prune obsolete
import graphs. Then run the existing content replacement on the chosen native
index, retaining its geometry, annotation contexts and validation restrictions.
Load the final source through the existing atomic asset-registration path; no
intermediate source is registered. An abandoned reply must discard the candidate.

## Protection and recovery

Retain edit-content requirements and incoming copy/unprotected requirements.
Appending a duplicate additionally requires native assemble permission. A protected
source policy appends the copied page's original provenance and overlay baseline,
retaining its original protection identity and credentials. Duplicate provenance
is valid only with assemble permission; no-assemble recovery keeps the existing
uniqueness restriction. Page indices remain bound to original page-count limits.
Protected save/reopen and recovery must retain permissions and origin. No plaintext
file is written by preparation. Prepared print sources remain ineligible.

## Editor integration and limits

The later dialog will freeze the entire session page array, choose incoming page,
preview the native candidate, then update every occurrence of the old source to
the returned source/mapping in one history commit. Keep PagePlan IDs, crop, rotation,
overlays and workspace bookmarks unchanged. Keep old sources for undo; close stale
candidates. Current crop/rotation/overlays are never inputs to native copying.

This native milestone does not yet expose IPC/UI. It does not reconstruct lineage
already lost by prior separate-source edits, import incoming annotations, or widen
the existing content-replacement/action support. Those gaps remain on the project
ledger. The complete replacement workflow still needs preview/apply, undo/recovery
and actual Windows acceptance.

## Acceptance

Test first/middle/last duplicate selection, one and many occurrences, nonduplicate
fast path, invalid indices/counts, raw crop/rotation/UserUnit geometry, top-level
text editability, native comments and shared form filling, local/self/named links,
catalog metadata and bookmarks, source integrity, repeated replacements/graph
growth, protected assembly/edit denial and allowed recovery, and abandoned replies.
Use independent readers and focused regression suites. One scoped fresh review
followed by one important-finding correction pass; preserve all evidence.
