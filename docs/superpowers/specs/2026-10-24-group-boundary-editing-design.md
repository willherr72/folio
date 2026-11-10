# Group editing across raster-sensitive boundaries

The preceding renderer checkpoint retains fractional widths correctly, but two
ReportLab split points cannot pass a pixel-identical intermediate merge even
though every glyph origin matches. The user needs to edit the phrase, not publish
that unchanged intermediate merge. This cycle improves that workflow without
changing the renderer or weakening preservation of surrounding content.

Coordinator choice under the user's standing autonomous authorization: retain
the existing native merge as a private editing representation. Preserve the
isolated original bytes separately and publish those for an unchanged replacement.
Require all existing resource, encoding, glyph, style and unselected-run proofs.
If the merge raster differs, permit it only inside the selected bounds (existing
three-pixel raster fringe), with the merged bounds matching the original union,
and no overlapping unselected text or non-text objects in that area. Exclusion
reserves three pixels for the selection and three for a neighbor's raster fringe,
with touching boundaries rejected. Exact-raster
merges retain their current admission. Changed replacements still pass existing
single-run font, crop, collision, exact text and outside-edit pixel validation.

Alternatives: changing renderer rasterization would affect every page and does
not follow from this local editing failure. A new stream-distribution writer
would preserve all split boundaries but needs separate handling for shortened
strings, following cursor state and repeated edits. Retaining unconditional
refusal leaves a demonstrated ordinary-text workflow unsupported. The selected
approach keeps the no-op output untouched and protects all content outside the
user's selected phrase.

Acceptance: all seven split points of the embedded ReportLab phrase inspect and
edit; unchanged replacement retains separate pieces and exact page pixels through
save/reopen. Changed replacement retains original embedded font, neighboring text,
page geometry, protection, comments and source bytes. Stale selection, unsupported
glyphs, bad spacing, overlaps and oversized output retain existing refusals. The
new raster fallback must refuse overlapping artwork/text. No generated spaces,
tagged PDFs, nested-form groups, mixed styles or new scripts are enabled.

Keep previous executable, renderer, review evidence and inputs intact. No external
publication or blanket parity claim. Scope is a small native admission/publication
change; the UI and public API remain unchanged.
