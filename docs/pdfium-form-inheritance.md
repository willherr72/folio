# Inherited text state in PDFium forms

## Original binary diagnosis

The previous upstream Windows x64 PDFium chromium/8044 DLL produces different text geometry
for equivalent PDFs when a child form inherits `TL` (leading), `Ts` (rise), or
`Tz` (horizontal scale), compared with explicitly setting the same value in that
child. These tests load the DLL directly with ctypes; Folio's parser, page import,
editing and export code are not involved.

`scripts/probe-pdfium-form-inheritance.py` generates 18 original synthetic pairs:
nine state cases, each at one and two form levels, with q/Q restoration. The
leading/rise/scale/combined cases differ in PDFium (eight pairs). Character
spacing, word spacing, font size, fill color and stroke-mode controls match (ten
pairs). MuPDF matches every pair. This compares each renderer with itself; pixel
equality between different rendering engines is not required.

Evidence: `artifacts/form-inherited-state-20261024/diagnostic-corrected/results.json`,
including hashes, character origins and rendered PNGs. DLL SHA-256:
`04100c03e41cac1f979e36e5e26fb860bcb5a7461f53830d3c098716624a27a9`.

```powershell
# Corrected development dependency: all 18 pairs must pass normal acceptance.
python scripts/probe-pdfium-form-inheritance.py --output artifacts/inheritance-new-normal
# Historical diagnosis against the preserved original binary only.
python scripts/probe-pdfium-form-inheritance.py --dll artifacts/form-renderer-fix-20261024/previous-dependency/pdfium.dll --output artifacts/inheritance-new-diagnosis --expect-known-defects
```

Use a new or empty output folder; existing evidence is never overwritten. The
diagnostic success is not a claim that the renderer is correct. After correcting
the dependency, acceptance must use normal mode and require all 18 pairs equal.
The probe also requires nonblank output and the expected native text, and checks
bitmap-fill success. Fault-injection checks demonstrated that blank output and
failed bitmap initialization are rejected. Earlier pre-correction evidence is
retained separately.

## Source explanation

In [the pinned parser source](https://pdfium.googlesource.com/pdfium/+/f91ca5a72358bb0b00b4da9481b21fe668157614/core/fpdfapi/page/cpdf_streamcontentparser.cpp),
`AddForm` constructs a default `CPDF_AllStates` and copies general, graph, color
and text subobjects. The three scalar fields are separate members of
[CPDF_AllStates](https://pdfium.googlesource.com/pdfium/+/f91ca5a72358bb0b00b4da9481b21fe668157614/core/fpdfapi/page/cpdf_allstates.h)
and retain defaults. This explains the isolated results; it is not evidence about
every renderer version, transparency group, font type, annotation or form field.
Local copies of the relevant upstream files are retained in the artifact folder.

Folio's source/validation-page comparison caught these mismatches. The original
diagnosis checkpoint added refusal/source-preservation regressions without changing
the product DLL. Those cases became positive editing tests only after the corrected
renderer passed the unchanged appearance checks.

## Verified development correction

A pinned source patch now transfers just the three omitted scalar values into
child form state. Parser matrices/text cursors remain independent. User PDF streams
are not rewritten to compensate, and Folio's appearance comparison is unchanged.

The pinned Chromium build revision is
`2cd94c0abeada712aeea6906d99021913c9fc28d`. Its toolchain requires Windows SDK
10.0.28000.0. The build assembles a private SDK from signature-verified, hash-pinned
Microsoft NuGet packages and references installed MSVC 14.44.35207. It does not
alter the machine's installed SDK/registry. See [the source recipe](building-pdfium.md).

Corrected DLL SHA-256:
`7aaf99fff2ff089444832b0d481eb870b3c391262d0cff7bf1f53f93271728e6`.
The otherwise identical unpatched source build reproduced all eight differences;
its 36 probe PNGs matched the prior binary exactly. The correction passes all 18
normal-mode pairs. Fifteen native font/state/transform combinations pass real
editing, export and independent MuPDF/pypdf reference checks. The full native run
passes **703 tests, zero failures, seven ignored**; an additional opt-in Microsoft
Print to PDF test passes actual GDI spooling/reopening.

The new debug checkpoint passes Windows editing/undo/redo/save/reopen/restore for
standard leading, embedded combined state and mirrored CID text, plus form filling,
native comment editing, PNG export and native print-dialog cancellation. Its loaded
module was verified to be the DLL in its own resource directory. Independent checks
confirm text pixels/glyphs, untouched second pages, changed form appearances, comment
contents and image dimensions. The old executable/DLL pair remains in
`artifacts/form-renderer-fix-20261024/previous-working/`.

Evidence and exact executable identity:
`artifacts/form-renderer-fix-20261024/verification.json`. Setup now refuses baseline
downgrades, and build fingerprints include the actual DLL/provenance/patch/source
lock. One scoped review found no actionable defects; later portable copy/hash
checks have coordinator regression evidence outside that review snapshot.

Limits: the recipe was exercised with the populated pinned cache, not an entirely
empty checkout; no release ZIP or physical-printer output is approved. Opt-in
private/large-corpus cases remain separate. This fixes the demonstrated renderer
defect and does not establish universal PDF compatibility or Adobe/Foxit parity.
