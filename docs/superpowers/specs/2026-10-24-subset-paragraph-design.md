# Source-aware embedded subset paragraphs

An embedded subset can encode every actual source character while lacking a
space inserted by PDFium's reading extraction. Individual editing handles this;
uniform and styled paragraph inspection still reject it. The failing native
regression is recorded in artifacts/subset-paragraph-20261024/red.log.

Reuse the bounded source operand mapper against an explicit page in the parsed
document. Paragraph probes may be multi-page, so retain the single-page wrapper
for individual edits and add a page-specific internal entry point. Recompute
only local eligibility from verified operands. Keep raw snapshots, selection
identities, neighbors, glyph positions and persisted memory bindings unchanged.

Publication and replay must use independently decoded output operands when
checking eligibility. Expected draft text alone is not proof. Keep resource,
font, transform, glyph, raster and source-immutability checks. Genuine missing
glyphs and unsafe appearance remain refused. No string matching on error reasons.

Verify both paragraph modes through inspection, suggestions, preview, export,
reopen and replay, with embedded font preservation and neighbor checks. Include
all page rotations and actual missing-space/unsafe-source negatives. Record
scoped review and final build/desktop evidence separately from automated tests.
Group editing, new encodings and general product parity remain outside this fix.
