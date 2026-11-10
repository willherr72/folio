# Persistent page-raster experiment

The previous diagnostic reproduced the unchanged memory-growth budget failure in
five optimized 0.27 sessions and identified growing GPU transfer-cache entries
whose sizes correspond to page rasters. Exact DOM ownership is still unproved.

## Decision and alternatives

Under standing autonomous authorization, test a reusable CPU-backed canvas inside
the existing SVG page transform. It targets repeated image-texture presentation
without changing the PDF engine, document state or packaged app. Changing browser
cache flags would be engine-specific and would not establish Folio's ownership
behavior. Caching only identical PNG content would not address ordinary varying
edits. Those alternatives are not the selected experiment.

## Protocol

Use the exact verified 0.27 package and prior Word fixture, captured and hashed
before launch. Run fresh stock and persistent-canvas sessions with the established
63 preview/cancel cycles, viewport, text and six unchanged budgets. Preserve every
failure and original package. No forced GC, browser cache purge or threshold change.

The diagnostic replaces only visible page-background presentation: retain the
existing SVG image node for source observation but hide its painting, place a
pointer-transparent foreignObject/canvas in the same source coordinates, copy
the image transform, and draw decoded PNGs into one reused 2D context with
willReadFrequently:true. Reset dimensions only when pixel dimensions change.
Track generations so superseded loads cannot paint; close temporary ImageBitmaps;
hide the surface when the native raster is absent. Preserve SVG overlays and
all existing interaction targets. Record context attributes, host creation count,
draw count, errors and readiness, without retaining decoded image data in logs.

This is an injected diagnostic surface, not shipped behavior or a production
component. Observe the actual drawing readiness before preview/Cancel assertions.
At open, first/last preview and final Cancel, capture the same page with canvas
presentation and native SVG image presentation. Independently compare the PNG
pixels. Stock repeats the same screenshot/animation-frame observation schedule.
Do not infer fidelity from source-URL hashes alone.

If fidelity and resource behavior improve, confirm with varying content before
proposing production integration, whose rotation/crop, focus, overlay, teardown
and asynchronous load tests remain mandatory. If the hypothesis fails, retain
the negative evidence and identify the next boundary rather than shipping it.
Finite sequential runs cannot certify a leak fix or constrained-hardware quality.

## Scope

No product code change, build, commit or packaging in this initial experiment.
Use owned process creation identities, pre/post runtime/input hashes and one
final independent review of the completed evidence. Keep the full goal active.

## Observed display distinction

The first canvas stress run passed resource thresholds but failed exact scaled-
display equality. A separate source-pixel inspection proves its raw canvas pixels
equal the native PNG; the CPU surface differs by one channel level at a small
number of scaled display edges. Preserve that strict failure. Proceed with varied
content solely to test the independent resource hypothesis; this is not production
acceptance or a revised passing pixel threshold. Broader transform/interaction
tests and an explicit rendering-quality decision remain required.

## Same-renderer lifetime discriminator

The CPU canvas materially reduces growth but changes scaled edge rounding. Test
two retained SVG image slots as a second bounded prototype: keep the unchanged
source image's element/resource intact while the other slot receives candidates.
Use the original SVG image geometry and preserveAspectRatio unchanged. This is
motivated by two new page-sized GPU entries per cycle in the earlier trace;
their exact source ownership remains a hypothesis. Hide the product-owned image
and mirror its requested href into at most two proxy images, reusing matching
hrefs and replacing the least recently used other slot. Preserve strict displayed
pixel equality, native workflow and all original resource budgets. Do not infer
readiness from an href string alone: wait for the selected proxy's load event.
