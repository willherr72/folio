# PDFium image placement and influence

Continue the JPEG area-redaction design without changing source selection or
enabling public admission. The installed AGG renderer has two direct stretch
paths using GetOuterRect, plus CFX_ImageTransformer using GetClosestRect and a
clipped intermediate bitmap. This connection consumes changed decoded samples,
an interval image-unit-to-device matrix, and the actual effective device clip.
It returns device pixel rectangles containing changes for the supported reader.

Chosen approach: compose the existing signed stretch and fixed-point affine
bounds with all possible native placement branches. Enumerate bounded integer
rectangle/size alternatives rather than choosing nominal rounded parameters.
Retain interval arithmetic through inverse and cropped-frame translations.
Reject unbounded/singular arithmetic or an exceeded work budget. These are
compatibility refusals, never authority to enlarge source removal. A generic
geometric halo cannot represent these filter and clipping dependencies; rendering
every production request is not a substitute for the pure publisher contract.

AGG normal placement uses signed outer rectangle sizes; quarter turns swap the
clip and swap/flip stretched cells. General placement enumerates closest-rectangle
starts/lengths, ceil(hypotf) stretch sizes, inverse-transformed outer clips, then
the result-local to cropped-stretch matrix. Uncertain branch tests include every
possible branch. The normal transformer branch reached only through AGG's exact
zero-axis test has zero stretch size and paints nothing. Geometry coordinates
and intermediate dimensions remain bounded, as do candidate count and cell work.

Prove with failing orientation/clip/budget tests, a varied owned matrix corpus,
installed PDFium rendering and saved-byte verification. Audit raw RGB here to
isolate placement; full JPEG decoder, page/device frame, compound opaque paint,
private publication, final phase3 review and public acceptance remain required.
Preserve accepted v0.30 and prior evidence. This is coordinator implementation
judgment under the user's standing opt-out of approval questions.
