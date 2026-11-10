# Searchable PDF OCR workflow

Continue the active everyday Adobe/Foxit usefulness goal with a complete local OCR
workflow, using the reviewed engine in `src/editor/ocr`. User authorization permits
design and implementation without another approval. Prior recognition work is progress;
it does not constitute a usable PDF OCR feature by itself.

## User outcome

Choose **Recognize text…** for the current page, a range, or all pages, choose the
languages present (English default; French/Spanish available), and recognize locally.
Show page progress and allow cancellation. Review the recognized words on the scan,
focus low-confidence words, correct or remove individual words, and Apply once.
Confidence is a score, not a probability. Applying changes searchable/copyable text;
the scanned image's visible lettering stays unchanged. State this beside corrections.
Do not expose invisible OCR text as editable visible ink through existing text editing.

Blank pages and pages with no new scan text are reported without dirtying the document.
Recognize mixed pages: retain original native text and omit recognized words when
either the word center falls in a native character box or a native character center
falls in the word box (0.5 raster pixel tolerance). The reciprocal check covers words
whose centers fall between letters, as demonstrated by the user's quiz.
The preview must show which words are excluded; Apply repeats the check natively.
Do not discard a scanned body merely because its page also has a native header.

An existing verified Folio OCR layer can be reviewed without recognition and replaced
without duplication. Correction-only review retains the layer's original raster axes,
even after the workspace or exported PDF is rotated. The update explicitly requests
`preserveOwnedOrientation`; native validation requires ordered surviving word boxes
from the proven layer before applying new text/removals in its original mapping.
Fresh recognition uses the current displayed orientation. Unrecognized or externally modified OCR remains ordinary native
text; never delete it on the strength of a private metadata label alone.
Copies of the same original page are updated together, explicitly explained in the
dialog. Native operations create full immutable source versions, preserving shared
form identity; Apply rebinds all occurrences of each changed source in one history step.
Unselected source pages have unchanged contents. Undo/redo, recovery and encrypted
save/reopen follow existing source ownership and protection rules.

## Native contracts and authority

`prepare_ocr_page(sourceId,pageIndex,rotation)` requires native edit-content AND
copy-content rights and rejects prepared print sources, signed documents and XFA.
`rotation` is the workspace's extra quarter-turn0/90/180/270. Return PNG plus dimensions,
effective DPI, native text exclusion boxes and any verified owned OCR record. Render
source contents at nominal300DPI, scaling down to6000pixels per side and20million pixels.
Exclude PDF annotations and interactive form appearances; do not OCR added editor
overlays. The UI shows them as existing content outside recognition. PNG <=32MiB.
The recognition DPI hint is effectiveDpi clamped to72–600; geometry uses actual pixels,
never that hint. One raster is retained at a time; review can re-render the active page.

`apply_ocr(sourceId,updates)` repeats authority, geometry, text and container validation,
then produces a new full `DocumentInfo`. Each update identifies pageIndex, workspace
rotation and `OcrPage` (same ordered blocks/paragraphs/lines/words as the engine).
At most100 distinct source pages per invocation,20,000words/100,000scalars per page,
and500,000words/2,000,000scalars per batch. Corrections may remove words; an empty
record removes only a verified owned layer. Validate finite positive boxes inside
the raster and finite0–100 confidence; reject controls and unsupported malformed data.
Page indices, pixel dimensions and intrinsic/extra rotations must match preparation.

Native work is serialized with the existing PDF engine. Cancel prevents adoption and
closes any late source; bounded native Apply may finish after dismissal. No original
source, file, history or global font registration is changed before publication.
Dropped native reply receivers reclaim candidate source handles. Multi-source Apply
is atomic in the workspace: if any source fails, dispose all candidates and retain
the review draft. Reject stale pages/tab state; close all unadopted candidates.

## Searchable layer and replacement proof

Use one invisible Form XObject per page, with a private font resource and a versioned
`FolioOCR` record. A real export probe proves PDFium preserves metadata on this Form
through Folio export (`artifacts/ocr-probe/transport.log`). The form contains only
deterministic rendering-mode3 text. Use a CIDType2 rectangle semantic font with one
ToUnicode scalar per CID, one text sequence per line, explicit spaces and bounded TJ
position corrections. Glyph bounds track recognized word boxes; uniform subdivisions
within a word are selection cells, not inferred typographic caret positions.

Reuse the semantic-font builder through a separate OCR entry point allowing65,534
glyph definitions and a format12 private-use cmap; preserve its existing4096-definition
shaped-text contract. Deduplicate scalar/metrics definitions. Reject font/content
capacity failures before changing the source. Unicode, accent and supplementary-plane
text must round-trip through independent readers.

Before treating an existing Form as owned, deserialize a bounded record, regenerate
its deterministic content/font/resources, and compare the complete relevant object
graph with bounded decompression. Hash or marker presence alone is insufficient.
Unknown/modified records are opaque and are not removed. Reject ambiguity from multiple
owned invocations or transforms that no longer match the record rather than deleting
unrelated contents. A retained record stores the original PDF-space mapping, so page
rotation on later export does not invalidate the proof. Transform review boxes into
the current displayed coordinate system.

Clone page resources before changes. Keep original content streams and image bytes;
wrap balanced original graphics state once and add a small Do invocation. Replacement
changes only the verified OCR Form reference/content. Check bounded decoded page
content (16MiB,1million operations) for balanced q/Q and BT/ET before wrapping. Never
inherit an original clip/CTM into the OCR layer. Prune unreachable objects only after
the private document mutation; retain every reachable original resource, annotation,
bookmark and shared AcroForm definition.

Before publishing, reopen the candidate, compare page dimensions and pixel output
for every updated page at a bounded verification resolution, and verify extracted
recognized text/word bounds where the native reader supports it. This is a runtime
guard, complemented by independent MuPDF/pypdf/rawPDFium tests. There is no universal
reader or whole-process memory claim from passing fixtures.

## Fidelity and release gates

Exercise clean/skew/blank, multi-column reading order, native headers over scanned
bodies, inherited/shared resources, nonzero CropBox origin, all intrinsic and workspace
quarter-turns, repeated corrections, damaged private metadata/font/invocation, shared
form fields, annotations/bookmarks, restricted-reader/owner permissions, protected
recovery/export and stale/canceled workflows. Pixel comparisons include every visible
scan pixel; extraction checks include original native text and corrected OCR words.
Keep source file hashes unchanged. Verify actual desktop scan→review→apply→Find→save→
reopen and undo/recovery, with external networking denied for recognition. One fresh
whole-workflow review follows implementation; fix material findings before packaging
a new version. Keep portable0.14.0-dev.1 intact. Overall parity is still unproven.
