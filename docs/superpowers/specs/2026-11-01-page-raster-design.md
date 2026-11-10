# Reusable page raster foundation

The 252-cycle experiment supports a persistent canvas but does not establish
production fidelity. Build and test a reusable React component before changing
PageView or a portable package. This is a concrete foundation milestone, not a
memory-fix release. The user's standing autonomous instruction covers these
implementation choices; no additional design permission is needed.

## Approach and boundaries

Use a persistent canvas inside SVG foreignObject, at the same SVG coordinates as
the current image. Keep the current SVG image behavior as fallback. The measured
two-slot SVG alternative still exceeded the unchanged memory budget; the existing
renderer remains the integration baseline. Do not use injected DOM observers.

PageRaster accepts url, width, height, transform and optional onStatus. URLs are
borrowed: never revoke them. One serial decoder per mounted component owns at most
one fetch/decode and the latest requested URL; cancellation aborts fetch, drops
stale decoded results and closes every bitmap. Cleanup also releases canvas
storage. Same-size successful paints reuse its backing dimensions. Clear before
drawing to prevent transparency from retaining a previous page.

Only local blob PNGs enter the canvas path. Cap encoded data at 32 MiB, dimensions
at 16384 per axis and 16,777,216 pixels before bitmap decode. Validate signature,
IHDR and decoded dimensions. Stream reads stop at the encoded cap. Unsupported
formats/APIs, oversized inputs and decode/context failures use SVG fallback.
These are extra-buffer bounds, not a claim about the existing SVG fallback's
memory. No new dependencies. Missing URLs show neither a stale page nor an error.

Use alpha-enabled, willReadFrequently 2D context. Maintain pointer transparency,
no focusable raster nodes, source-to-page geometry and overlay ordering. Loading
and fallback status must be tied to the current request, including A-B-A races.

## Fidelity contract and gates

Require exact raw decoded RGBA for opaque source and edited native PNG fixtures.
Transparent pixels can undergo browser premultiplication: compare both reference
and candidate composited pixels, and preserve raw differences separately. Do not
reclassify the previous zero-difference display failures as passes. In this
foundation, scaled/rotated/cropped display comparisons are diagnostic results,
not a newly relaxed shipping threshold. No production switch until geometry,
display filtering, real editor interactions and sustained memory are accepted.

Test out-of-order completion, cancellation, unmount/remount, same-URL repeats,
URL ownership, fallback recovery and storage bounds. Exercise the actual bundled
React component in Chromium with opaque/alpha PNGs, source/edited native frames,
rotations, crop, zoom, parent raster transform, focus, pointer and overlay order.
Browser evidence here is standalone component evidence, not Windows WebView or
whole-editor acceptance. Keep existing package and all failed evidence.
