# Preserve fractional form border widths

Before field border/background controls, correct the existing renderer's loss of
fractional widget border widths during appearance generation and interactive filling.
The bundled DLL produces no border for `/BS /W 0.5` and a one-unit border for 1.5;
the synthetic evidence is in `artifacts/form-border-precision-20261026/diagnosis`.

## Approach

Use a small fifth patch to the pinned chromium/8044 source: carry floating-point
widths from `CPDFSDK_BAAnnot` through `CPWL_Wnd` creation and drawing. Keep the
existing Border/BS precedence, missing-width default (1), integer setter and public
C ABI. Numeric source widths must survive generation, focus/edit/blur, and save.
This is an implementation decision under the user's autonomous-work instruction.

Alternatives were post-processing native output and limiting the controls to
integer widths. Post-processing cannot repair the live editor's inset or clipping;
integer-only controls still lose one-point borders on scaled pages. Neither fixes
existing PDFs.

## Scope and constraints

- Separate candidate DLL, exact source hashes, patch/probe provenance and preserved
  old runtime/build. Do not install until direct and focused native checks pass.
- Test fractional and integer controls in text/choice/button appearances, modern
  BS and legacy Border arrays, scaled page units, and actual text replacement.
- Keep all four prior corrections. Run their direct probes on the candidate.
- Verify original saved answers and geometry through an independent PDF reader.
- After installing, run the full native suite and a fresh debug desktop build;
  exercise field filling/save/reopen. Keep any failures and scope limits explicit.
- No UI additions, custom-artwork rewriting, new border-style semantics, relaxed
  reflow admission, JS/XFA support, release or parity claim in this prerequisite.
- One final scoped review; inline corrections, no rereview or commits.

Malformed nonnumeric widths are outside valid numeric precision claims. Preserve
their old conversion when practical; avoid introducing nonfinite layout values.
Following the reproduced review defect, retain the old integer fallback for
negative, nonfinite and values outside the signed-integer range. Use the existing
PDF decimal writer for all SDK border-width output; scientific notation is not a
valid PDF numeric operand. This adds `cpdfsdk_appstream.cpp` to the patch scope.
Broader border/background controls resume after this checkpoint.
