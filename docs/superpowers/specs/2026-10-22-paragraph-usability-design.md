# Paragraph usability cycle

The user authorized autonomous product development toward professional PDF-editor
quality. This cycle makes existing uniform paragraph editing understandable and
reviewable directly on the page. Native eligibility and preservation checks remain
the authority for what can be edited.

Show a native rendering of the original page immediately in the paragraph dialog.
Overlay accessible text-piece targets synchronized with the checkbox selection.
After checking the selection, show the fixed paragraph area and current numeric
dimensions. During text editing the page targets are read-only, avoiding accidental
selection changes from clicking the preview. Explicit checkbox changes still select
a different paragraph and require checking it again.

Offer fit-width, 150%, 200%, and 300% preview zoom with scrolling. Once a verified
candidate exists, offer Original/Edited comparison; default to Edited. Zoom and
comparison never invalidate the candidate or re-run PDF mutation. Text/area/member
changes continue to invalidate it. Keep Preview and Apply accessible in the footer.

The page preview component owns only the original page's raster URL. It must revoke
late/unmounted blob URLs and never close the document source. The dialog continues
to own candidate source IDs until successful Apply. Source dimensions are supplied
by the App page plan, so targets use native displayed-page coordinates accurately.

Keep keyboard selection, accessible control names, responsive layout, errors and
cancel available. A source-render failure must not prevent checkbox selection or
native validation. Original and edited images must be clearly distinguished.

No backend schema change or new product dependency is needed. All prior source,
overflow, mixed-format and undo protections remain intact.
