# Combined paragraph canvas and recovery acceptance

Exercise the current baseline-anchor candidate through an actual Windows editing
session. The immediate quality gap is missing evidence that attributed text,
highlights, on-page resizing, document history and recovery compose correctly.
This is an acceptance milestone, not a claim of overall parity.

Use the existing mixed, highlighted, linked-field PDF fixture and isolated
WebView2 workflow. Edit only the highlighted range, expand the paragraph frame
with keyboard canvas handles, preview/cancel, then apply. Verify exact text and
raster Undo/Redo. Capture a durable unsaved checkpoint, stop only the owned test
process, and restore from a copied checkpoint after moving the test input copy.
Save, reopen, and inspect the recovered paragraph again.

An independent MuPDF/pypdf audit must verify selected text, styles, baseline,
highlight paint, untouched pages and pixels outside the paragraph, linked fields,
links and bookmarks. Require exact replay pixels and a negative control for the
comparison. Preserve originals, package artifacts and failures. No new native
build is needed unless a product defect is reproduced.

Approaches considered: model-only tests are quick but cannot prove native saved
output or recovery; a fresh full native suite does not exercise UI composition.
Use the existing candidate and desktop acceptance first, then add targeted
regression coverage for any defect. Real producer coverage and measured resource
budgets remain subsequent tasks, not implied by this synthetic fixture.
