# Source-bound JPEG influence in PDFium

Connect immutable JPEG selection to the verified installed-AGG placement stage.
This advances JPEG area redaction, while preserving source coefficients outside
selected MCUs and keeping source removal separate from display-only influence.

Chosen approach: retain an outward native content matrix alongside the existing
nominal selection matrix during the admitted content traversal. Compose that
matrix with a caller-proved page-to-device transform. For the actual bitmap
device dimensions, enumerate every eligible power-of-two decoder/cache factor
from one through pdfium_maximum; partial MCU edges retain the no-reduction policy.
Expand selected MCU support by the proved one-decoded-cell chroma halo, then use
the complete signed/clipped AGG placement calculation. Union device rectangles.
Do not infer decoder size from ideal image extents or use disclosure as selection.

Use the already admitted original/canonical numeric correspondence; casting a
nominal accumulated matrix would lose native intermediate rounding. q/Q saves
both matrix representations. Raw-only documents retain their current path.
Bound aggregate rendering attempts, cell-stage work, and output rectangles in
CoverageBudget, charging work consumed by failed calls too. Each placement call
retains its existing 65536-unit/4096-output bound. The device clip must be inside
the actual device dimensions. No global renderer state is introduced.

Test decoder alternatives, partial edges, shared budget exhaustion, stack order,
source binding and immutable masks. Export actual rewritten baseline/progressive
Gray/RGB JPEGs, including subsampling and partial edges, and audit saved PDFs under
normal/rotated/reflected/clipped placement and cold/higher-resolution cache states.
Retain all source/edited pixels and independently verify saved input geometry and
payloads. This stage does not yet prove compound opaque paint, MuPDF's complete
tile/scale route, arbitrary zoom disclosure or public preview/apply acceptance.
Accepted v0.30 and public JPEG admission remain unchanged.
