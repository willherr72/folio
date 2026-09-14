# Strict external-copy separator gate (#14 / #16)

Investigated after native layout checkpoint `10241d3`. The user explicitly
requires identical copying in external viewers before release. No production
writer, desktop feature, or downloadable release changes in this checkpoint.

## Root cause

The pinned PDFium chromium/8044 implementation's
[`CPDF_TextPage::CloseTempLine`](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fpdftext/cpdf_textpage.cpp#816)
removes consecutive ASCII spaces from both its temporary string and character
list before bidi processing. This explains why the repeated-space specimen loses
a character as well as copied text. The source URL and inspected SHA-256 are
retained in the [machine-readable evidence](issue16-separator-proof.json).
Replacing ASCII spaces with nonbreaking spaces would change the original Unicode
and does not meet the requirement.

## Bounded encoding matrix

The reproducible probe generates 80 small PDFs: five inputs, four page rotations
and four semantic encodings. Inputs cover a single-line control, repeated spaces,
soft wrapping, wrapping with repeated spaces, and a hard newline. Every candidate
paints the same independently defined vector letter ink; only the invisible
semantic layer differs. All 80 candidates retain the reference raster exactly.

| Encoding | Cases | All-reader exact text | All-reader text allowing one terminal EOL | Exact PDFium cell boxes |
| --- | ---: | ---: | ---: | ---: |
| Separate ordinary Courier objects | 20 | 0 | 3 | 0 |
| One TJ with scalar Type3 cells and per-cell vertical bounds | 20 | 0 | 4 | 12 |
| One TJ with repeated spaces expanded from one Unicode mapping | 20 | 0 | 4 | 12 |
| Whole logical ActualText around the single TJ | 20 | 0 | 4 | 4 |

Raw exact copying is reported separately from a diagnostic allowing just one
terminal CRLF/LF. The latter passes only single-line controls; it does not repair
any separator failure. Courier's real glyph boxes differ from the intentionally
uniform Type3 cell rectangles, so its zero cell-box total is a control result,
not a claim that ordinary Courier selection is broken.

For `A  B`, every strategy produces `A B` in raw PDFium, while MuPDF and pypdf
retain the repeated space. For soft-wrapped `A B C D`, the single-TJ scalar cells
retain exact text and all character boxes in PDFium at the tested rotations;
pypdf also retains the logical text. MuPDF still inserts a newline according to
the displayed rows. Whole ActualText does not solve the complete gate and loses
multirow character geometry. Hard-newline encodings likewise disagree between
readers. Raw strings and per-character boxes are retained without normalization.

These are synthetic representation probes, not newly supported text-area output.
They do not cover imported font semantics, native save/reopen, arbitrary viewer
versions, interactive Foxit/Acrobat selection, or all possible encodings. Existing
native layout/save/reopen evidence remains in the preceding checkpoint. A future
candidate needs exact Unicode plus preserved selection geometry and the full
integration/roundtrip matrix before it can replace the production writer.

## Enforced gate and verification

The existing area inspector now exits nonzero by default when external exact
copying fails. The seven exported native-layout fixtures correctly fail that
command. Explicit `--diagnostic` still succeeds for their source-range and
roundtrip evidence, while recording `exactCopyGatePassed: false`. Fourteen unit
tests pass, including new cases for collapsed spaces, inserted line breaks,
missing readers, empty evidence, misleading success flags and failed roundtrips.
Independent review verified all 80 artifact hashes and independent-reader
results, and found no blocker in the probe or strict-gate implementation. Rust/frontend
sources are unchanged, so their previously passed suites were not rerun.

```powershell
python -B -X utf8 scripts/probe-text-separators.py --output artifacts/text-separator-proof
python -B -X utf8 scripts/test_inspect_text_area.py
# Expected exit 1 with the current native fixtures:
python -B -X utf8 scripts/inspect-text-area.py --expected-cases 9
# Investigation only, not release acceptance:
python -B -X utf8 scripts/inspect-text-area.py --expected-cases 9 --diagnostic
```

The probe refuses an existing output directory. It uses the installed MuPDF
1.27.1/pypdf 6.14.2 and the bundled PDFium identified by its hash. The unresolved
copying requirement remains a release gate for #16; #14 retains the repeated-space
reader issue. No Folio-only clipboard replacement is accepted as its resolution.

Follow-up: [tagged PDF and native selection-copy investigation](text-selection-proof.md)
distinguishes page-extractor terminal newlines from selection behavior and exposes
MuPDF origin/selection failures in the single-TJ candidate. Passing PDFium cell
boxes alone must not be interpreted as cross-reader selection compatibility.
