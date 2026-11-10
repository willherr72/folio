# JPEG source envelope across aligned crop and reduction choices

Continue phase 3 under standing autonomous authorization. The previous cropped
tile helper works for an actual window. The combined policy cannot assume that
one request describes the cached tile, so it needs a source-derived envelope
covering every admitted aligned window at a given decode/reduction pair.

Let the JPEG decode factor be D (1/2/4/8), extra box factor f (power of two),
and total factor T=D*f<=64. The actual source crop begins on T-aligned original
coordinates and ends at a T boundary or the original image edge. Full-image
tiles are included. Decoder support [l,r) is in the full decoded grid and
already includes effective chroma taps. A crop starts at decoded index q,
which is f-aligned. Cropping can only contract [l,r).

The fixed-group stream subsampler uses [floor((l-q)/f),ceil((r-q)/f)). The
partial-edge overlapping pixmap subsampler is contained by the same left edge
and right edge ceil((r-q+f-1)/f). The previous exact crop proof then places
each mapped output cell edge at nominal original coordinate k*T minus a
deficit between zero and T-1. Thus every affected source-coordinate cell lies
inside:

    [floor(l/f)*T - (T-1), ceil((r+f-1)/f)*T)

with clipping to the original image extent. Apply independently on both axes.
This is an envelope for the decoder and extra reduction only, before downstream
stretch/affine sampling. It includes fixed and overlapping groups without
choosing a renderer path from observed pixels. It never changes the removal mask.

## Cache-union simplification

With a conservative one-decoded-cell halo, MCU rectangle starts L are multiples
of every admitted D. Away from image clipping, the lower endpoint above is
`(ceil(L/T)-1)*T-(T-1)` and the upper is `(ceil(R/T)+1)*T`. An image-edge upper
endpoint always clips to that edge. These expressions are independent of how T
is split into D and f. As a power-of-two T grows, the lower bound cannot increase
and the upper cannot decrease: the coarser upward quantization adds at most
T-t on the lower side, while its subtraction grows by 2*(T-t). Clipping preserves
that containment. Therefore one maximum-T envelope covers every supported
higher-resolution cache factor, using D=min(T,8), f=T/D, and halo[1,1].
Coverage::source_envelope requires an explicit validated maximum; it does not
infer the maximum from page geometry or install a default.

Implement a consuming SampleRegions::source_envelope(source,D,f), requiring
the incoming dimensions to equal ceil(source/D), validating all dimensions and
factors even when no regions exist. Return integer original-coordinate cells.
Coverage::decoded_source_envelope composes the existing full-grid decoded
support and this operator. Policy callers must choose source-proved candidate
factor pairs; no maximum-factor default is supplied.

Verification: observed failing examples, exhaustive exact small-grid crop/box
enumeration, existing actual cropped tile evidence, and actual MuPDF compressed
JPEG decoding with total factors through64 on the owned large fixture. Save
native-exported envelopes independently of pixels; reject deliberately narrowed
and outside-cell controls. Preserve all older decoder/corpus artifacts.

Next composition: downstream source/destination sampling pitch, renderer factor
choices, binary32 matrices, grid fit, affine precision, and opaque paint. Final
phase review and public preview/apply/Windows acceptance remain open.

Evidence: artifacts/jpeg-source-envelope-20261102/checkpoint.json. Native242
passes with one explicit corpus skip; standalone115 passes.1,329,366 exact
crop/box chains and606 actual MuPDF JPEG comparisons validate the implementation,
including total reduction64. Prior decoder and saved-output artifacts match.

The derived next stretch term is max(Ps,Pd)+(Ps+Pd)/2 in source coordinates:
the sample-centre filter window plus one source-sample arithmetic allowance and
half of a destination-cell extent on each side. Bounding Ps by T gives
max(T,Pd)+(T+Pd)/2. Exact comparison with1344 prior native full-grid scaling
configurations passes and rejects a missing stretch term. This still needs a
renderer-proved upper bound on Pd for cropped/fractional paths; no product
integration or full page-raster claim is made for that term yet.
