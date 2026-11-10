# Conditional cropped JPEG sample support

Continue the accepted JPEG area-publication plan. Standing authorization is to
make implementation choices autonomously. This is part of the incomplete
renderer-bound policy, not permission to enable ordinary JPEG redaction.

The pinned MuPDF image.c first adjusts the requested source rectangle to the
decode alignment, clips the decoded stream, and only then performs any extra
box reduction. Its image matrix maps that tile over the adjusted original-image
rectangle. A cached full-image tile can instead be returned. The caller must
use the actual source window, not assume the requested window was honored.

For an explicitly established decoded bitmap and integer cell crop [L,T,R,B),
intersect each affected rectangle with the crop and translate it to a zero-origin
tile. Drop empty intersections, including edge-touching regions. Do not add a
filter halo here: decoder support must already be expanded before cropping.
The existing box/scale operators then operate on that tile's actual dimensions.

For original image dimensions W,H and actual original-coordinate source window
[X,Y,U,V), tile width w and affected horizontal cell edge x map to
`(X*w + x*(U-X))/(W*w)`. The top-origin vertical edge maps similarly, then is
inverted for bottom-origin image units. Use u64 exact products/sums (all factors
at most 2^24, so the products are at most 2^48), followed by outward f64 division.
This avoids substituting a nominal decoder denominator or a full-decoded-grid
ratio for the tile's actual geometry. This is ideal source-frame geometry;
binary32 matrix concatenation and final raster placement remain separate.

Coverage will expose an aligned original-coordinate decode-window adapter.
Denominators are 1/2/4/8; starts must align, ends must align or be actual image
edges; all coordinates are validated before coverage allocation. It derives
decoded cell crop coordinates and consumes the general crop operator. All
helpers are conditional on caller-established renderer choices and source frame.

Implementation/verification sequence:
1. Observe failing tests for crop intersection, invalid windows and partial-grid
   source mapping; implement bounded operators and the coverage adapter.
2. Export actual native geometry for owned JPEG fixtures, crop windows and decode
   scales. Exercise actual MuPDF compressed-image tile decoding, verify returned
   dimensions/matrices, then compare changed tile cells with exported bounds.
3. Test cache fallback and partial-edge mapping controls. Check previous JPEG and
   sampling artifacts remain unchanged; run the affected native regression.
4. Record evidence and limits. Continue fractional placement, grid-fit and affine
   precision work before final phase review and public preview/apply acceptance.

No new codec, pixel payload, removal mask, ownership record or package changes.

Evidence: artifacts/jpeg-crop-support-20261102/checkpoint.json. The actual native
gate passes 237 tests with one explicit corpus skip; the fast harness passes110.
438 crop configurations pass rational coordinate checks; 686 actual MuPDF tile
comparisons cover fresh decoding, unaligned request adjustment and full-image
cache fallback, including higher-resolution tiles. Native matrix comparisons
are exact for the tested axis-aligned source frames, not a general error bound.

The follow-on crop-union derivation in crop-envelope.json bounds all aligned
crop choices at one decoder factor by an ideal coordinate deficit at most D-1
source units. Extra sampling and renderer matrix/placement errors still need
composition before this can become a complete page-disclosure policy.
