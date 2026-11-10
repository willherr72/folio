# JPEG disclosure geometry and renderer support

Continuation of the accepted JPEG area-redaction design. The existing 218-test
occurrence checkpoint remains the starting point. This work does not enable
JPEG page admission or alter which coefficients are removed.

## Separate the three quantities

1. The selected MCU mask determines data removal and opaque paint. It is stable
   across preview zoom and never accepts a disclosure region as another mark.
2. Decoder and separable-filter support expands a source region in image-unit
   coordinates. Clip this expansion to the image **before** transforming it.
3. Pixel-grid placement, affine sampling and its numerical error can extend
   the displayed result in page coordinates. Apply that margin **after** the
   source transform; do not clip it back to the image's mathematical outline.

The geometry primitive will take explicit nonnegative image-unit and page-space
margins, use the existing outward interval arithmetic, and retain disjoint
regions. There is deliberately no default margin and no claim that arbitrary
caller-supplied margins establish fidelity. The renderer-policy derivation is
a separate required gate before public use. This is a coordinator implementation
choice under the standing authorization to continue autonomously.

## Source investigation

The pinned local PDFium build uses AGG (the compiled page target defines
`PDF_USE_AGG`; no Skia definition). `CFX_ImageTransformer` uses axis/quarter-turn
shortcuts, otherwise a separable stretch followed by four-tap bilinear sampling.
The latter quantizes inverse coefficients to 1/256. Coefficient rounding alone
contributes at most `(local_x + local_y + 1) / 512` intermediate pixels per axis;
this is only one error term, not a final bound. Float inversion, coordinate
evaluation, stretch support, decoder support and grid placement remain separate.
The local JPEG decoder caps reduction at 1/8 and disables it for partial MCUs.

MuPDF 1.27.1 first grid-fits rectilinear images, can perform JPEG reduction and
additional box subsampling (total factor at most 64), then separable scaling and
affine sampling. Its affine increment precision is 1/16384. The scale filter's
source window is `filter_width / min(1, destination_size / source_size)`.
Thus neither a fixed source-pixel fringe nor an output-pixel-centre test describes
the full dependency chain. Cached higher-resolution decode paths must also fit
the eventual policy.

Reference-only official MuPDF files and URLs are pinned in
`artifacts/jpeg-disclosure-20261102/reference/mupdf-source.json`. No MuPDF
implementation is copied into the product. Exact local PDFium reference files
and compiled backend evidence will be pinned with this checkpoint.

The local libjpeg `jdsample.c` further narrows the decoder term: fancy 2:1
upsampling combines the current and adjacent component samples, affecting at
most one decoded output pixel beyond an MCU edge per axis. It is disabled when
the minimum scaled DCT size is one (1/8 decode); integer replication then has no
cross-MCU tap. For the allowed power-of-two reductions, the scalar decoder term
is therefore at most four original source pixels. Component-specific scaled
DCT sizes may remove an upsampling stage, never enlarge this bound. This is a
scalar-source derivation, not yet a verified policy for both shipped readers:
the SIMD/reduced-decode/partial-edge output corpus and MuPDF's subsequent box
subsampling remain required. The relevant scalar and x64 SIMD files are pinned.

Decoder continuation clarification: the four-original-pixel scalar tap term is
not by itself a page-unit bound for partial images. A width65 image reduced by8
has9 decoded columns; decoded edge8 maps to source coordinate520/9 when that
bitmap fills the image unit square, whereas the nominal original-grid edge is64.
The discrepancy is56/9, despite fancy upsampling being disabled at this reduction.
Map decoded cell edges by the **actual decoded dimensions**, then account for
the reader's subsequent transform/filter stages. Do not simply divide source
MCU edges plus a scalar fringe by the original width. Exact grid checks cover all
source dimensions1..32768 and reductions1,2,4,8 in
`artifacts/jpeg-decoder-support-20261102/decoded-grid.json`. PDFium's partial-edge
reduction refusal avoids this particular decoder-grid change; it does not prove
the independent reader has the same policy.

`Coverage::decoded_unit_regions` now performs that conditional full-bitmap grid
mapping with explicit reduction and zero/one decoded-pixel tap support. It
returns image-unit regions only, does not publish paint or change selection, and
charges the existing shared rectangle/cell budget. Source-derived exact checks
and actual scalar decoder outputs verify it. It is not a decoder-choice policy:
the caller must establish the reduction/taps, account for cropped decode tiles,
include the separately serialized opaque paint, and propagate downstream support.

`check-quantization-support.py` checks 99,225 rational combinations for each
fixed-point rounding rule, including ties and negative coefficients. Both obey
the triangle-inequality term above and both reject a constant-one-intermediate-
pixel control at large coordinates. It deliberately excludes float operations,
decode/filter support and grid fitting; it cannot authorize an overall margin.

## Required verification

- Observe a failing geometry regression before implementation: separate source
  clipping and page-space expansion, including reflected/sheared transforms.
- Verify nonnegative finite margins, geometry limits, disjoint paint regions,
  partial-edge geometry, zero-margin equivalence and unchanged mask/paint bytes.
- Verify all generated prior transaction/corpus PDFs remain identical.
- Before policy admission, derive support for decoder reduction/chroma filters,
  both stretch paths, affine fixed/float precision, and grid fitting. Validate
  source-derived bounds with both readers and effective outside-region controls.
  The previously measured 2.541714-source-pixel halo is an observation, not a
  policy constant. Do not fit an exclusion mask to changed output pixels.
- Preserve the outer-antialias opacity limitation; the current tests prove full
  interior and adjacent-block seam opacity, not every boundary pixel.

The public preview/apply integration and the single final integrated phase review
remain open. Geometry plumbing alone must not close either gate.
