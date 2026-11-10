# Physical-pixel page presentation

User goal: reliable everyday PDF editing with bounded sustained resources. The
user has delegated implementation choices and opted out of approval questions.
Previous goal turn made progress by isolating focus-sensitive SVG reference
output and rejecting an apparent rendering improvement.

The existing PageView renders a native PNG inside SVG; a reusable foreignObject
canvas prototype cannot reliably reproduce the focus-sensitive filtering of that
SVG image. Alternatives are further empirical SVG filter tuning, a physical-grid
HTML canvas below SVG controls, or a new native viewport renderer. Select the
HTML canvas experiment: it can preserve the existing native decode and document
coordinates while removing the second SVG image scaling boundary. A native
viewport API is a larger fallback, not currently justified.

The display contract separates original PNG data, derived display pixels and PDF
document fidelity. Original opaque decoded pixels remain exact. A derived canvas
is intentionally resampled for the current projection and composited on white
paper, matching PageView's existing white stage. Its interior displayed pixels
must match the derived buffer, independently of input focus. The outer fractional
clip boundary may be excluded by at most one physical pixel on each side, computed
from viewport geometry before looking at output. This is not acceptance of arbitrary
PDF-content differences. Rotation/crop/projected marker geometry, source/edited
distinction, downscale readability and original-document preservation still need
separate checks. Historical SVG comparisons stay failed and retained.

First run an isolated spike with the existing four preserved fixtures at DPR1
and1.25, fractional offsets, quarter-turns/crops and focused/blurred input. Paint
once into a physical-grid backing surface; place it as an HTML sibling underneath
SVG controls with no SVG image rescale. Capture the raw derived buffer and screen.
Check exact interior equality and focus stability with fixed geometry bounds and
negative controls. Do not claim Windows, memory or PDF parity from this spike.

If the display boundary is verified, implement bounded projection/reuse in product
code with serial decoding, retained source ownership, generation-safe readiness,
fallback, abort/dispose, scroll/resize/DPR updates and interaction tests. Then
integrate PageView and measure actual Windows fidelity and sustained resources.
If the spike fails, preserve its counterexample and investigate the specific
boundary before integration. Never relax a passing requirement to fit a result.

## Measured placement revision

The local-origin implementation failed at several fractional phases despite an
exact reported box. Screen-origin fixed positioning passed the same cases. Use
that placement with explicit intersection of the page, window, and scroll-pane
client rectangles. Present only complete physical pixels within that intersection;
this stays within the original one-pixel fractional-edge exclusion. Reject
unsupported fixed-containing-block, shaped-clip, and visual-viewport contexts to
the existing SVG path. Scroll/resize/DPR updates retain one decoded source and
reuse the display surface. Full Windows interaction and resource acceptance remain
mandatory; the browser component proof alone does not authorize a release.
