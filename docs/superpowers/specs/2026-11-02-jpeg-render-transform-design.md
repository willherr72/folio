# Separate binary32 transform bounds

Continue the private JPEG disclosure phase. Preserve existing source selection
and binary64 ImageTransform admission. Introduce a separate renderer arithmetic
type; do not silently install it as proof of the original PDF operands.

Evidence changes the integration boundary: MuPDF's PDF lexer has an alternate
numeric path for a long integer spelling before a decimal point. A valid
56-byte token with leading zeros and a long fraction becomes NaN in the actual
lexer while normal conversion returns a finite value. The current structural
preflight accepts that token's grammar. Therefore taking lopdf's f32 as proof
of both readers' original operand is unjustified. Preserve the reproduction;
operand correspondence or token-preserving publication remains a required
integration step, not a reason to relax the renderer envelope.

Implement RendererTransform for known binary32 coefficient intervals. Each
interval contains exact real arithmetic and binary32 rounded intermediates,
including possible fused evaluation and subnormal flushing. Endpoints are
binary32 values stored in binary64. Their products are exact in binary64;
TwoSum residuals direct addition endpoints before outward binary32 bracketing.
Include zero for subnormal ranges. Reject nonfinite or unbounded arithmetic.
Bound all intermediate endpoints by2^60; no inversion or determinant admission
is provided by this arithmetic type.

Expose exact-known constructors, interval coefficient bounds, preconcatenation
and forward image-unit rectangle bounds. Identity remains exact. Source bounds,
removal decisions, and existing saved output must remain unchanged. Callers
must prove operand intervals before using these results for source disclosure.

Verify red/green tests for cancellation, accumulating rotations/shears, small
values, invalid inputs and identity. Export actual Rust intervals with serialized
known f32 operand chains. Compare the actual PDFium parsed image-object matrices
and native MuPDF fz_concat/point operations against the intervals. Retain an
exact-binary64-only negative control. Run serial native regression and preserve
all prior output artifacts.

This is arithmetic foundation, not complete placement. Original numeric-token
correspondence, sizing axis lower bounds, grid fit, inverse/fixed-point affine
support and opaque-paint union remain open before integrated phase review and
public preview/apply/Windows acceptance.
