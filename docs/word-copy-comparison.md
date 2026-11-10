# Word export comparison for multiline copying

On October 22, 2026, the September copying failures were reproduced and compared
with Microsoft Word PDF exports. Word exhibits the same repeated-space,
automatic-wrap and short hard-break failures in the tested reader operations.
Tagged output does not change those results. An ordinary established exporter
therefore does not supply a passing reference for Folio's strict requirement.

This is a completed diagnostic milestone, not a fix or release approval. After
reviewing it, the user approved the [revised copying contract](text-area-copy-policy.md):
exact text within Folio with specified whitespace differences permitted in external
readers. The strict results below remain historical raw-equality evidence.

## Reproduced baseline

The existing 40-case multiline probe regenerated 80 original/resaved PDFs with
the same pinned PDFium DLL, MuPDF 1.27.1 and pypdf 6.14.2. All historical text,
character geometry, selection points, raster hashes, gate results and original
PDF hashes match. The only 40 report differences are resaved file hashes; those
files are not claimed byte-identical to September's artifacts.

The real-row candidate still passes complete copying in only 4/20 cases, all
single-line controls. Its character origins match in 20/20 cases. The old
representation still passes origins in only 8/20. All 25 focused diagnostic tests
pass: five selection, four clipboard and sixteen native-area inspector tests.

Fresh inspection of the seven **previously generated** native-area original/resaved
pairs also returns zero exact-copy passes and the expected strict-gate exit 1.
Their source-range and roundtrip evidence remains valid. This reinspection does
not constitute a fresh native export, build or desktop test.

## Word control method

Microsoft Word for Microsoft 365, COM version 16.0, build 16.0.20430, created six
new documents in a hidden isolated instance. Each contains one specimen in
Courier New, nominal 20 pt, on a 240 by 180 pt page with 28 pt line spacing.
Wrapping uses a 54 pt text width; the other controls use 144 pt. The generator
checks the exact Word source text, one-page output and expected one/two-line count
before exporting. Logical LF is authored as Word's manual line break U+000B;
Word's compulsory final paragraph mark is excluded from the logical input.

Each document is exported with structure tags enabled and disabled through
[Word's ExportAsFixedFormat API](https://learn.microsoft.com/en-us/office/vba/api/word.document.exportasfixedformat).
The inspector confirms the structure tree is respectively present or absent.
Twelve original PDFs and twelve MuPDF-resaved PDFs are measured using the same
operations as the existing investigation: PDFium full character-range text,
MuPDF page-corner selection with LF mode, and pypdf extraction. MuPDF CRLF
selection and page extraction remain separate diagnostics. No string is trimmed
or normalized. The PDF and DOCX hashes are verified against the generator manifest.

The Word files use ordinary visible embedded font text. They are a comparison
of equivalent logical inputs and wrapping behavior, not a pixel-identical match
to Folio's synthetic glyph specimens. Word controls cover rotation zero only.

## Raw results

JSON notation below makes every space and newline explicit. Tagged and untagged
results agree in all six controls, including geometry and raster.

| Logical input | PDFium range text | MuPDF LF selection and pypdf extraction |
| --- | --- | --- |
| `"A B"` | `"A B"` | `"A B "` |
| `"A  B"` | `"A B"` | `"A  B "` |
| `"A B C D"`, automatic wrap | `"A B \r\nC D"` | `"A B \nC D "` |
| `"A  B C D"`, automatic wrap | `"A B \r\nC D"` | `"A  B \nC D "` |
| `"A\nB"`, explicit break | `"AB"` | `"A \nB "` |
| `"A B\nC D"`, explicit break | `"A B\r\nC D"` | `"A B \nC D "` |

All 12 original/resaved pairs retain exact measured text, character geometry,
selection endpoints and raster. None passes the complete copy gate. Even Word's
single-line control fails because its PDF contains a separate trailing-space
text object, retained by MuPDF and pypdf. This is visible directly in the decoded
content stream as `[( )] TJ`; the original Word source has no trailing space.

The automatically wrapped and explicitly broken `A B` / `C D` pair has identical
raster output. MuPDF selection and pypdf extraction cannot distinguish the two
logical inputs in this test. PDFium distinguishes them only by a space before
its inferred CRLF; neither result equals the automatic-wrap source. Their PDF
content streams and character records are not identical.

[Full raw results, geometry, hashes, methods and manifest](issue16-word-copy-comparison.json).
No interactive Foxit, Chrome or Acrobat clipboard result is inferred from these
engine operations. Resaving uses MuPDF, not Folio's native save/recovery pipeline.

## Why this changes the next step

The pinned PDFium source explicitly deletes repeated ASCII spaces in
[`CPDF_TextPage::CloseTempLine`](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fpdftext/cpdf_textpage.cpp#816).
The fetched source SHA-256 is
`2cd51fcce2948568b7c7dfdcde72f6020e8c9581ac71f86eb5d3dc8c4f1988e4`.
The same code inserts CRLF when it infers a line break. Word's controls establish
that these failures also occur outside Folio's experimental encoding. They do not
prove that every possible PDF encoding, reader version or interaction must fail.

Further unbounded encoding experiments had no demonstrated path to meeting the
strict external-copy requirement. The user approved permitting documented
external-reader whitespace differences while requiring exact logical text within
Folio and through editable save/reopen/recovery. The [current policy](text-area-copy-policy.md)
defines those differences and rejects lost content or joined words. That verifier
change is separate from the measurements in this report; the production writer
and desktop text-area integration remain unchanged.

## Reproduction

Run from the repository root with the pinned PDFium DLL under
`src-tauri/resources/pdfium`, Python with PyMuPDF 1.27.1, pypdf 6.14.2 and pywin32,
and installed Microsoft Word. Choose fresh output directories; generators and
the Word inspector refuse existing ones. Word startup may take roughly a minute
on this machine. The generator leaves the system clipboard untouched and closes
its own documents and application instance.

```powershell
python -B -X utf8 scripts/probe-multiline-selection.py --output artifacts/restart-baseline
python -B -X utf8 scripts/test_probe_tagged_selection.py
python -B -X utf8 scripts/test_reader_clipboard.py
python -B -X utf8 scripts/test_inspect_text_area.py
python -B -X utf8 scripts/probe-word-copy.py --output artifacts/word-copy-controls
python -B -X utf8 scripts/inspect-word-copy.py --input artifacts/word-copy-controls --output artifacts/word-copy-results
```

The Word inspector's exit 0 means the comparison completed and roundtrip checks
passed; each `exactCopyGate` remains false and `releaseAcceptance` is false.
Generated DOCX/PDF files and decoded content streams stay in ignored `artifacts/`.
The measured run used `artifacts/restart-2026-10-22/word-controls-54pt` and
`artifacts/restart-2026-10-22/word-reader-results-final`.
