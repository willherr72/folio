# Complete-source PDF save fidelity

## Intent and evidence

Preserve the original PDF graph when saving a complete immutable source. Actual
Word 16 exports lose Lang, Metadata, ViewerPreferences and, for tagged output,
StructTreeRoot/MarkInfo through the current PDFium page-copy composer. Text and
144-dpi MuPDF pixels remain identical, so visual comparison alone misses this.
Evidence: `artifacts/producer-text-20261026/save-probe-1/audit.json`.
This is a prerequisite to broader Word editing, not a claim that its font,
clipping, marked-content or text-publication blockers are resolved.

## Approaches and decision

Reconstructing all catalog relationships after page import would need a separate
tag/ParentTree/optional-content/output-intent remapper. It is necessary eventually
for arbitrary combination, but is too broad for this correction. Retaining the
complete source graph preserves all original object references without guessing
their meaning. Use this route first. This is the implementer's decision under the
user's standing authorization to continue autonomously.

## Contract

- Save intent only: one source, every page exactly once in original order.
- No requested overlays and no annotations imported out of the native source
  (check the retained imported-annotation index sets, including deleted drafts).
  Never resurrect annotations deleted in the workspace.
- Retain original bytes when bookmarks, crop and rotation are unchanged. Otherwise
  load the bounded original graph and modify only requested page Rotate/CropBox
  entries and an explicitly changed outline tree. Existing page IDs, content,
  annotation references, structure, metadata, resources and trailer remain intact.
- Compare requested bookmarks to this source's actual outline tree, remapped to
  request page IDs, ignoring UI bookmark IDs. Explicit empty override removes
  Outlines; an unchanged outline preserves otherwise unmodeled source properties.
- All existing request validation, authorization, protection, cancellation,
  output byte bounds and atomic publication run. Authenticated source bytes are
  decrypted; output protection must be reapplied through the common final stage.
- Rotation is relative to native inherited rotation. Crop uses existing raw-unit
  geometry logic. No content regeneration or page duplication in this route.
- Other Save requests retain the existing composer, except source catalogs with
  StructTreeRoot, MarkInfo, OCProperties or OutputIntents are refused before page
  copying: silently discarding those relationships is not an acceptable fallback.
  This is a preservation guard, not a complete PDF conformance validator.
- Preview, printing, image and text export retain their existing composition path.
- No dependencies, frontend changes, source overwrites, commits or releases.

## Acceptance and limits

Native public-API regressions must prove complete graph retention, nested tagged
references, no-op byte identity, changed/removed bookmarks, crop/rotation, no
annotation resurrection, fallback page/overlay correctness, protected save and
denial, cancellation and destination preservation. A fresh producer audit must
retain original Word catalog and page graphs, text and pixels. Run existing
save, protection, bookmarks, crop, annotation and representative text/form tests.
Create a runnable local build and exercise Save/Reopen in Windows. One scoped
final review follows implementation. Broad tagged reflow, tagged page combination,
overlay preservation on tagged sources and producer font/clipping fixes remain
future work; don't claim accessibility conformance or Adobe/Foxit parity.
