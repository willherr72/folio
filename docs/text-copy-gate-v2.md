# Selection-aware copy gate and form-field feasibility

The user confirmed that their Foxit testing works. This is retained as a
user-reported result; no exact viewer version or clipboard report was provided.
It does not imply that the separately reproduced PDFium/MuPDF failures passed.

## Gate correction

The native text-area inspector now measures three explicitly named operations:

- PDFium: fresh `FPDFText_GetText` over the full character range of each PDF.
- MuPDF: `fz_copy_selection` between unrotated page corners, with `crlf=0`.
- pypdf: page extraction, retained as a conservative compatibility requirement
  even though pypdf has no interactive clipboard API.

Each must equal the original logical Unicode before **and** after save/reopen.
The inspector never trims or normalizes strings. Whole-page extraction remains
in `actualText`/`resavedText`; actual selection/range results are separately stored
as `copyText`/`resavedCopyText`. A missing selection result fails rather than falling
back to page extraction. MuPDF's `crlf=1` selection is retained separately, not
silently substituted based on which setting passes. The method and endpoints are
recorded so this cannot be mistaken for a completed interactive viewer test.

PDFium text is reread from the original and resaved files and checked against the
native fixture manifest. Selection results now participate in roundtrip stability.
Previously a terminal newline from MuPDF page serialization could fail even an
exact selection; it no longer determines the copy result. An extra newline in the
**selection itself** still fails.

Sixteen inspector tests pass, including the previously failing terminal-newline
control, missing selection evidence, collapsed spaces and resaved copying changes.
The three tagged-selection and four clipboard tests also pass (23 total). The
seven native exports still produce **zero passing complete copy cases**. Their
source-range, raster and save/reopen evidence remains valid. Default inspection
exits 1; diagnostic mode may report the valid investigation with the gate closed.

```powershell
python -B -X utf8 scripts/test_inspect_text_area.py
python -B -X utf8 scripts/inspect-text-area.py --expected-cases 9
# Expected failure for the current specimens; not a release approval.
```

The report now identifies `copyEvidenceVersion: 2` and reports
`exactSelectionAndExtractionCases` separately from the historical
`exactLogicalCopyCases` page-text metric. The gate remains a necessary check, not
proof of correct drag selection, every external viewer, or complete area acceptance.

## Alternate representation: editable AcroForm (throwaway prototype)

A multiline form stores the logical value independently of its painted wrapping.
Three synthetic Latin/Helvetica specimens were created: repeated spaces with a
soft wrap, explicit LF line breaks including a blank row, and leading/trailing
spaces. Each was saved and independently reopened/resaved by MuPDF. MuPDF and
pypdf both read the exact field value in all six files; all three before/after
appearance pairs have identical raster hashes.

A separate process then used the bundled PDFium's version-1 form-fill interface:
load page, focus the actual widget, `FORM_SelectAllText`, and
`FORM_GetSelectedText`. This does not read the Windows clipboard or automate the
desktop. Results:

| Input | PDFium form selection, original and resaved |
| --- | --- |
| Repeated spaces and soft wrap | Exact: both ASCII spaces retained, no wrap newline |
| Leading/trailing spaces | Exact: all edge spaces retained |
| Explicit LF breaks and blank row | Fails exactness: LF becomes CRLF |

This is useful feasibility evidence, **not a solution approved for integration**.
Field-value equality alone would have missed the newline conversion in actual
form selection. The prototype changes the interaction to clicking inside a
fillable field; external viewers can edit it. It does not preserve ordinary PDF
page-text selection, does not establish complex-font/shaping behavior on focus,
does not test native Folio export/recovery, and has not been tested interactively
in Foxit or Acrobat. Flattening would discard the form interaction whose copying
was tested. No JavaScript, actions or proprietary viewer dependency were added.

[Recorded prototype results](issue16-form-field-feasibility.json) include raw
values, raw form-selection strings, file hashes and before/after raster hashes.
The throwaway PDFs and probe are local development artifacts under
`artifacts/text-area-form-prototype/`. They are not release assets or a new app mode.
Ordinary page-text selection remains the product behavior while the alternative
interaction is under discussion. Exact external copying remains the requirement.

References: [PDFium form selection test](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/4714/fpdfsdk/fpdf_formfill_embeddertest.cpp),
[PyMuPDF widget API](https://pymupdf.readthedocs.io/en/latest/widget.html).
