# Physical point boundary for non-default PDF UserUnit

Evidence: the inherited-paragraph diagnostic confirms wrong reported dimensions
for U=0.5/2. Independent MuPDF/pypdf plain-export checks across all rotations show
that current PDFium page copying/export preserves UserUnit, physical dimensions,
and exact native content pixels. Source bytes remain immutable. The remaining
editing pipeline is being exercised beyond its initial dimension assertion.

The first architecture review prefers canonical PDF normalization, but identifies
that wrapping content would invalidate existing paragraph identity matrices,
portable appearance proofs and owned OCR. Re-emitting all of those, plus form DA,
shared widget appearances and destinations, introduces unrelated content rewrites.
Given observed unit preservation, the coordinator's preferred next approach is an
explicit physical-point API boundary around the existing raw-unit engine.

Keep source bytes, native PDF geometry, private PDF records, font proofs, bookmarks
and worker algorithms in their original PDF units. Expose displayed physical points
to the editor. Convert dimensional public responses by U and inverse-convert
dimensional requests by U using source-owned, validated page metadata. Never trust
a caller-supplied scale. Raw PDF origins explicitly documented as raw stay raw.
Ratios, angles, colors, strings, indices, pixels and font identities are unscaled.

The conversion must cover **every** relevant API, not just PageInfo:
- document/open/password/unlock/generated/recovery results and imported overlays;
- text/search/form/comment hit regions, font sizes and paragraph selections;
- paragraph bounds and requested sizes; raw layout origins retain their contract;
- all export/preview/print PagePlan dimensions and text/image/ink/highlight/comment
  overlay lengths, including text-area insets and stroke widths;
- new source results from all editing operations;
- OCR physical DPI/raster allocation and print permission DPI caps, while OCR's
  pixel data and raw PDF mapping remain in their existing domains;
- recovery format versioning/migration for old raw-unit workspace coordinates.

Permission and source policy baselines remain native raw data and requests must
reach authorization in that same domain. No normalization of signed bytes or
changes to cryptographic ownership are introduced. Current physical-size limits
must be checked in physical units; raw font/area bounds cannot silently redefine
those limits for scaled pages. Conversion failure must roll back source admission.

This choice remains subject to review and full acceptance. A page-size-only patch
or passing unit-1 tests does not satisfy it. Required coverage includes preserved
UserUnit through every standalone edit/source path, indirect and mixed units,
reorder/duplicate/crop/rotation, physical glyph sizes/positions, annotation/form/
OCR semantics, old/new recovery, protection and independent rendering/printing.
Implementation is now in progress at the public API boundary; this specification
is not an acceptance record. See the inherited-paragraph-pages progress ledger.
