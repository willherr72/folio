# Text-field resize with native reflow

Continue existing field positioning by preserving font settings during text-field
resize. Standing user authorization permits autonomous implementation. The last
verified checkpoint scales saved AP artwork; this remains an explicit alternative.

Default ordinary text fields to Reflow with field settings. Multiline fields wrap;
single-line fields retain their single-line layout. Keep explicit font sizes, auto
font-size behavior, color, alignment, border, rotation, current value and shared
identity. Move-only edits retain exact saved APs. Checkbox/radio/choice positioning
retains the existing scale behavior in this cycle; choice reflow is subsequent work.
Preview and one-step Undo/Redo/cancel/source ownership remain unchanged.

Approaches considered: whole-document no-op filling does not regenerate APs (observed
probe.log failure); whole-document missing-AP regeneration works but may touch other
widgets and lose a font supplied only by local AP resources. Selected approach:
construct a bounded one-field native probe with effective text settings and the
source-bound font; remove only its AP, let native filling of its unchanged value
generate the new AP, then graft only that AP into the original geometry transaction.
Original field dictionaries, sibling APs, actions and global DR are untouched.

Add optional reflowText boolean (default false for old callers). Enable only for
text widgets and only regenerate when width/height changes. Read-only source flags
remain untouched; the probe can clear its private read-only bit to trigger native
generation. Rich-text/file-selection fields retain an explicit refusal. Ordinary
source positioning admission, protected permissions and alias checks stay intact.

Resolve inherited DA, Ff, Q, MaxLen and V; resolve DA from the AcroForm if absent.
DA must have one unambiguous valid Tf. Prefer the named original normal-AP font,
then AcroForm DR, matching the existing filling map; do not silently substitute a
page/field resource or a platform font. Probe keeps original MK, Border/BS, Rect,
effective field properties and all needed font resources. It is one synthetic page
and widget; no source page artwork, siblings, actions or other form fields enter it.
Bound graph copies to 16 MiB/100000 direct values and depth64; reject page-tree
references. Compare generated font graphs with their source bindings under a bounded
comparison before grafting. Unsupported or ambiguous fonts refuse reflow and leave
Scale saved appearance available. Preserve source bytes and no partial publication.

Regenerated normal AP must have valid geometry, preserve complete current value,
and render at the new size without scaling old glyph shapes. Font resources must be
source-equivalent; generated fallback fonts may be accepted only when equivalent
to an original appearance font. Graft normal AP and its bounded resource graph,
preserve other AP entries, then prune unreachable replaced resources so repeated
edits do not accumulate graphs. Preserve all field metadata and sibling APs.

Tests: multiline resize/single-line/empty/read-only/auto-size; local font versus
conflicting global font; rotation/UserUnit; duplicate/shared identity and sibling
appearance equality; repeated resize and graph bounds; no-op move; stale/invalid
requests; desktop default/explicit scale/preview/Undo/save/reopen. Independently
verify glyph size, line positions and unchanged surroundings. One final reviewer.

Reference: PDFium widget appearance lifecycle source was inspected at
https://pdfium.googlesource.com/pdfium/+/refs/heads/main/fpdfsdk/cpdfsdk_widget.cpp.
An attempted default-appearance-source fetch failed; actual bundled renderer probes
are authoritative for this implementation. No generic renderer/parity claim.

Review corrections: native DA lookup does not evaluate competing color operators
in order. Canonicalize only the private probe DA to its single Tf and final supported
DeviceGray/RGB/CMYK color; reject other text-state operators. Original DA stays exact.
Native border generation truncates fractional widths and replaces custom dashes.
Reflow therefore admits solid integer-width borders only, with zero corner radii,
no dash array, and consistent BS/Border widths when both exist. Other borders retain
Scale saved appearance. Opaque resource edges with structural field/widget/action
markers are refused, including untyped nodes; budgets alone are not isolation.
