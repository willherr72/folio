# JPEG Tile Frame Propagation

Continue private JPEG integration by connecting actual decoded source windows to
renderer transform bounds and the existing fractional sampling helper. Use the
actual returned tile frame, including full-image cache fallback; requested crop
coordinates are insufficient authority. Native pixmap coordinates use row zero
at the matrix origin, so this helper does not apply PDF's image-row inversion.
The page-to-device caller remains responsible for that initial convention.

Add RendererTransform::mupdf_tile_frame(source, actual_window), validating source
axes 1..32768 and a nonempty in-source window. The full-image frame returns self
unchanged. Partial frames enclose binary32 ratios (width/source_width,
height/source_height, left/source_width, top/source_height), then use existing
bounded precomposition. Reuse the proved interval division primitive by moving
it from the fractional module onto Interval; keep its semantics unchanged.

Rejected alternatives: a binary64 ideal crop matrix loses native rounding;
mapping requested crops instead of actual returned frames misplaces cached full
tiles; rounding the nominal image transform loses the original source-removal
identity. This helper changes only renderer disclosure arithmetic.

Integrate evidence through the owned JPEG decoder corpus: export bounded native
tile transforms and post-grid-fit scale support for actual decoder cell bounds.
Audit original and coefficient-redacted JPEGs through fresh crops and cached
full-image results, axis/reflected/quarter/general transforms, and post-decode
grid-fitting modes. Verify each transform and all scaled changed pixels. Keep
existing exported bytes identical and retain reader/reference/source hashes.

This does not choose arbitrary source crop/cache policies or prove affine final
page painting. Device composition, inverse mapping, opaque paint union, final
phase3 review and public acceptance remain required. No package change/commit.
