# Fractional Image Sampling Bounds

Continue private JPEG coverage toward final renderer integration. Existing
integer-grid stretch support does not model fractional origins, signed extents,
native small-size clamping or clipped output. Use an interval evaluation of the
native candidate tap window, followed by monotone searches for affected output
pixels. This is tighter and more auditable than a fixed halo, and avoids
enumerating up to 2^24 destination pixels for every selected source region.

Input: SampleRegions already containing decoder and box-reduction influence;
two binary32 endpoint intervals for origins and signed extents, each within
[-2^24,2^24]; optional integer output clip. Output: integer device rectangles
in left/top/right/bottom order, containing complete affected pixels. The model
covers fz_scale_pixmap's full or clipped grid. It does not choose renderer paths,
construct page-to-device transforms or invert output rectangles back to pages.

Split extent-sign uncertainty, apply native minimum absolute extent 1, and bound
pixel-grid origin, subpixel phase and output count. The horizontal negative path
reverses destination columns; the vertical negative path reverses source rows.
Do not assume these paths are interchangeable at fractional coordinates.

Bound centre = ((j-phase+0.5)*source/extent)-0.5 and candidate window =
max(1,1/(extent/source)) with every binary32 arithmetic step enclosed. Directed
division endpoints use exact products of binary32 values to test rounding.
Candidate index bounds are monotone in destination j. Binary search obtains
the first/last potentially influenced output pixel. Include native fallback
floor(centre) clamped to the source; selected edge cells may therefore extend
to the output edge. Window correction only removes candidates. Weight
normalization cannot introduce a sample outside the candidates/fallback.

Use at most 4096 source rectangles, 32768 samples per source axis, two signs per
axis, and logarithmic searches. Return one conservative hull per source region,
then intersect with the output clip. Reject invalid intervals and unsafe
intermediate range. No new dependencies, copied third-party implementation,
public feature enablement, commits or accepted package changes.

Tests and native probes cover fractional and negative origins, all flip pairs,
subpixel extents, edge cells, single-pixel images, cropped output, interval
uncertainty and bounded refusal. Compare actual changed source/edited rasters
with the predicted rectangles, retaining input and output hashes and negative
controls. Existing corpus outputs must remain identical.
