# PDFium affine bilinear-stage influence

Continue the JPEG placement proof with PDFium's general-transform sampling stage.
Input is the actual clipped stretched bitmap's SampleRegions, the proven
result-local-to-stretched-local CalcData matrix, and its integer result rectangle.
These are post-stretcher inputs. The page matrix, decoded JPEG grid or requested
crop cannot substitute. Normal/quarter-turn stretch paths remain separate.

Enclose FXSYS_roundf(matrix*256), refusing nonfinite/saturated coefficients or
singular quantized maps. Local destination coordinates are nonnegative integers
below 2^24. Bound each floating multiply/add over the result rectangle, requiring
every resulting coordinate to stay within int32 because the native remainder
conversion is not saturated. Derive a uniform roundoff envelope: integer-valued
operations below 2^24 are exact; larger operations use a conservative full ULP
for each multiply/add. Invert the quantized linear map and expand source support
by these errors before returning complete destination pixel rectangles.

Native source indices truncate toward zero, admit [0,dimension] inclusive and
clamp the last index. Bilinear reads that index and its clamped successor. A source
interval [L,R) therefore permits index max(0,L-1) through R-1, extended to dimension
when R reaches the final edge. Index zero includes negative fractional coordinates.
Use inclusive integer-valued fixed-coordinate endpoints and retain exact overflow
refusals; do not use ideal floor-coordinate behavior or a guessed fringe.

Source dimensions <=32768, result axes <=2^24 and at most 4096 regions keep work
bounded independent of output pixel count. Require four-byte bitmap addressing
to fit native signed row offsets. This is renderer-only disclosure evidence,
never authorization to expand JPEG source selection.

Verification: red/green unit and exported interval cases; build a pinned-source
native probe for the exact bilinear matrix/rounding/interpolation helpers with
the pinned compiler and float contraction disabled. Persist before/after pixels
and reverify byte containment independently. Explicitly label this source-derived
probe, not an actual installed-DLL render or completed page pipeline. Full
PDFium stretcher/branch/device integration, JPEG publication, opaque paint union,
final phase3 review and public Windows acceptance remain required.
