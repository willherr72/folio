# Renderer sizing and conditional stretch composition

Continue the existing private JPEG area-publication phase. No public admission
or package change. The prior checkpoint's source envelope needs renderer-proved
decode limits and downstream filtering before page placement can be bounded.

Pinned PDFium StartLoadDIBBase passes render-device width/height through the
loader/cache into CPDF_DIB, not the image transform. Fresh decode uses the
largest power-of-two <= min(source_width/device_width,source_height/device_height),
clamped to1..8. The local renderer forces full decoding for partial MCU edges.
Its cache admits a reduced bitmap only when both bitmap dimensions cover the
device. For fully aligned JPEGs those dimensions are exactly source/D, so the
same maximum covers cache reuse. Partial-edge sources always have factor1.
This helper accepts positive device dimensions and verified JPEG shape facts.

MuPDF chooses factors through64 from the whole image's post-grid-fit transform,
before subarea calculation. Given proven lower bounds on its integer requested
extents, use max(lower,1), clamped to the source size, and require both next
reduced dimensions >= that value+2. A possible actual zero extent disables
reduction and is covered by factor1. A lower bound of zero must not be treated
as proof that the actual extent is zero. Actual matrix-to-integer lower bounds
remain a separate forthcoming proof; do not feed unrounded ideal lengths here.

Add redaction_render_sizing.rs with these two bounded integer policies and
focused tests. Export policies with owned fixture and device/extent inputs.
Audit PDFium against all scalar decode candidates rendered as raw images,
including image placement independent of device size and cache warmup. Audit
MuPDF actual tile dimensions using explicit integer/fractional extent examples.

Add SampleRegions::into_stretch_envelope(T,pitch_upper), consuming an existing
source-coordinate envelope. Validate factor, source dimensions, and positive
finite destination source-pitch bounds. Its conditional full-grid source margin
is max(T,P)+(T+P)/2 per axis. Outward arithmetic and the established image-unit
expansion helper preserve endpoints and clipping. Caller must prove the pitch;
fractional scaling/window arithmetic is not silently added to this contract.

Observe red/green tests, verify exact rational margin containment and prior
native scaling geometry, run serial redaction tests, and bind evidence. Renderer
matrix/grid-fit/affine bounds and serialized opaque paint remain unfinished;
final integrated review and public preview/apply/Windows acceptance remain open.

Evidence: artifacts/jpeg-render-sizing-20261102/checkpoint.json. Final native
247 passes with one explicit corpus skip; standalone120 passes.224 PDFium
comparisons match the expected scalar candidate exactly; image placement and
device size are varied separately, including cache upgrade/retention.203 MuPDF
tile-size checks cover the extent-lower-bound contract.609 native stretch
configurations pass exact outward checks and enclose1344 earlier native scaling
configurations. All177 earlier decoder and375 corpus artifacts remain identical.
This validates the conditional pieces, not the still-open binary32 matrix,
fractional/grid-fit and affine page-disclosure composition.
