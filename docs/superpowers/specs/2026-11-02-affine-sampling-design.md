# MuPDF affine sample influence

Continue the JPEG phase3 placement proof with a renderer-only support function.
The input is the actual post-decode/post-scale pixmap's SampleRegions, its proven
unit-image-to-device RendererTransform and the effective integer scissor after
destination, shape and group-alpha intersections. The caller must establish this
stage identity; requested crops or pre-scale grids cannot substitute for it.

Derive native scan-origin intervals from the transformed unit rectangle, including
near-axis and near-quarter-turn two-corner branches. Intersect with the effective
scissor. Bounds are finite and limited to +/-2^24 device coordinates; invalid or
unsafe arithmetic refuses. Empty sample regions do not bypass validation.

Pre-scale the matrix by reciprocal actual pixmap dimensions and use the proved
reader inverse envelope. Model MuPDF's 14-bit fixed-point coefficients, truncation
of initial float coordinates to int32, and subsequent exact int64 increments.
Check all increments including loop-final increments and interpolation additions
for overflow. Source dimensions remain 1..32768 and at most 4096 rectangles.

For nearest and bilinear sampling together, each selected source-axis interval
[L,R) can influence initial fixed coordinates in [L*16384-8192,R*16384+8192),
clamped to [0,source_dimension*16384). This follows from the bilinear half-step,
two taps, clamped edge samples, and the paint acceptance condition. It does not
authorize changing those neighbors in the JPEG coefficients.

Invert the bounded linear fixed-step map, enclosing all real inverses and the
integer source-support endpoints. Translate from local scan offsets to complete
device pixel rectangles and intersect the scanned extent. Singular or ambiguous
fixed-step maps refuse; such cases can be valid rendering and remain compatibility
debt. Work is O(number of source rectangles), independent of rendered pixel count.

Verify red/green tests for identity, reflection, clipping, interval uncertainty,
integer limits and singular steps. Export owned affine cases, exercise actual
MuPDF drawing with nearest and bilinear sampling without intervening scale changes,
persist complete before/after pixels and independently verify containment. Retain
native source/runtime hashes and all prior exports. Do not copy AGPL implementation
into product code. PDFium's distinct fixed-point path, device composition, opaque
paint union, final phase3 review and public JPEG acceptance remain open.
