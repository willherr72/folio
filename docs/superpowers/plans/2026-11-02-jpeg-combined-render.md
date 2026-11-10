# Canonical JPEG redaction rendering and opaque paint

Continue phase3 of `2026-11-02-jpeg-area-redaction.md` inline. Its design remains
the authority. Preserve accepted v0.30, original documents, predecessor evidence
and ordinary JPEG=false. No per-helper review; one fresh integrated phase3 review
remains required after publication acceptance.

- Bind canonical page dimensions to selected occurrences. Derive actual PDFium
  viewport and MuPDF interpreter matrices with native arithmetic/order; retain
  caller-owned actual bitmap dimensions. Join decoder influence with bounded
  opaque polygon paint influence without changing the source MCU mask.
- Export actual parser/editor/publisher transactions and source-derived affected
  areas at multiple bitmap sizes, including nested transforms and partial edges.
  Verify both readers' outside pixels and source-bound coefficient preservation.
- Verify opaque coverage against an independent solid-black reference derived
  from the original MCU selection. Require black fully covered pixels, no shared
  seams and boundary coverage at least that reference. Keep missing/reordered
  fill and cover-only controls effective. Save/reopen/re-edit retains prior masks.
- Correct demonstrated failures, run serial native regression and independently
  verify saved evidence. Record remaining host/UI acceptance without declaring
  public JPEG admission or commercial parity.

The current native viewport is origin zero, unrotated, with canonical MediaBox,
matching CropBox and UserUnit=1, as already enforced by private area admission.
This does not add a new restriction on previously admitted area pages.

The combined audit reproduced two product defects: an extended rectangle can
repaint the previous edge through native rounding, and PDFium's rectangle fill
shortcut can trim fractional geometric coverage. The correction accounts for
old/new paint footprints and publishes a single nonzero compound triangle fill.
Coverage version 2 binds that grammar; untouched version 1 occurrences retain
their existing grammar until explicitly selected for an edit.

Remaining phase3 acceptance after this checkpoint:

- Extend the ordinary-viewer corpus to explicit JPEG color overrides, selected
  partial final MCUs, disjoint masks/holes, and warmed caches. Current combined
  opacity references independently form one/two adjacent cells' rectangular
  union; partial-size source files alone do not prove selected final-edge fills.
- Reconcile the previous MuPDF direct-device evidence's non-null separation
  target with the new ordinary-viewer null-separation target. Preserve the old
  evidence as proofing-mode evidence; do not relabel its viewing configuration.
- Complete one fresh integrated phase3 review, then connect the combined
  preview/apply host and UI and run Windows acceptance. Public JPEG admission
  remains disabled until those gates; accepted v0.30 remains unchanged.
