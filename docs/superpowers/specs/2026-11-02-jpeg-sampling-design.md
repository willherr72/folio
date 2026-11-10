# Conditional sampling support propagation

This extends the decoder-grid work; it does not enable public JPEG redaction or
replace the complete supported-reader disclosure requirement. Previous checkpoint:
`artifacts/jpeg-decoder-support-20261102/checkpoint.json` (verified3490 pins).

## Integer cell regions

Carry the decoded bitmap dimensions and a bounded list of top-origin integer
cell rectangles through sampling stages. Normalize to image units only after
the last such stage. Keep this analysis separate from the immutable MCU removal
mask and serialized opaque paint. Consume intermediate region sets rather than
copying unbounded state. At most4096 rectangles; dimensions at most2^24; no pixel
buffers are allocated by this geometry code. The original coverage budget pays
for source rectangle construction.

## Box subsampling

The pinned MuPDF1.27.1 `fz_subsample_pixmap` uses overlapping sample groups for
partial dimensions when both dimensions are at least the reduction factor. It
does not always use independent fixed groups. For source length S, power-of-two
factor f and N=ceil(S/f), let r=f*N-S. Every group j reads inside
`[j*f-r, (j+1)*f)`, with image-edge clipping. Fixed grouping is included in this
superset. Thus a changed source interval [L,R) can affect output cells in
`[floor(L/f), ceil((R+r)/f))`, clipped to [0,N). This is derived from the maximum
cumulative overlap r, not from observed changed pixels.

The17-to5 example with factor4 and changed cell13 needs output cells3 and4.
A naive fixed-group calculation omits cell4. Test against the installed MuPDF
Pixmap.shrink method, which calls this actual routine.

## Full-bitmap separable scaling

For a positive integer source length S<=32768 and destination length
1<=D<=2^24, the pinned PDFium box/nearest/bilinear and MuPDF simple-filter
paths have a source-centre support window bounded by `max(1,S/D)`.
PDFium's rounded residual end weight fits that window; MuPDF's window_fix only
contracts its interval and its zero-weight fallback chooses the central sample.

Allow one additional source sample for binary32 centre/window arithmetic. With
full integer grids anchored at zero, fewer than32 operations have magnitudes
bounded by2*S+2; gamma32*(2*32768+2)<0.126. One sample covers that uncertainty.
This argument excludes fractional origins, cropped source grids and matrix
inversion; those remain separate renderer-policy terms. The binary64 PDFium
weight calculation has smaller arithmetic error under these same bounds.

Let H=max(D,S)+D, i.e. the scaled filter window plus that numeric allowance.
For selected source sample cells [L,R), their centres span [L+1/2,R-1/2].
The conservative first and one-past-last destination sample indices are:

    first = ceil(((2*L+1)*D - 2*H - S) / (2*S))
    end   = floor(((2*R-1)*D + 2*H - S) / (2*S)) + 1

Clip to [0,D), using signed integer division. Every term fits i64 under the
declared bounds. This directly produces complete affected destination cells,
avoiding floating-point exclusion boundaries. Full zero-origin bitmap scaling
will be tested against actual PDFium and MuPDF routines, with outside-cell and
naive-overlap controls. Conservative extra cells are allowed; widening the bound
from a rendered difference is not.

## Integration and remaining gates

Refactor Coverage's decoded-cell construction to return the integer region set;
preserve decoded_unit_regions as an adapter with identical outward endpoints.
Add box/scale chaining and export actual geometry for independent checks.
No renderer choice is implied by these operators. Before public admission, the
policy still must account for crop/subarea sampling, actual grid-fit transforms,
affine interpolation and fixed/float matrix precision, and union opaque paint.
The final integrated review and public preview/apply/Windows acceptance remain
open. This design does not claim a general page-raster bound.

References: pinned local PDFium cstretchengine.cpp; official reference-only
MuPDF draw-scale-simple.c and pixmap.c under the sampling/disclosure artifacts.
No MuPDF implementation is copied into the product.

## Verification

The actual native operator/harness exports agree for all 116 artifacts. Prior
114 decoder artifacts remain identical after the integer-region refactor.
28 owned JPEG cases supply actual decoded buffers for 112 MuPDF downsampling
comparisons and 504 full-grid scaling comparisons across MuPDF and PDFium.
The 17x17 impulse rejects naive box grouping, and each reader's outside-cell
mutation control is rejected. These checks validate the conditional operators;
they do not establish complete JPEG page-pipeline support. Final native gate:
232 passed, zero failed, one explicit corpus skip. Evidence is bound in
artifacts/jpeg-sampling-support-20261102/checkpoint.json.
