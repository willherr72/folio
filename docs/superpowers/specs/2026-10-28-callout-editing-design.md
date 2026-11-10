# Faithful editing inside hollow rectangular callouts

The representative-producer investigation reproduced twelve refused workflows:
Word, ReportLab and MuPDF, standard/embedded/tagged variants, both single-run and
paragraph insertion. The shared cause is full nontext object bounds treating a
stroked rectangle's empty interior as occupied. Existing source and saved-output
checks remain necessary; geometry permission alone never authorizes publication.

Use the existing Rust/PDFium engine and source-bound editing routes. User delegated
implementation choices and opted out of routine approval questions. Coordinator
choice: a common conservative occupied-region helper, used in single-run growth,
styled paragraph publication, styled automatic fitting and grouped raster
isolation. The GUI and CLI inherit the shared engine behavior.

Alternatives: blanket rectangle exemptions would permit crossing borders; raster
sampling alone would miss thin/hidden content and vary with resolution. Instead
prove one closed, unfilled, stroked rectangle from path segments and transformed
geometry, then reserve four border strips. Require finite nondegenerate matrix,
width and coordinates, exact alternating axis-aligned edges, a single subpath and
closed final edge. Unsupported/filled/compound/curved/degenerate geometry retains
the original whole-object bounds. Clipping may reduce occupied geometry but never
enlarges it; do not discard any clip or drawing operators.

Transform half stroke width conservatively by matrix row-vector norms. Include a
six-native-raster-pixel clearance (the editor's three-pixel change fringe plus the
neighbor's three-pixel fringe), bounded from page dimensions and the 1000x1600
verification raster. Hairlines use the same raster margin. Reserve full continuous
edges even for dashed strokes. Use the union of actual object bounds and centerline
geometry plus padding for outer extents; if no positive finite interior survives,
fall back to full bounds. Support rotation/reflection/nonuniform scale when the
transformed path is exactly axis aligned. Arbitrary rotated/skewed polygons remain
conservative. Do not change source paths or use generated replacements for borders.

Acceptance: failing native text-growth and styled fit/publication cases first;
save/reopen/re-edit; conservative legacy uniform refusal and group consistency; thick/dashed/hairline and
scaled/reflected cases; refusal when text reaches the padded border, an interior
object, a fill/compound/curved shape or degenerate geometry. Fresh producer runs must
exercise all twelve callout workflows against unchanged original exports and
independent audits. Exact outside pixels and font/text/links/untouched-page checks
remain. Run serial native suite, current frontend/build and owned Windows workflow,
then one final scoped review. Preserve failures, baseline and package 0.24. No
commit, publish, user-app restart or unrelated compatibility changes.

Discovered required integration: tagged Word callout paragraph publication reaches
an existing restriction on unchanged pagination artifacts. Retain selected-context
admission and tag ownership exactly. Allow only comparison of unselected marked
contexts as opaque serialized scopes, with known nonempty clipping and no clipping
text; compare every original show and exact marked events as before. This grants no
editing authority to headers, ActualText, or unknown marks. Changed surrounding
mark metadata/clip must still fail; test the distinction directly. This dependency
is needed for the original tagged Word callout acceptance, not a general expansion
of editable semantic content.

Final-review correction I1: the legacy uniform API appends replacement shows and
newly allowing bordered edits exposes changed extraction order. Retain its original
whole-object graphics refusal until source-position publication is implemented.
The current desktop/CLI styled route also handles uniformly formatted paragraphs
and retains the new capability. Regression coverage checks refusal without source
mutation and complete saved text order for the supported routes. Original failed
audits stay intact; corrected output audits are separate.
