# Faithful composition of structured PDFs

## Intent and evidence

Folio must let a user combine documents, edit their existing content, arrange pages and save a useful PDF without losing accessibility structure or changing unrelated content. The broad commercial-quality goal remains active; this work addresses one demonstrated obstacle in that workflow.

The authored tagged Word plus rotated image baseline in `artifacts/combined-editing-save-20261030/baseline-1` reaches four pages but Save refuses before any edits. `baseline-refusal-audit.json` verifies the exact inputs, Save request, absence of publication and owned-process cleanup. `complete_source_save` retains only one complete source, while the imported route deliberately rejects structure and rendering catalogs.

An isolated probe using the exact checked-in renderer (`import-probe-1/report.json`) establishes that Word's page StructParents and 19/2 MCID markers survive ordinary import and unchanged content generation, while StructTreeRoot and MarkInfo are absent. This does not prove that every marked-content form, dirty page, or annotation survives arbitrary regeneration. The original guard must remain until an admitted replacement can prove the relevant relationships.

## Approaches and coordinator decision

1. Repair the imported output after PDFium composition. This reuses the current renderer and overlay flow, but structure repair would need a complete provenance map for imported pages, resource streams, links, comments and widgets. Existing repair modules replace some of these objects independently, so copying only the tag tree is insufficient.
2. Compose bounded source object graphs with explicit identity maps, then apply page plans and existing edit finalizers. This retains marked-content bytes and gives structure/form/link work one map. It requires a new composition route and explicit document-catalog reconciliation, but avoids matching ownership by appearance or coordinates.

Choose approach 2, as an engineering decision under standing autonomous authorization. Do not remove the guard, flatten pages, discard tags, switch the Word input to an untagged equivalent, or disable combining as a substitute. Retain the byte-exact one-source fast path.

## Required end behavior and staging

The completed structured-composition work must cover multiple tagged/plain sources, existing paragraph/image edits that already produce immutable source versions, page selection/order/occurrences, normal overlays, and save/reopen. Inherited rendering settings and form/annotation identity must not be silently lost. The combined acceptance design remains authoritative; a passing append-only baseline does not close that objective.

Build in separable stages: (1) bounded graph import and source/output provenance; (2) catalog and structure reconciliation with full-source combination; (3) page-occurrence projection and overlay integration; (4) complete saved-output/desktop acceptance. Keep unsupported cases explicitly refused while subsequent stages are being implemented. Do not report a limited intermediate route as full composition or commercial parity.

## Graph ownership and bounds

Introduce an internal source-graph composer with an explicit map from source identity and indirect object ID to output ID. For page duplication, add output occurrence identity wherever a page-owned object must be independent. Preserve references and shared resource aliases within their proper scope; never locate an annotation/widget/XObject solely by bounds or pixels.

Parse each immutable source using existing strict, bounded loading. Allocate all destination IDs before moving/copying that source's objects so forward/cyclic indirect relationships are rewritten consistently. Do not rely on a traversal that ignores unreachable objects and later makes them reachable. Check missing references, object-ID overflow, direct nesting, page-tree cycles, aggregate object/value/stream bytes and expansion before allocation. Reuse the existing 512 MiB output ceiling and 64 MiB per-stream decompression bound; define an aggregate retained-graph budget so many small sources cannot evade it. Cancellation must be checked during traversal, copy, structure reconciliation, finalization and serialization.

Maintain source page-tree inheritance explicitly when installing the requested output page tree. Original Contents/Resources/Group and marked-content properties remain source-bound. Crop/rotation continue using existing physical-unit geometry. No original file is mutated. Publish only the final bounded result through the existing atomic path.

## Document structure

Preserve both directions of content ownership: structure K/MCR/OBJR references and ParentTree entries with their page/XObject/annotation StructParent(s) keys. Allocate unique parent-tree keys per imported owner, preserving MCID array positions and null gaps. Update ParentTreeNextKey. Validate actual owner identities, not only key counts. Retain nonempty trees and their parent links; reject cycles, missing owners or inconsistent reverse mappings before publication.

Merge source root children without changing their internal semantics or dropping element attributes, alternate/actual text, language or revision information. Preserve each source's language inheritance when the combined catalog differs. RoleMap, ClassMap and IDTree collisions need deterministic source-qualified identifiers and corresponding rewrites, or a specific refusal until semantics are implemented; never choose a conflicting definition silently. PDF 2.0 namespace ownership is part of the same identity problem.

For page subsets, remove only content items belonging to excluded occurrences and prune empty ancestry without dropping retained content. For duplicated pages, create distinct owner keys and corresponding structure occurrences. The exact grouping/reading-order policy for cross-page containers must be settled before enabling these operations: page-wise cloning must not accidentally split one table/list into unrelated semantic objects. Do not treat syntactic validity as proof of faithful reading order.

MarkInfo and metadata must describe the result honestly. A tagged source combined with untagged pages does not acquire complete tagging merely by retaining a tree. Do not invent semantic tags, call all added content artifacts, use Suspects as a generic escape hatch, or inherit accessibility/archive certification claims without proving their conditions. Resolve the correct partial-structure flag/metadata behavior against the specification before coding the catalog merger.

## Other catalog and edit integration

Use the existing bookmark model for requested outline changes. Preserve internal/named link targets against the explicit occurrence map, including annotation identities referenced by structure. Form roots, widget appearances, default appearance/font context, calculation order and field-target actions must use the same mapping. Reusing `acroforms::preserve` is permitted only if it exposes/reconciles any replacement IDs with tagged OBJR owners; its independent cloning must not leave stale structure references.

Keep catalog rendering guards for optional content/output intents until compatible reconciliation is implemented. Do not select one conflicting rendering configuration silently. Document-level metadata/view preferences require a stated combined-document policy, distinct from per-page semantic preservation. Existing protection and composition authorization remain upstream of the new route.

Apply overlays and imported-annotation deletions through their existing validated semantics. Any finalizer that replaces graph objects must update provenance before the final structure validation. Newly added untagged content must not inherit an unrelated source MCID. Unsupported custom semantics remain an explicit refusal rather than a lossy fallback. Reader/preview routes may remain separate, but a Save must never silently fall back to a path known to drop structure.

## Builder-check dispositions and foundation boundary

The builder check in `artifacts/combined-editing-save-20261030/builder-check.md` identified five integration decisions. These are real gates, not reasons to weaken the saved-output requirements.

1. **Graph-native finalization:** the lopdf graph stays authoritative through serialization. Existing PDFium import/resource/comment repair routes are not called on it. Later adapters must return typed field/widget/annotation replacements and deletions; one generic replacement map cannot represent a split field/widget. Appearance generation may use isolated renderer probes, never replace the authoritative graph wholesale.
2. **Editing lineage:** physical source-object maps do not authorize cross-version coalescing. Later integration must carry verified logical page/object lineage from editing transactions, or prove unchanged shared identities before combining versions. Permission origin and matching object numbers are insufficient. Until then, mixed versions are explicitly outside the enabled route.
3. **Occurrences:** preserve source semantic order within retained source containers. Full-source blocks can be appended without splitting their internal grouping. Reordering/duplicating a cross-page semantic container is not enabled until a validated occurrence projection defines its grouping; no per-page cloning assumption. Self-links/current copy and other links/first retained copy can follow existing navigation policy, independently of structure policy.
4. **Partial tagging:** the foundation does not set MarkInfo or enable catalog merging. The next-stage policy is to preserve the source logical structure while setting Marked false when untagged content is added; do not assert full Tagged PDF or PDF/UA conformance for that result. Selected-page re-edit admission still needs implementation. A new full structure validator must account for tagged forms and annotation reverse ownership; the narrower selected-text checker is not that validator.
5. **Bounds:** separate parsed-object relocation from loading. The initial primitive accepts an already parsed document and bounds aggregate stored payload/object/value counts and direct nesting before committing objects. It cannot bound allocations already performed by the parser, decompression, allocator overhead or total process memory. Parser admission/expansion and cancellable serialization are separate required work before enabling composition. Do not describe existing strict loading as an aggregate memory guarantee.

The first implementation plan is `docs/superpowers/plans/2026-10-30-composition-graph-foundation.md`. Its relocation map is useful regardless of the remaining catalog choices and remains disconnected from Save until those gates are implemented and tested. The full combined editing objective is unchanged.

### Partial-structure evidence and consequence

The official Adobe PDF 1.7 reference was retrieved and hash-bound in `artifacts/combined-editing-save-20261030/references/mark-info-extract.json`. Sections 10.6–10.7 (printed pages 855–856 and 883–884) distinguish extensible logical structure from the additional Tagged PDF conventions. Table 10.8 defines Marked as a conformance flag, default false; section 10.7 requires true for Tagged PDF. Therefore retaining a structure tree does not justify claiming complete tagging after appending untagged pages. Setting Marked false while retaining the tree is the coordinator's conservative output policy, not a claim that all assistive software will use partial structure. Preserve UserProperties and actual TagSuspect meaning where present, without inventing suspects. Do not propagate a source's PDF/UA/PDF/A conformance declaration to the combined catalog; copied source metadata must not become the combined document's declaration. Independent reader/accessibility behavior remains an acceptance limitation.

### Existing parser reuse discovered during foundation work

The repository already has `lopdf::Document::load_mem_with_resource_limits` in `vendor/lopdf-0.45.0/src/load_limits.rs`, exercised by `redaction_loader_tests.rs`. Reuse and extend this opt-in facility rather than duplicating parser accounting. It bounds input, aggregate decoded/object bytes, parser attempts and cross-reference entries per load, but currently has no cancellation callback or multi-load remaining-budget result. Sequential parsing plus bounded retained graph accounting can bound simultaneous ownership; cancellation and serialized output checkpoints still need implementation and tests. Existing `pdf_output::load` does not opt into this facility.

## Verification

Native regressions must include two tagged sources with colliding parent keys/role/class/ID names; marked text and tagged reusable forms; tagged links and widgets; language inheritance; subset/reorder/duplicate and cross-page containers; overlays/deletions; non-unit physical pages; malformed/cyclic/missing owners; bounded expansion, cancellation, permissions and unchanged destinations. Preserve prior refusal tests as negative controls for still-unsupported semantics and replace only the cases now genuinely supported.

For the existing four-page baseline and subsequent edits, independently compare full extracted text/order, source glyphs outside edited regions, images, links, annotation/form identity, nonempty forward/reverse structure relationships and rendering at multiple scales. Reopen in Folio and re-edit the saved paragraph/image. Keep source hashes and candidate/helper fingerprints, retain failures, and verify all owned processes close. Exact pixels alone do not prove valid structure.

## Primary references and remaining builder decisions

Adobe's [PDF 1.7 Reference](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.7old.pdf), section 10.6, describes structural parent keys and content ownership. The [PDF Association ISO 32000-2 errata, clause 14](https://pdf-issues.pdfa.org/32000-2-2020/clause14.html) clarifies structure-root children, parent-tree ownership and namespaces. These require consistent forward/reverse references; the route choice and staging above are engineering inferences, not quotations or a conformance certification.

Before enabling the complete composition route, implement and verify the builder dispositions: graph-native finalizers with typed identity updates, proven cross-version lineage, semantic occurrence projection, honest partial structure and selected-page re-editing, and parser/serialization resource controls. The tested relocation foundation is only the first layer. These are technical decisions delegated by the user, not reasons to ask routine permission or redefine the goal. If the preferred route cannot meet them, revise its architecture rather than weakening acceptance.
