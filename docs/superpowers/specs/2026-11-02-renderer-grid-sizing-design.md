# Renderer Grid Fitting and Decode Sizing

Continue the private JPEG disclosure implementation under standing autonomous
authorization. Source-token correspondence and forward transform bounds exist;
decode sizing still requires caller-supplied extents. Close that connection for
MuPDF's actual pre-decode grid fitting and binary32 axis lengths.

Use interval evaluation of the observed grid-fitting choices, rather than an
ideal norm or a fixed pixel allowance. A generic allowance loses translation,
reflection and branch-threshold information needed by subsequent placement work.
Keeping this outside the existing transform would duplicate arithmetic proofs.
The coordinator chooses a child module of RendererTransform for bounded grid
fitting and axis extents, with a sizing result exposing the fitted transform,
integer extent intervals and maximum decode reduction.

Modes are Disabled, Outward and Tiled. Classification follows binary32 epsilon
tests. Uncertain classifications include all possible branches. Integer snapping
uses bounded truncation and interval arithmetic, retaining both outcomes of an
uncertain epsilon comparison. Both signs and zero are included as appropriate.
Tiled quarter-turn behavior must match the native implementation, including its
different origin mapping. References are inspected, not copied into product.

Axis lengths bound square/add/sqrt binary32 arithmetic. Directed square-root
endpoints are checked by exact binary64 squares of binary32 candidates. Reject
unbounded casts and extents above 2^24 before integer conversion. Source extent
clamping and the existing maximum-reduction function remain authoritative.

Tests cover reflections, quarter turns, epsilon boundaries, interval branches,
fractional negative origins, large/cancelled coordinates, degeneracy and refusal
bounds. Export owned matrix cases and compare fitted coefficients, requested
dimensions and reduction against actual MuPDF. Keep predecessor artifacts and
native/runtime hashes. No public JPEG admission, inverse-transform proof, final
opaque paint union, package replacement or commercial parity claim is included.
