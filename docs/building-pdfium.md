# Building Folio's PDFium dependency

Folio uses the pinned chromium/8044 source with corrections for inherited form
text state, fractional horizontal font advances, ordered default form colors, and
original embedded form-font selection, fractional widget border widths, and
horizontal Form-space text separators. The upstream binary loses
inherited leading, rise and horizontal scale and truncates fractional font widths.
Its default appearance lookup also chooses the first gray/RGB/CMYK occurrence rather
than the final effective color. Its form charset heuristic can also substitute a
system font despite a valid glyph in the original embedded face. Native widget generation and interactive
layout truncate fractional border widths, removing sub-unit borders on fill.
Some line-break comparisons also mix local coordinates from different Form
invocations, changing extracted separators when equivalent text becomes page text.
Setup verifies all six patches and rejects older
dependencies.
`scripts/pdfium-source-lock.json` records the revisions, downloads, patches,
modified source hashes, public probe fixtures and GN arguments.

## Prerequisites

Use Windows x64, Python 3.11 or newer, Git, Ninja, and Visual Studio 2022 C++ build
tools with MSVC **14.44.35207**. Install the Python probe dependencies with
`python -m pip install -r scripts/corpus-requirements.txt`.

The build downloads pinned Microsoft SDK NuGet packages and verifies their
signatures and SHA-256s. It assembles a private SDK, without elevation, registry
changes or replacing your installed SDK. It uses junctions to your existing
licensed MSVC installation. Never recursively delete those junction targets.
Allow substantial disk space and time for the first source build.

From the repository root in PowerShell:

```powershell
./scripts/build-pdfium.ps1 -OutputDirectory artifacts/pdfium-candidate
```

The wrapper uses a short `R:` mapping to avoid upstream Windows path limits.
Choose another unused letter with `-BuildDrive S` if necessary. It rejects a drive
belonging to another directory and only removes mappings created by that call.
An existing cache is checked against the lock; unexpected edits are preserved
and rejected. Interrupted downloads are preserved as `.download` files.
Use a new output directory for each candidate; existing results are never replaced.

The recipe builds a separate DLL, copies license notices, and runs all 18 direct
inheritance comparisons plus 10 fractional/integer width comparisons against
independent references, two native AllCap fallback controls, and 33 ordered/canonical form-color
comparisons, plus18embedded form-font controls across dropdown, list and text
fields. Another 130 checks cover generated and filled text/choice/checkbox/radio
border geometry, scaled pages, legacy arrays, indirect widths, missing defaults,
and boolean/null fallback controls. Independent MuPDF border-band comparisons cover
rectangular borders; circular radio strokes have vector geometry assertions.
Another 210 raw-token cases cover overflowing/extreme numeric widths, legacy
fallbacks and valid decimal serialization for tiny widths across five border
styles, text fields and radio states. These checks inspect appearance streams;
they do not replace focused native-editor or Windows acceptance.
The font controls verify exact original CID text and retained missing-glyph,
out-of-range-glyph and nonembedded fallback behavior. The form probes use a null-callback, script-disabled form environment
with the pinned x64 FPDF_FORMFILLINFO v2 layout. It writes
`provenance.json` only after all probes succeed.
The recipe does not install the candidate.

The separator probe has 52 synthetic cases covering shared/nested Forms, mixed
page/Form text, same-line spacing, positive unequal scales, page rotations and
physical units. Degenerate/extreme transforms and excluded shear, reflection and
vertical controls compare against the installed DLL when one is available. The
correction only normalizes horizontal comparisons with positive axis-aligned Form
matrices and a safe finite inverse/result with strictly positive relative scales.
Compensated huge/tiny X and Y controls reject collapsed relative axes. It changes some previously inaccurate
generated spaces into line breaks; actual glyphs and rendering remain unchanged.
It does not relax any of Folio's paragraph preservation checks.

## Acceptance and installation

Before replacing a working dependency, exercise the candidate explicitly:

```powershell
$env:FOLIO_TEST_PDFIUM_LIBRARY = (Resolve-Path artifacts/pdfium-candidate/pdfium.dll).Path
cargo test --manifest-path src-tauri/Cargo.toml --test pdfium_inheritance
cargo test --manifest-path src-tauri/Cargo.toml --test form_dependent_text
cargo test --manifest-path src-tauri/Cargo.toml --lib choice_probe_tests
cargo test --manifest-path src-tauri/Cargo.toml --test form_border_precision
Remove-Item Env:FOLIO_TEST_PDFIUM_LIBRARY
```

Preserve `src-tauri/resources/pdfium` by moving it to a new backup directory under
`artifacts`. Copy the candidate's `pdfium.dll`, `provenance.json`, `LICENSE`,
`VERSION`, and `licenses` directory to a new `src-tauri/resources/pdfium` directory.
Keep probe outputs in artifacts; they are not runtime resources. Then run:

```powershell
./scripts/fetch-pdfium.ps1
cargo test --manifest-path src-tauri/Cargo.toml --lib --tests --no-fail-fast
npm run tauri -- build --debug --no-bundle
```

Keep the prior accepted executable. Native tests and the direct probe do not
replace Windows open/edit/undo/save/reopen, form, annotation, image-export and
printing acceptance. Record those results before release. Build fingerprints must
use `scripts/build-fingerprint.py` so changes to the actual DLL, provenance, patches
and source lock invalidate acceptance alongside product source changes.

The source recipe has been exercised against the populated pinned build cache.
A completely empty-cache end-to-end rebuild remains a separate validation item.

Known minor limit: malformed boolean width objects may be interpreted as zero by
the precise getter instead of the legacy integer value one. Such malformed fonts
are not covered by a claim of identical fallback rendering. Valid numeric widths,
including the uppercase fallback cache, have dedicated controls.

Color lookup scans linearly with at most four retained numeric operands. It skips
escaped/nested literal strings locally without changing font parsing. Finite device
components are clipped to [0,1]; invalid or nonfinite numbers discard pending
operands. This is not an absolute native token/time budget or a general color-space
interpreter.

The form-font correction prefers the original embedded horizontal face only when
the requested BMP character round-trips through its PDF code and maps to a nonzero
glyph within that face. It does not add supplementary-character or shaping support.
Choice editing additionally proves resource mappings and native generated text;
font selection alone is not evidence of layout fidelity.

The border correction preserves numeric widths through the internal SDK and PWL
editor. It retains the existing Border/BS precedence, missing-width default and
nonnumeric integer fallback. Negative, nonfinite and out-of-range numeric values
also retain their legacy integer fallback; ordinary positive fractions are kept.
Border line widths use PDF decimal serialization, avoiding invalid scientific
notation for tiny or large values. Public C signatures and the legacy integer setter
are unchanged. It does not authorize rewriting custom appearances or relax
Folio's conservative text-reflow admission checks.
