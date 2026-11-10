# PDFium stretched-grid influence

Connect source SampleRegions to the actual clipped stretched bitmap consumed by
the general affine stage, also covering signed normal/quarter-turn stretches.
The caller supplies exact signed destination dimensions and its nonempty integer
clip within the absolute destination grid. These are native stage parameters,
not inferred page extents. Return SampleRegions in the clipped bitmap's row frame.

Model both the binary32 ratio used to choose source clipping and the binary64
ratio/evaluation used by weight tables with outward arithmetic. Union possible
nearest, bilinear and area candidate windows; weight normalization only removes
or adjusts weights within those candidate indices. Include native source-window
edge clamps and fallback reads. Source-window uncertainty is permitted only when
all possible admitted endpoints prove a valid nonempty native source window.
Refusals are conservative compatibility limits, not malformed-PDF judgments.

For each axis, evaluate candidate-index interval bounds as a monotone function
of source-ordered destination pixels, retaining actual negative-scale arithmetic.
Binary searches find possible intersections with each changed source rectangle.
If a changed region intersects a possible source-window endpoint, extend the
corresponding candidate test to include clamped/fallback weights. Convert back
to physical destination order, intersect the clip and subtract its origin.

Dimensions: source <=32768, absolute destination <=2^24, up to4096 regions.
O(regions * log(destination dimension)) work; no per-output-pixel production
iteration. Validate native integer/range constraints even for empty support.
Source removal remains unchanged.

Tests cover signed axes, partial clips, interpolation neighbors, edge fallback,
invalid/uncertain windows and owned exports. Exercise actual installed PDFium
rendering of raw RGB image pages with signed scale, device clips and smoothing
flags, retaining complete pixels and independent verification. This closes a
stretching stage, not all page placement: closest-rectangle/branch/affine frame
composition, combined JPEG decoding, opaque paint union, final phase3 review and
public acceptance remain required.
