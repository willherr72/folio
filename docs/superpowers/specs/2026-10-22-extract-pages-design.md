# Extract pages

User authorized autonomous progress toward everyday professional PDF workflows.
Expose Extract pages in native Save options and Page actions. A modal offers current
page (default), all pages, or comma-separated 1-based ranges. Use current document
order, deduplicate overlaps, validate bounds and show selected page count. Include
unsaved rotations, additions and annotations exactly as an editable Save copy does.
Explain that page numbers refer to the current arrangement and the document stays
unchanged. The native save dialog chooses the destination; source overwrite guards
remain in force. A cancelled save keeps the range draft; an export error is shown
inside the dialog for retry. Success closes the modal and reports destination.

Use the existing export engine with a subset of PagePlans. Do not mutate history,
saved digest, selection or source ownership, even if all pages were selected. Block
workspace navigation and close during export. Demo stays a demo-plan workflow and
has no Extract PDF action. Share range parsing with Print to keep interpretation
consistent. No PDF schema, new native command or dependency is needed.

Acceptance: extracting reordered pages with unsaved overlays yields only those pages,
in order, editable on reopen; original source and workspace dirty state remain intact.
Verify cancellation, retry, keyboard modal behavior, invalid ranges and pending export
exclusion. Batch splitting and blank insertion are separate cycles.
