# Selected and reordered structured pages

The next Save adapter preserves real PDF objects when users select or reorder unique pages from tagged sources. The existing graph path combines only complete, contiguous, original-order sources. The user has authorized autonomous implementation and routine design decisions; no approval pause or commit is required.

## Approach

Extend the graph-native path: assemble each contributing current source once, then project the assembled graph onto the ordered unique requested page identities. This retains the existing catalog, language, navigation qualification, security and structure validation. Re-importing pages through the renderer loses graph relationships; cloning a complete source for each occurrence unnecessarily amplifies memory and cannot establish correct duplicate tag ownership. Those approaches are rejected.

Stage 1 supports reordered/interleaved unique pages and subsets whose remaining graph has no reference to a removed page or structure element. Existing compatible bookmark reconstruction can remove ordinary bookmarks; explicit discard remains authoritative. Native-only bookmark metadata stays preserved when unchanged. Navigation to removed pages, including unused named destinations, is guarded until a typed navigation-pruning adapter exists. Do not silently redirect links or strip unknown metadata. Repeated pages, multiple immutable versions of the same origin, combined AcroForms and added/imported overlays retain their existing separate adapters/guards.

## Invariants and design decisions

- Resolve page identities canonically and reject unknown, empty or repeated selections. Keep content/resource/image/annotation identities and encoded stream bytes for retained pages.
- Materialize only PDF's inheritable page fields (Resources, MediaBox, CropBox, Rotate) before attaching retained pages to one new flat Pages root. Required MediaBox and Resources must be available. Null means absent. Preserve page-local UserUnit, Group, boxes and annotations exactly.
- Preserve authored logical structure order. PDF 1.7 pp. 856 and 889 explicitly distinguish it from physical page/content order. This does not implement reading-order editing or certify PDF/UA.
- Inspect the original bidirectional ownership graph before projection. Filter K leaves by their effective Pg. Retain ancestor elements needed by surviving leaves, with canonical retained identities, parent links, MCIDs and attributes. Preserve originally empty elements unless their explicit/inherited page was removed. Remove an ancestor's Pg if it points to a removed page; surviving descendants inherit the same effective retained pages as before (materialize where necessary).
- If partial removal changes an element carrying Alt, ActualText or E, refuse: guessing a replacement would misdescribe retained content. Preserve unchanged descriptions. Keep role/class/namespace metadata unchanged. Structure IDTree and cross-element attribute references require explicit handling or guarded refusal when their target is removed.
- Partial removal under an element with A/C attributes is guarded pending typed attribute projection (for example table row spans). Removed elements with ID are guarded because string relationships such as Headers are not detected by an indirect-reference walk. Unknown metadata on a page branch being flattened is also guarded. These decisions favor an explicit refusal over silently invalidating producer metadata.
- Rebuild ParentTree entries from surviving exact content-owner edges, including marked streams and whole annotations. Preserve MCID slot indices/null holes; remove keys with no surviving content only when their owners are no longer reachable. Shared owners must still satisfy the existing reverse ownership inspector after projection.
- Traverse the final trailer-reachable graph with bounded work. Any removed page/element still reachable is a guarded refusal, including opaque metadata, links, old page branches or aliases. Remove unreachable original objects only after successful inspection; page removal is not secure redaction.
- Projection consumes an owned scratch Document. Any error/cancellation discards that scratch graph and never publishes an output. Budgets: 100,000 objects, 1,000,000 traversal values/steps per bounded pass, depth 64, 16 MiB staged metadata; existing cumulative graph/parser/output budgets remain unchanged. Charge before metadata clones, never clone encoded streams.
- Source registration uses first encounter order, not assumed contiguous blocks. Bookmarks compare against current source bytes with normalized page identities before selecting Preserve versus CompatibleReplace; security provenance stays original. Old recovery provenance remains conservative.

## Verification

First reproduce existing Save refusal on unique reordered/interleaved pages and a tagged subset. Add low-level tests for inherited/null/aliased page attributes, cross-page K branches, MCR/OBJR ownership, sparse ParentTree slots, complete reorder preserving logical order, opaque references, description guards, repeated pages, cancellation and bounded metadata. Native tests must compare rendering, exact extracted text, stream bytes, retained geometry, tag owners and navigation targets after save/reopen; destination must survive every refusal.

Use authored Word/Pictures inputs for selection and permutation, then independent PyMuPDF/pypdf graph and raster audits. Bind current source/renderer fingerprints. Final implementation review occurs once after scoped gates. Desktop acceptance and a fresh candidate build remain necessary before claiming the milestone is usable in the app. Keep unresolved navigation deletion, duplicate occurrence, forms/overlays, broader regression and portable gates explicit.

## Primary reference

Local Adobe PDF 1.7 reference: `artifacts/combined-editing-save-20261030/references/adobe-pdf-reference-1.7.pdf`, pp. 145-149 (page inheritance), 856-872 (structure and ownership), 889 (distinct logical order). Baseline fingerprint: `2512809ee1fa091bab31cfe50a5eec48ede6786511f4476e6ba23d4ee07dd9a8`.
