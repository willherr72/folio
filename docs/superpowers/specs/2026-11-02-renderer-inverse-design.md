# Bounded Reader Inverse Transforms

The next affine prerequisite is a renderer-only inverse envelope. PDFium's
GetInverse uses binary32 determinant/division and translation numerators;
MuPDF's fz_invert_matrix uses binary64 determinant, reciprocal and products
before final binary32 conversion. Neither an ideal inverse nor only one native
path is enough for shared disclosure arithmetic.

Add RendererTransform::reader_inverse(). Evaluate both algebraic paths using
existing outward binary32 endpoint intervals, then union each result. These
intervals include exact and correctly rounded binary32/binary64 intermediate
values. Refuse determinant intervals touching MuPDF's +/-DBL_EPSILON band and
all nonfinite/out-of-budget intermediate values. Negative determinants divide
through negated numerator/denominator. Preserve existing source-selection
ImageTransform behavior; the new method is only renderer evidence.

Conservative refusals for nearly singular but individually invertible matrices
remain explicit compatibility debt. Do not infer that a refused PDF is invalid.
This step does not implement final fixed-point affine sampling or enable JPEGs.

Verification: failing unit tests for exact identity/scaling/reflection, differing
translation routes, singular/uncertain determinants and numeric limits. Export
owned known/interval matrices and admitted bounds. Compare with actual MuPDF,
exact rational inversion and a native probe compiled from the pinned PDFium
GetInverse method with the pinned compiler and contraction-disabled settings.
The probe is source-derived evidence, not a call to the installed DLL's private
method. Retain source/compiler/build/fixture hashes and full regression evidence.
