# Page raster sampling correction

The previous goal turn was progress: bounded reusable component, corrected
surface-transition cap, focused/full-target evidence and a demonstrated display
blocker. Current checkpoint hashes match. No active job was inherited.

Resolve the actual displayed-pixel failures before connecting PageRaster to
PageView. Preserve the exact native PNG and all reference results. The prior
minimum-axis mip experiment matched only32/64 arrangements; the baseline canvas
matched14/64. Do not turn failed exact comparisons into passes by adding tolerance.

Investigate mip-level selection and canvas pixel snapping as separate causes.
Use captured source/edited and opaque/alpha PNGs with mutually exclusive candidate
and SVG-reference presentation. Record actual transforms, browser version and
input hashes. Primary Chromium/Skia source can explain hypotheses; the installed
browser's output determines acceptance. Do not claim the current main branch
source is an exact match to the installed version.

The preferred result is a bounded persistent surface with correct filtering and
placement across rotation/crop/zoom/DPR. Alternatives are an explicit display-size
resampling surface or another presentation mechanism if the current architecture
cannot meet fidelity. Do not paper over a display defect by disabling useful PDF
workflows. A display-sized surface may derive new pixels; the original/native PNG
remains untouched and source-pixel verification must remain distinct from display
verification. Preserve alpha/color semantics and pointer/focus/overlay ownership.

After a supported correction, add meaningful regression tests and actual browser
matrix evidence. Integrate into PageView only if display acceptance is established;
Windows interaction/resource acceptance and packaging remain separate gates.
If evidence changes the architecture, record the decision and concrete next
implementation instead of claiming a shipped fix. One final fresh scoped review
for the completed milestone; preserve package0.27 and source PDFs throughout.


## Source-backed alternative, added during diagnosis

The screen-grid/mip alternatives still fail. Chromium153's ServiceTransferCache
constructor accepts the force_gpu_mem_discardable_limit_bytes preference and
evicts unlocked entries against it; the corresponding command-line switch is
force-gpu-mem-discardable-limit-mb. Test64MiB in an owned exact0.27 WebView process
using the unchanged SVG page renderer and unchanged63-cycle resource budgets.
This is a real cache policy, not forced GC or a measurement-only cache purge.
Verify runtime behavior; source alone does not prove WebView accepts the policy.
If it passes constant/varied workflows and exact stock frames, prefer this smaller
production integration over a renderer that changes displayed text. Any native
startup implementation requires its own argument-ownership tests and fresh build.
The old package verifier requires current source equality and is inappropriate
now that unused modules/tests were added. Instead bind the known accepted archive
SHA256 and use the unchanged accepted archive/extracted-file verifier to check all
package files. Explicitly test the frozen0.27 binary, not current-source parity.

64MiB failed the unchanged growth limit.32MiB passed both constant/varied63-cycle
experiments; preserve exact-screen differences from the constant run as unresolved,
not accepted via tolerance. Product candidate keeps the default SVG path and adds
the32MiB switch while explicitly retaining wry default switches. Verify the fresh
binary using a CDP-only environment, actual owned browser arguments and unchanged
resource oracles. A fresh build and repeat interactions are necessary; no native
engine change occurred and no full-engine-suite rerun is represented. Broader
large-image/many-tab performance and commercial quality remain separate gates.
