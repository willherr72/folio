# Preserve form text color during filling

Prior cycle made concrete verified progress with text-field resize reflow. Current
ordinary fill still delegates to PDFium GetColor, which picks gray before RGB/CMYK
rather than the last effective color. Standing autonomous authorization applies.

Correct the pinned native renderer at CPDF_DefaultAppearance::GetColor. A bounded
single-pass token scan retains the last valid nonstroking DeviceGray/RGB/CMYK color.
Do not match color names inside names/strings/comments; consume numeric arguments
and clear operand state at other operators. Clip valid color components to0..1.
Default-appearance text state remains unchanged; do not rewrite source DA/fields.
Support valid repeated/mixed device color operators for text and choice controls,
including inherited defaults, shared widgets, saved output and later repeated edits.

Alternatives: rewriting/restoring field DA around filling requires temporary ownership
markers and inherited/shared mapping. Repairing saved AP commands after generation
risks matching unrelated drawing colors. Native correction addresses the actual shared
lookup and preserves document structure. Existing resize canonicalization remains.

Add third pinned renderer patch, update reproducible recipe/provenance/setup/fingerprint
checks, build separate candidate, verify direct color probe and prior native probes.
Preserve installed dependency until candidate acceptance, then archive old resources
and install explicitly. No old executables or PDFs overwritten. Native test failing
on old DLL must pass using candidate, covering text/combo/list, final repeated color,
inheritance/shared siblings, unchanged metadata/actions and save/reopen. Full native
regression after dependency install, frontend/build and Windows acceptance. One final
review with parent reproductions/corrections, no rereview. No parity/release claim.
