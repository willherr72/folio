# PDFium form inheritance correction

## Problem and outcome

Folio currently refuses valid nested text whose inherited leading, rise or scale
is lost by bundled PDFium chromium/8044. The independent direct-DLL diagnosis is
recorded in `docs/pdfium-form-inheritance.md`. Correct rendering and editing at the
dependency boundary while preserving source semantics and the existing guards.

## Implementation choice

Build a separate Windows x64 DLL from PDFium commit
`f91ca5a72358bb0b00b4da9481b21fe668157614`. Retain distributor build patches from
`bblanchon/pdfium-binaries` tag chromium/8044, commit
`5453f3afc4785cbad82c05f6ceb4dabea0cb81a0`. Add only three scalar copies in
`CPDF_StreamContentParser::AddForm`, with const getters for leading/rise in
`CPDF_AllStates`. Do not inherit parser text matrices/positions or reinterpret PDF
streams. No JavaScript/XFA, matching the present dependency configuration.

Source, dependency revisions, build arguments, compiler/SDK versions, upstream
patches, Folio patch, licenses and resulting DLL hash must be recorded. Use pinned
depot_tools commit `b52fe87d04af61df3dd07748ff2c2c0b649dca09`, disable auto-update,
and use the installed VS 2022 toolchain plus the required Microsoft Windows SDK
10.0.28000.0. Assemble the SDK from signature-verified Microsoft NuGet packages
10.0.28000.2270 (CPP, x64 and x86), plus debugger platform 20260319.1511.0. Pin their
hashes and keep all extracted files local. Use a private MSVC view/environment
script so the installed SDK/registry remain unchanged. The initial installer
attempt was cancelled at elevation; no further machine-wide SDK install is planned.

All checkouts and build outputs stay under `artifacts/dependencies/pdfium-build-8044`.
Use an owned temporary R: mapping to that directory to keep build paths short;
verify mapping ownership before reuse/removal. Installed MSVC files are referenced
through private junctions and must never be edited or recursively deleted.
No replacement of the product DLL before direct probe success and native candidate
validation. Keep the previous verified executable/DLL checkpoint available. No
commits, external issue posts, release publication or parity claim.

## Acceptance and integration

1. The existing direct probe must fail with eight mismatches on the old DLL and
   pass all 18 pairs in normal mode on the candidate, with expected text/nonblank
   rasters. Extend cases for child-return state, local overrides, transformed
   placements, and embedded/CID fonts where source behavior warrants it.
2. Change the inherited-state refusal regressions into positive edit/save/reopen
   cases only after the candidate actually satisfies the original/flat comparison.
   Preserve clipped, dependent-show, malformed and collision refusals. Verify
   affected exported PDFs in MuPDF/pypdf against explicit authored references.
3. Run the relevant broad native corpus and renderer-dependent workflows. Diagnose
   differences rather than updating reference output blindly.
4. Introduce a reproducible source build recipe and provenance. The dependency
   setup must not silently restore the old known-broken DLL. Include the DLL,
   provenance and patch in build fingerprints and portable packaging validation.
5. Build Folio with the candidate and perform real isolated Windows reading,
   text editing/undo/save/reopen, annotation, form-fill, image export and printing
   checks. Physical printer acceptance remains explicitly separate when unavailable.
6. Obtain one fresh implementation review and one regression-verified correction
   pass. Record precise evidence, unresolved gaps and executable identity.

The direct probe alone cannot authorize dependency adoption. If prerequisites or
build failures arise, keep the candidate separate, preserve logs and continue
resolving the evidenced cause. A successful diagnostic or compile is not product
completion.
