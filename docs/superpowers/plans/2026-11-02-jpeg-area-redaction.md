# JPEG area redaction implementation plan

Execute inline using superpowers:executing-plans under the standing autonomous authorization. The whole target is source-preserving JPEG area redaction, followed by public combined preview/apply. The current phase proves the codec choice; it does not narrow completion to a prototype.

Spec: `docs/superpowers/specs/2026-11-02-jpeg-area-redaction-design.md`.

## 1. Fidelity investigation and executable proof

- [x] Compare JPEG normalization with current native and independent readers on owned Gray/RGB baseline/progressive fixtures.
- [x] Compile and test a coefficient-domain prototype, verifying exact outside coefficients and rendered bounds with corruption controls.
- [x] Review evidence and integration requirements, preserve provenance and checkpoint the decision.

## 2. Bounded production codec foundation

- [x] Select and pin a supported libjpeg-turbo source release and its license record; repeat the coefficient proof on that version.
- [x] Vendor the production dependency and integrate build/source fingerprinting and license collection.
- [x] Add a small in-process C/Rust interface with contained errors, immutable input, explicit memory/work/output budgets and cleanup.
- [x] Observe failing valid/malformed/budget tests, implement the bounded codec, and verify strict marker/color/precision admission and fresh metadata-free output.

Foundation evidence: `artifacts/jpeg-codec-foundation-20261102/checkpoint.json`. Final ordinary redaction gate:170 passed,0 failed,1 explicit producer-corpus skip. Independent actual native output:12 cases,64,768 retained coefficients exact,5,888 selected coefficients checked, PDFium/MuPDF at1x/3x, eight zero-mask cases and two negative controls. Review findings corrected with observed red/green regressions; remaining coverage limits are explicit in the coordinator disposition. This completes the private codec foundation only; phases3/4 below remain open.

## 3. Image model and area publication

- [x] Add explicit pixel/JPEG payload types and stable domain-separated identities without altering raw/Flate behavior.
- [x] Bind selected source pixels to MCUs, distinguish rewrite and disclosure masks, bound transformed fringe geometry, and guarantee selected-area opacity with correctly ordered fills and no adjacent seams.
- [x] Publish isolated sanitized occurrences; test shared images, other pages, text/path composition and save/reopen/re-edit.
- [x] Independently compare source/kept coefficients, retained content and rasters; run serial native checks and one final fresh review for the implementation phase.

Model-stage evidence: `artifacts/jpeg-image-model-20261102/checkpoint.json` (177 ordinary native passes, one explicit skip;14 independently checked saved-model cases; unchanged existing pixel outputs). Page JPEG admission remains gated. The fill-reedit counterexample requires verified image/fill ownership and preservation of previous coverage; see the model design's integration findings. Phase3 is still in progress and has not received its final integrated review.

Coverage-stage evidence: `artifacts/jpeg-coverage-20261102/checkpoint.json`. Original pixels now map to immutable MCU masks; mask union retains prior coverage; coalesced compound paint preserves holes; serialized coordinate envelopes handle affine placement. Ordinary native:186 passed,0 failed,1 explicit skip;107 previous corpus files remain byte-identical. Six saved-model placements pass48 two-reader/scale pairs and all3 corruption controls;23,040 retained coefficients exact,4,608 selected coefficients checked. Low-scale/sheared raster evidence disproves a universal one-source-pixel fringe assumption. Persistent source-bound paint ownership, transaction-wide codec resources, derived disclosure bounds and full page publication remain open, so the phase3 checkbox above remains unchecked.

Ownership-stage evidence: `artifacts/jpeg-ownership-20261102/checkpoint.json`. Versioned records bind canonical image content/color and masks; another charged codec pass proves claimed blocks already removed. Identity includes required coverage, re-edit unions prior masks, and per-occurrence source verification precedes logical fill collapse. The actual fresh-graph publisher now emits owned paint after each occurrence with context/depth/operation guards. Ordinary native:198 passed,0 failed,1 explicit skip;143 older saved outputs remain byte-identical.18 actual publisher PDFs pass independent record/coefficient/two-reader checks, including12 bound records and3 corruption controls. General source canonicalization, transaction-wide codec reuse/budgets, ImageWork integration and conservative disclosure remain pending; JPEG page admission stays guarded and phase3 is not complete.

## 4. Public feature acceptance

- [x] Connect the combined host/UI preview/apply transaction, show actual affected areas, and validate Windows save/recovery/undo paths.
- [ ] Revisit transparency, masks, profiles, general PDF image compatibility and resource-quality requirements. Keep the broad parity goal active until independently proven.

Transaction-cache continuation (phase3 still in progress): `artifacts/jpeg-transaction-cache-20261102/checkpoint.json` records209 ordinary native passes,0 failures,1 explicit skip and161 previous outputs byte-identical. An explicit per-edit ImageContext now shares validated source/canonical-output images, pixel/JPEG variants and bounded input/hash/codec/coverage work through preparation, text removal, space probes and reopening. ObjectId aliases remain document-local; hits cannot authorize occurrence paint. JPEG variant re-edit retains prior coverage. JPEG page admission remains guarded. Multi-page selection still repeats source parsing/hashing under the shared budget; batch inspection remains a large-document follow-up. Source fill canonicalization, JPEG ImageWork integration, conservative disclosure and the final phase3 review remain next.

Occurrence integration continuation: `artifacts/jpeg-occurrence-integration-20261102/checkpoint.json` records218 native passes,1 explicit skip; actual parser/worker/kernel save/re-edit with isolated shared JPEG occurrences and retained owned fills. Six placements produce18 audited transaction PDFs across48 reader/scale pairs; combined text/path/image output passes16 additional raster comparisons. An independent outside-pixel failure exposed nominal text-show float rounding; the bounded exact-advance choice corrects it without widening exclusions, with all96 legacy text cases and23 changed raw/predictor outputs re-audited successfully. Ordinary JPEG admission remains false, with a test-only context constructor exercising the real path. Source parsing and ImageWork integration are complete privately; derived general resampling disclosure and final integrated phase3 review remain next before public enablement.

Disclosure geometry continuation: `artifacts/jpeg-disclosure-20261102/checkpoint.json`
records223 native passes,1 explicit skip and374 prior saved outputs unchanged.
The geometry helper now separates clipped image-unit filter support from
page-space display margins, with outward arithmetic and unchanged removal/paint
identity. It takes explicit margins without defaults; it is not a completed
renderer support policy. Pinned renderer source and exact quantization evidence
explain why fixed one-pixel assumptions are insufficient. Complete support
derivation, reduced-decode/partial-edge validation, final phase review and public
admission remain open; phase3 is not marked complete.

Decoder-support continuation: `artifacts/jpeg-decoder-support-20261102/checkpoint.json`
adds20 owned partial-edge fixtures, including vertical subsampling, and validates
28 aligned/partial cases. Actual rewrite/re-edit outputs pass1792 scalar decode
comparisons and224 native/independent full-size comparisons with effective
controls. Decoder sample regions now use actual ceil-divided bitmap dimensions;
1344 exact geometry checks and1344 decoded-data comparisons verify the helper.
Final focused native:30 passed; fast harness:100 passed;114 native/harness decoder
artifacts and86 prior outputs match. This closes the tested decoder/full-bitmap
mapping gap, not crop/subarea or downstream page sampling. The general disclosure
policy and public feature acceptance remain the next work.

Sampling continuation: `artifacts/jpeg-sampling-support-20261102/checkpoint.json`
records conditional integer-grid box/scale support, including MuPDF's overlapping
partial-edge reduction groups. Actual native redaction:232 passed,0 failed,1
explicit skip; fast harness:105 passed.28 owned cases pass112 actual box and504
actual scale comparisons with effective outside-cell and naive-box controls.
All114 prior decoder artifacts and375 previous corpus artifacts are identical;
116 native/harness artifacts match. These operators do not choose renderer paths
or establish cropped, fractional, grid-fit or affine page bounds. Complete
disclosure policy and the final phase3 review remain open; public admission and
the accepted package remain unchanged.

Crop-support continuation: `artifacts/jpeg-crop-support-20261102/checkpoint.json`
records237 native passes,1 explicit skip and110 fast passes. Integer tile crops
preserve expanded decoder support and map through the actual original-image
frame with outward rational arithmetic.438 configurations and686 actual MuPDF
tile comparisons cover partial edges, unaligned requests and full-image cache
fallback. Existing116 decoder artifacts and375 corpus artifacts stay identical.
An exact pre-sampling crop-union bound is derived for future policy integration;
complete renderer choice, extra filtering, matrix/grid-fit and affine composition
remain open, along with final phase3 review and public feature acceptance.

Combined source-envelope continuation:
`artifacts/jpeg-source-envelope-20261102/checkpoint.json` records242 native
passes,1 explicit skip,115 harness passes, and1,329,366 exact crop/box checks.
An explicit maximum-factor envelope covers all admitted aligned crop, extra
reduction and higher-resolution cache choices without changing removal masks.
29 owned JPEGs pass606 actual MuPDF comparisons through total reduction64;
each changed cell's complete source-coordinate footprint is enclosed.117 older
decoder artifacts and375 corpus artifacts remain identical. A conditional
stretch term passes1344 exact native-geometry comparisons but is not integrated
yet. Renderer sizing, matrix/grid-fit/affine composition, opaque paint union,
final phase3 review and public acceptance remain open.

Renderer-sizing continuation:
`artifacts/jpeg-render-sizing-20261102/checkpoint.json` records247 native passes,
1 explicit skip and120 harness passes. PDFium decode sizing uses render-device
dimensions; MuPDF uses proven lower bounds on requested transform extents.
224 actual PDFium candidate comparisons and203 MuPDF size checks pass, including
zoom-cache transitions. Conditional outward stretch composition is implemented
and passes609 exact configurations plus1344 previous native scaling cases.
177 prior decoder artifacts and375 corpus artifacts are unchanged. Binary32
matrix/axis bounds, fractional/grid-fit/affine composition and opaque paint union
remain necessary before final phase3 review and public preview/apply acceptance.

Renderer-transform continuation:
`artifacts/jpeg-render-transform-20261102/checkpoint.json` records253 native
passes,1 explicit skip and126 harness passes. Separate binary32 forward bounds
contain717 actual parsed PDFium matrices,717 native MuPDF concatenations, and
5736 native points plus5736 exact rational points.179 decoder artifacts and375
previous corpus artifacts remain unchanged. Actual MuPDF lexer evidence exposes
a valid leading-zero numeric token that becomes NaN despite a finite ordinary
conversion. Original operand correspondence is therefore an explicit additional
integration requirement. Source selection remains unchanged; no inverse, raster,
grid-fit or full disclosure proof is implied. Phase3 and public acceptance remain
open; the accepted portable package is unchanged.

Content-number correspondence continuation:
`artifacts/jpeg-number-correspondence-20261102/checkpoint.json` records260 native
passes,1 explicit skip and131 harness passes. Original content and final saved
operands must prove equivalent finite reader interpretation.3021 tokens produce
12084 actual-reader comparisons:2781 admitted tokens agree before/after saving;
213 refusals expose native differences and27 are conservative proof refusals.
All179 decoder artifacts,375 prior corpus artifacts and2 transform exports are
unchanged. Dictionary numeric provenance and the remaining placement policy are
still open. This is a private fidelity guard, not public JPEG enablement or
final phase3 acceptance.

Native tile-placement continuation:
`artifacts/jpeg-tile-placement-20261102/checkpoint.json` records 280 native
passes, 1 explicit skip and 144 harness passes. Actual returned crop frames now
precompose bounded binary32 renderer transforms, preserving full-frame identity.
The linked coefficient-redacted JPEG pipeline passes 6120 native decoded pairs
and 4590 post-grid-fit/scaled pairs. All 234504 decoded and 200836 scaled changed
pixels are contained; the saved 33,225,120 pixel bytes were separately rechecked.
Controls distinguish requested crops from cached full frames. All 582 earlier
exported artifacts remain identical. Device composition, general affine/inverse
sampling, opaque paint union and final phase3/public acceptance remain open.

Fractional-sampling continuation:
`artifacts/jpeg-fractional-scale-20261102/checkpoint.json` records 277 native
passes, 1 explicit skip and 141 harness passes. Interval candidate-tap bounds
cover fractional origins, signed/clamped extents, separate horizontal/vertical
flip paths, central-sample fallback and clipped output. Actual native scaling
passes 2056 comparisons across 1840 owned cases; all 48063 changed pixels stay
inside predicted complete-pixel bounds. A separate saved-byte verifier checks
every source and output archive record. All 581 prior exported artifacts remain
unchanged. Device composition, actual cropped-tile frame propagation,
inverse/affine support, opaque paint union and final phase3/public acceptance
are still open. This does not enable JPEG page admission.

Native grid-sizing continuation:
`artifacts/jpeg-grid-sizing-20261102/checkpoint.json` records 272 native passes,
1 explicit skip and 136 harness passes. RendererTransform now bounds MuPDF
disabled/outward/tiled grid fitting and source-clamped integer axis extents,
feeding the existing maximum decoder reduction. All 1326 actual native checks
across 386 owned matrices pass, including 180 samples of interval-valued inputs.
Controls identify 449 grid-induced size changes and three ideal binary64 norm
disagreements. The 580 prior exported artifacts remain unchanged. This closes
the conditional pre-decode grid-fitting/sizing connection, not page-to-device
integration, fractional/cropped filtering, inverse/affine support, opaque paint
union, final phase3 review or public feature acceptance.

Dictionary-number correspondence continuation:
`artifacts/jpeg-dictionary-numbers-20261102/checkpoint.json` records 267 native
passes, 1 explicit skip, 9 loader lifecycle passes and 131 harness passes.
Opt-in raw numeric validation covers direct and compressed objects at all five
private redaction byte-loading routes; final fresh graph numbers are validated
before serialization. Malformed numeric prefixes are refused before approval.
Fourteen owned inherited page-box cases produce eight accepted cases and six
refusals with actual reader geometry differences. Sixteen source/saved raster
comparisons are unchanged. The final exports match the audited inputs exactly;
179 decoder artifacts, 375 earlier corpus artifacts, two transform exports and
the numeric corpus remain unchanged. Whole-document conservative admission is
explicit compatibility debt. Remaining renderer-placement and final phase3
acceptance work is still required before public JPEG enablement.

Reader-inverse continuation:
`artifacts/jpeg-reader-inverse-20261102/checkpoint.json` records 284 native
passes, one explicit skip and 148 harness passes. Separate renderer-only inverse
intervals contain the PDFium binary32 and MuPDF binary64 evaluation routes.
832 owned inputs yield 10022 native comparisons; all 9987 admitted samples also
contain exact rational inverses. PDFium evidence uses its pinned source compiled
with the pinned compiler, not a public-DLL inverse call. Independent rational
Gaussian elimination verifies all persisted records. All 583 previous exports
remain byte-identical. Twenty-three inputs are conservatively refused, including
some finite inverses; source-selection geometry is unchanged. Fixed-point affine
sampling, page/device composition, complete opaque paint union, final phase3
review and public acceptance remain open. JPEG admission remains disabled.

MuPDF affine-sampling continuation:
`artifacts/jpeg-affine-sampling-20261102/checkpoint.json` records 289 native
passes, one explicit skip and 153 focused passes. Renderer-only influence now
bounds scan origins, 14-bit truncated steps, int32 initial coordinates and int64
increments. Actual drawing passes 3336 pairs across 1248 owned cases; all 61470
changed pixels remain inside predicted bounds. Ideal geometry misses changes in
336 pairs. Every saved source/output byte is independently reverified, and all
584 prior exports are unchanged. The audit is conditional on the actual post-scale
pixmap/frame and effective scissor. PDFium sampling, combined device/decoder/scale
integration, opaque paint union, final phase3 review and public acceptance remain
open. This adds no new source-removal authority or public JPEG admission.

PDFium affine-sampling continuation:
`artifacts/jpeg-pdfium-affine-20261102/checkpoint.json` records 294 native
passes, one explicit skip and 158 focused passes. Conditional post-stretcher
influence covers 8-bit coefficient rounding, float point arithmetic, negative
fractional edge access and inclusive final-pixel clamping. A pinned-source native
probe passes 642 pairs across 386 owned inputs; all 13254 changed pixels stay
inside predicted bounds. Every saved source/output byte is independently checked,
and all 585 predecessor exports remain identical. This is explicitly not a render
through the installed DLL. Preceding stretch/branch/device integration, actual
combined reader rendering, opaque paint union, final phase3 review and public
acceptance remain open. Source-removal selection is unchanged.

PDFium signed/clipped stretch continuation:
`artifacts/jpeg-pdfium-stretch-20261102/checkpoint.json` records 299 scoped native
passes, one explicit skip and 163 focused passes. Bounds cover signed constructor
source windows, nearest/bilinear/area candidate weights, clamped boundary reuse
and the cropped output frame. The installed PDFium DLL audit passes 2304 render
pairs across 768 owned cases; all 23507 changed pixels are contained. Independent
verification checks every saved source/output pixel and all 4608 PDFs, including
raw image bytes and placement commands. The initial audit's extra display-axis
flip is retained as failed evidence and corrected only in the harness. All 586
predecessor exports remain identical. Uncertain tiny source windows are refused
conservatively. Renderer placement branches, page/device/decoder composition,
opaque paint union, final phase3 review and public acceptance remain open.
Source-removal selection and accepted v0.30 are unchanged; public JPEG admission
remains disabled.

PDFium AGG image-placement continuation:
`artifacts/jpeg-pdfium-image-20261102/checkpoint.json` connects the actual decoded
sample frame to normal, reflected, quarter-turn and general-transform device
placement. Bounded integer alternatives preserve uncertain native rounding and
branch thresholds; inverse/cropped-stretch arithmetic remains outward. The
installed renderer passes 2052 pairs across 684 owned cases, containing all 58636
changed pixels. Independent verification checks all saved samples and 4104 input
PDFs. Scoped native regression: 306 passed, one explicit skip; fast harness: 170
passed. All 587 predecessor exports remain identical. Accepted v0.30 GUI, CLI and
ZIP hashes are unchanged. Full JPEG decoder/page-device composition, MuPDF's
complete tile/scale route, compound opaque paint and public acceptance are still
required. This conditional device calculation never enlarges source removal.

Source-bound PDFium JPEG influence continuation:
`artifacts/jpeg-pdfium-influence-20261102/checkpoint.json` retains native content
matrix arithmetic per selected occurrence and joins decoder/cache alternatives
with the AGG placement calculation under a shared work budget. It preserves the
original MCU mask and keeps influence separate from opaque paint. All 8352 saved-
PDF render pairs pass across 29 unchanged source/coefficient-rewritten fixtures;
676 cases distinguish warmed caches. All 2611724 changed pixels are contained.
Independent verification checks every saved byte and all 16704 PDFs. Scoped
native regression: 313 passed, one skip; fast harness: 177 passed. All 588 earlier
exports remain identical. Canonical page/device derivation, complete MuPDF
tile/scale composition, opaque paint and combined private/public acceptance
remain required. Accepted v0.30 and public JPEG admission are unchanged.

Source-bound MuPDF JPEG influence continuation:
`artifacts/jpeg-mupdf-influence-20261102/checkpoint.json` connects source crop,
decode/cache alternatives, extra box reduction, fractional scaling and final
affine painting under shared bounds. Original cm operands are retained and
replayed from the initial interpreter matrix, including q/Q restoration and the
image row flip; the caller must prove that initial matrix and an identity draw
device. Actual saved-PDF rendering passes 25056 pairs across 4176 cases; all
7017651 changed pixels are contained. Separate native checks validate 8352 tile
choices, including 2552 cache frame/dimension changes. Independent byte/pypdf
verification checks every retained raster and all 50112 PDFs. Scoped native:
321 passed, one skip; focused: 185 passed. All 647 predecessor exports remain
identical. The complete reader connection is verified for the finite contract;
canonical viewer binding, explicit-color combined coverage, compound opaque
fill/publication acceptance, final phase3 review and public UI remain open.
Original source-removal masks, accepted v0.30 and ordinary JPEG admission are
unchanged. This does not claim release or commercial parity.

Canonical combined JPEG publication continuation:
`artifacts/jpeg-combined-render-20261102/checkpoint.json` binds the real page
viewport to reader calculations and includes old/new opaque-paint footprints.
Independent rendering exposed PDFium's snapped-rectangle edge trimming; new
owned fills use compound triangles with unchanged source geometry. Coverage
version 2 binds this grammar, while untouched version 1 fills retain their
appearance and explicitly selected edits upgrade safely. Source-cache identity
includes that version. Actual save/reopen/re-edit passes 560 ordinary-viewer
comparisons and 56 coefficient checks; independently reverified saved pixels
include 768864 black-reference pixels and 56007 boundary pixels. Mixed-content
acceptance adds 16 comparisons. Scoped native: 328 passed, zero failed, one
explicit skip; focused: 191 passed. Of 707 prior exports, 651 remain identical
and 56 differ only in the intended paint/version correction; image payloads are
unchanged. The reference is built from exact original MCU geometry, not output
paint. Previous direct MuPDF audits used a non-null separation/proofing target;
the new audit uses the ordinary-viewer null-separation target and does not
relabel old evidence. Color overrides, selected final partial MCUs, disjoint/hole
boundary cases, ordinary-viewer cache variants, final integrated phase3 review
and public Windows/UI acceptance remain open. Accepted v0.30 is unchanged.

Phase 3 private integration complete for the admitted contract:
`artifacts/jpeg-publication-coverage-20261102/checkpoint.json` records the final
332-test native pass (zero failures, one explicit skip), 3024 actual saved-PDF
comparisons and independent verification of all retained pixel evidence. The
expanded corpus includes explicit ColorTransform 0/1, Gray/RGB, partial final
MCUs, disjoint selections, holes, seven placements and cold/warm rendering.
All 252 coefficient checks pass; 14803353 changed pixels stay inside derived
bounds, with 24123374 black-reference and 893990 boundary-reference pixels.
Actual cache differences occur in 42 PDFium and 168 MuPDF comparisons.

The sheared-ring failure led to a bounded merged-contour publisher. New version
3 binds its source-grid union grammar; untouched versions 1/2 retain their exact
appearances and explicitly selected edits upgrade. Exhaustive 512 small-mask
checks cover winding/hole/diagonal-contact geometry. The final corpus matches
the independently audited corpus byte-for-byte. Of 848 predecessor files, 766
are identical and 82 differ only in coverage version/binding records; all 84
compared JPEG payloads and all page content streams are unchanged.

The one fresh integrated phase3 review approves the private integration with no
actionable findings; 108 frozen hashes remained stable. Its remaining finite
corpus, numeric-boundary, large-document resource/performance and compatibility
limits are retained in `review-phase3/review.md`. Phase4 now remains: combined
host/UI preview/apply, actual affected-area disclosure, Windows save/recovery/undo
and packaging acceptance. Public JPEG admission is still disabled and accepted
v0.30 is unchanged. This is not release or commercial-parity completion.

Phase4 desktop integration checkpoint: `artifacts/content-removal-workflow-20261102/checkpoint.json`. The explicit combined route enables JPEG only in its owned prepared-source context, with actual paired native rasters, conservative source-bound disclosure and all-page review. Corrected native343pass/1skip; unchanged frontend1450pass plus fresh build; clean Windows Apply/Undo/Save/reopen/recovery and tall-page tests. Independent audit:72 reader/viewport comparisons,1296 coefficient blocks,18 vector checks,45 controls. One integrated review finding was reproduced and fixed with a stricter prepared-source boundary. Accepted0.30 remains unchanged. The remaining phase4 compatibility/resource checkbox and portable packaging remain open; no universal sanitization or parity claim.
