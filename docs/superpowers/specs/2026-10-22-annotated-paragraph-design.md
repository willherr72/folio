# Paragraph editing with existing Folio annotations

Remove the blanket prohibition on paragraph editing when the current page has Folio
text, ink, highlights or comments. Preserve all overlays exactly in the workspace
and save/reopen. Original and edited previews must include them; showing only the
source would be misleading. Paragraph font/layout compatibility rules stay unchanged.

Add native render_page_plan(plan,width): validate a one-page export plan, export via
the existing editable PDF pipeline into a private temporary directory, reopen raw
bytes without importing/removing annotations, and rasterize. Return only PNG bytes;
no registered source is created and temporary files are deleted on success/failure.
This shares appearance/font behavior with actual saved PDFs. Width remains bounded
by the existing raster contract. Source documents and user files are never changed.

The paragraph dialog passes current overlays to original/candidate composition when
nonempty, keeps source axes (rotation zero) so targets and text bounds still align,
and preserves workspace rotation on Apply. Empty-overlay pages retain the existing
renderPage path. If a non-native adapter cannot compose overlays, refuse the annotated
workflow explicitly. A composition error retains draft, releases the candidate and
disables Apply. Annotated preview URLs follow existing ownership/late-cleanup rules.

Tests: native pixels show overlays and preserve source; rotation/dimensions, invalid
plans and export/reopen fidelity; paragraph edit with text/ink/highlight/comment
retains all overlays through undo/redo/save. Rendering failures cannot leak or apply
unreviewed sources. Browser/desktop acceptance and fresh review close the cycle.
