# Automation paragraph area and baseline correction

Evidence is rooted at `artifacts/portable-025-20261029`; staged corrections and
their focused harnesses are in its `font-fit-correction` subdirectory.

The native-full-2 regression and font-fit-diagnosis-1 reproduce a real compatibility
failure: changing Helvetica to DejaVu Serif with Fit succeeds; supplying the returned
rectangle with fit:false fails. The wrapper translates the source baseline by the
rectangle-origin change even though Fit added headroom rather than moving text.

This is a coordinator decision within the user's autonomous development instruction.
No additional permission or separate design-review round is needed. Product/test
inputs remain frozen until the active native-full-2 command terminates.

## Alternatives and decision

Translating the baseline for every rectangle-origin change mistakes Fit-added
headroom for movement. The first correction tried legacy frame placement for all
explicit areas; focused testing rejected that approach because it also displaced
text on ordinary resizing and changed a requested 160-point baseline to 159.818.
The failed source and evidence remain in `font-fit-correction/round-1`,
`green-native-1` and `contract-red-2` under the portable artifact root.

Use a separate optional baseline control and a precise default convention:
- Omitted bounds retain the inspected source baseline.
- Bounds with changed width or height resize the area and retain that baseline.
  Fit-produced rectangles therefore do not move text when reused.
- Bounds with exactly the inspected width and height are position-only moves;
  translate the baseline by their displayed origin offset.
- An explicit `baselineAnchor:{x,y}` overrides these defaults, including a combined
  move and resize. Callers can copy the reported point and rectangle together.

This is an explicit geometry contract, not a font-dependent inference or a retry
with relaxed validation. Position-only clients should copy the inspected dimensions
exactly; an explicit point expresses placement when changing dimensions too.
Fit retains the resolved anchor. Exact source no-ops preserve the original bytes.
Preview metadata reports the resolved anchor when available; older metadata without
this optional field remains readable.

This supersedes the automation-only implicit-origin-translation rule in the earlier
baseline-anchor spec. Native layout, desktop intent/history, source authorization,
font proof, clipping and collision checks remain unchanged. Only the point's baseline
axis positions the line; the rectangle and alignment determine horizontal placement.
On quarter-turned pages the displayed baseline axis is x instead of y.

## Validation and limits

Use a strict two-number baseline object, reject null, unknown members, nonfinite and
absolute coordinates above 1,000,000. Keep native page, area, source binding, font,
collision, clip, permissions and raster checks unchanged. The anchor is placement,
never authorization. A changed anchor prevents exact-input no-op classification.

Keep the existing automation_fonts regression unchanged in its fixed-area
expectation. Add anchored Fit/echo pixel equivalence, deliberate movement with changed
ascent, invalid-anchor refusal without output, exact source no-op, nested forwarding
and MCP schema/real-call coverage. Existing native rotation/UserUnit tests still cover
physical conversion, with a targeted automation rotated case if an existing fixture
can exercise it without unrelated fixture work.

## Execution

- Wait for terminal native-full-2, preserve complete failures and fingerprints.
- Archive exact affected inputs and expand the 0.25 scope; preserve previous source
  and prepared-consumer bindings rather than overwriting their provenance.
- Add failing CLI/MCP tests; run focused red serially with locked/offline Cargo/jobs1.
- Change only automation paragraph input/defaulting/metadata, MCP schema and docs.
- Run focused CLI/MCP, font and nested paragraph tests; independently check fitted
  and echoed rasters and source/neighbors. Diagnose every failure before proceeding.
- Freeze corrected inputs, rebind verification/package consumers to a fresh native
  gate and source fingerprint, then resume the existing portable plan. Do not rerun
  unaffected frontend/packaging checks absent an affected input or new concern.
- One final portable review after actual acceptance, one correction pass, no rereview.

No runtime correction, optimized package, acceptance, release or parity is claimed
by this design document.

Native comparison correction: nested_text_automation:389 compares an unanchored native request against an implicitly source-anchored CLI request. Match the native input to the source anchor, including the equivalent constructor in nested_paragraph_automation. Keep exact rendered-byte equality; do not add tolerances or remove cases. Logged first mismatch:365pixels in [215,363,258,382], recorded in `artifacts/portable-025-20261029/nested-oracle-diagnosis-1`. Both corrected constructors passed the first focused run; complete corrected acceptance remains pending.

Public descriptions explicitly state that only the baseline axis uses baselineAnchor; the area/alignment still determine horizontal placement. Quarter-turned pages use displayed x for the baseline axis.
