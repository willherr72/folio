# Real line origins: missing-row selection fix (development prototype)

The user reports that single-line and repeated-space checks work in Foxit and
Chrome, while both multiline checks fail. The saved Foxit reports sharpen that
observation: `soft-0-untagged.pdf` produced `A B` instead of `A B C D`, and
`hard-0-untagged.pdf` produced `A ` instead of `A\nB`. Two positive reports both
identify the single-line fixture; repeated-space success and the Chrome results
remain user-reported rather than independently matched clipboard records. No new
manual testing was requested for this checkpoint.

## Cause and bounded fix

The old single-TJ experimental encoding moves glyph boxes vertically while keeping
all text origins on one baseline. Its oversized font-wide bounds also overlap the
rows. PDFium cell-box checks alone had not exposed that independent readers could
select only one of those rows.

The new experimental `positioned-cells` writer emits one text object per real row,
with an actual text-matrix baseline and tight ordinary Type3 bounds. An explicit
newline changes the baseline instead of being encoded as a fake text glyph.
The vector ink is identical to the old specimen; no production Folio writer or UI
was changed. Existing native text-area fixtures already use per-line overlays, so
this is a correction to the alternative representation, not a claimed fix to the
shipped application.

Two regression tests failed with the old representation and pass with the new
one. They check actual MuPDF selection, baseline positions, nonoverlapping row
bounds, all four rotations for soft wrapping, and both LF/CRLF hard-break copying.
All 25 focused inspector, selection and clipboard tests pass. Rerunning the old
80-case separator matrix produces identical PDFs and reader results.

## Results and remaining failure

The probe compares five inputs × four rotations × two representations, each
original and MuPDF-resaved: **40 cases, 80 PDFs**. All 40 preserve the reference
raster and retain exact reader text/geometry/raster through that save/reopen.
This roundtrip is MuPDF's save, not Folio's native export/recovery pipeline.

| Measurement | Old scalar-cell representation | Real row origins |
| --- | ---: | ---: |
| MuPDF character origins match painted cells | 8/20 | 20/20 |
| Exact copy in PDFium, MuPDF LF selection, and pypdf | 4/20 | 4/20 |

Only the single-line control passes the complete copying check. Geometry improves,
but **no multiline case meets strict cross-reader copying**. At rotation 0:

| Source | PDFium range text | MuPDF LF selection | pypdf extraction |
| --- | --- | --- | --- |
| `A B C D` with a soft wrap | `A B \r\nC D` | `A B \nC D` | `A B \nC D` |
| `A\nB` | `AB` | `A\nB` | `A\nB` |
| `A  B` | `A B` | `A  B` | `A  B` |

Both rows now participate in MuPDF selection; soft wraps still become literal
copied line breaks. PDFium retains both letters of the short hard-break sample
but does not infer their separator. Its repeated-space discrepancy also remains.
LF and CRLF results are recorded separately without normalization. The selected
page-corner endpoints and raw character boxes are retained; this is engine evidence,
not a fresh interactive Foxit/Chrome pass.

[Full original/resaved results](issue16-multiline-selection-origins.json).

## Reproduction

```powershell
python -B -X utf8 scripts/test_probe_tagged_selection.py
python -B -X utf8 scripts/probe-multiline-selection.py --output artifacts/multiline-selection-origins
```

The output directory must not exist. The old 80-case separator matrix is unchanged;
the new representation is exercised only by the dedicated multiline probe.
The existing selection-aware native-area verifier remains unchanged and closed.
No feature flag, version bump, release asset, or field-based interaction was added.

Real line origins are necessary for this candidate's selection geometry, but not
sufficient for preserving soft/hard logical separators. A subsequent representation
must pass both requirements together; restoring the overlapping-origin technique
to make an extraction string look better would reintroduce the missing-row defect.
