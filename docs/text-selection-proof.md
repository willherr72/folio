# External selection and tagged-PDF investigation (#14 / #16)

Follow-up: the [version-2 copy gate](text-copy-gate-v2.md) now measures actual
MuPDF selection in the native-area inspector. Historical page-extraction results
below are retained; they are no longer substituted for selection evidence.

This continues the strict external-copy requirement after `c39a14c`. No desktop
feature or production writer changes, and no release acceptance is claimed.

## What the new evidence establishes

Whole-page extraction is not identical to selection copying. With the same simple
`A B` PDF, MuPDF's page extractor returns `A B\n`, while its native
`fz_copy_selection` returns exactly `A B`. Neither result is normalized. This
distinction matters: the terminal newline is not evidence that interactive copy
adds that character. The existing strict page-extraction verifier is unchanged;
these diagnostic observations do not turn it green or relax the user's rule.

The single-TJ candidate has a separate geometry problem. It encodes row movement
inside Type3 glyph bounds while keeping text origins on one baseline. PDFium's
cell boxes can match the painted rows, but MuPDF's default character origins and
font-wide bounds overlap across rows. In the soft-wrap control `A B C D`, selection
between the page corners returns only `C D`. This is a real engine-selection
failure for those recorded endpoints, not a full interactive viewer test. The
retained raw character origins and bounds make the problem inspectable.

Adding a connected tag tree does not repair default copying in the tested
readers. The fixture connects Catalog → StructTreeRoot → Document → P → Span →
MCID 0, with page/parent-tree back references; vector ink is marked as an artifact.
Variants put the exact logical string on the structure Span, marked-content Span,
both, or neither. This is a synthetic tagging test, not a PDF/UA conformance claim.

## Matrix

Five inputs × four rotations × five tagging modes = **100 PDFs**. All retain the
reference raster exactly. Every group below contains 20 cases.

| Tagging mode | Exact PDFium page text | Exact MuPDF selection (LF mode) | Exact pypdf page text | Exact PDFium cell boxes |
| --- | ---: | ---: | ---: | ---: |
| Untagged | 12 | 8 | 12 | 12 |
| Tags only | 12 | 8 | 12 | 12 |
| Structure ActualText | 12 | 8 | 12 | 12 |
| Marked-content ActualText | 8 | 8 | 12 | 4 |
| Both ActualText locations | 8 | 8 | 12 | 4 |

MuPDF selection passes the single-line and repeated-space inputs. PDFium still
collapses repeated spaces in every mode. No mode handles all five inputs correctly
across the measured engines. MuPDF page extraction adds a terminal newline in all
100 cases. CRLF selection mode is recorded independently. Structure-collection
mode also gets a separate result: its copy-selection call returns empty for these
tagged fixtures. That result does not establish what a structure-aware viewer or
assistive application would do; the default selection result is not replaced by
an opportunistically chosen mode.

[Raw strings, PDF hashes, character geometry and raster hashes](issue16-tagged-selection.json)
are retained. The scripts refuse an existing output directory. These are new
synthetic files, not native save/reopen or arbitrary imported-document evidence.

## Reproduce

Run from the repository root with the same Python/PDFium setup as the preceding
separator probe:

```powershell
python -B -X utf8 scripts/probe-tagged-selection.py --output artifacts/text-tagged-selection
python -B -X utf8 scripts/test_probe_tagged_selection.py
python -B -X utf8 scripts/test_reader_clipboard.py
python -B -X utf8 scripts/test_inspect_text_area.py
```

The selection tests use the real MuPDF API. Clipboard tests never read or write the
live clipboard. The application/native code is unchanged, so their previous
regression evidence is not claimed as a newly executed suite.

## Interactive clipboard procedure

Foxit is installed on the development machine. Its synthetic control was opened,
but no copy keystrokes were sent because the desktop was active. **No Foxit result
is claimed.** Acrobat and other interactive viewers remain unverified too.

1. Open `single-0-untagged.pdf` from the generated directory in the external viewer.
   Choose its text-selection tool, select the specimen, and copy it. Expected:
   `A B` with one ASCII space, no terminal newline.
2. Immediately run the command below, replacing the viewer label with its actual
   name/version. The checker reads `CF_UNICODETEXT` without changing the clipboard,
   verifies the PDF's manifest hash, and records exact characters and the first
   differing Unicode scalar. Exit 0 means that one comparison matched; exit 1
   means it differed. Neither is overall release acceptance.
3. Repeat with `spaces-0-untagged.pdf` (two ASCII spaces),
   `soft-0-untagged.pdf` (`A B C D`, no copied newline),
   `hard-0-untagged.pdf` (`A\nB`), and their `structure-actualtext` counterparts.
   Record whether drag-selection follows the visible rows separately. Do not
   copy the expected string from this document to produce a passing report.

```powershell
python -B -X utf8 scripts/check-reader-clipboard.py `
  --manifest artifacts/text-tagged-selection/results.json `
  --pdf single-0-untagged.pdf `
  --viewer "Foxit PDF Reader <installed version>" `
  --output artifacts/foxit-single-copy.json
```

The checker relies on the operator to identify the viewer and copy action; it
cannot authenticate clipboard provenance. Reports may include unrelated clipboard
text if the operator copied the wrong thing, so keep them in ignored `artifacts/`
and review them before sharing. `--from-file` supports exact UTF-8 offline checks,
preserves CRLF bytes, and labels those results `offline-file` rather than clipboard
evidence. Existing reports cannot be overwritten.

## Consequence for the next implementation

The single-TJ/per-glyph-offset representation is not ready for production. Future
work must preserve real character origins in independent readers as well as the
painted bounds, and must distinguish measured clipboard behavior from page-text
serialization. Tag metadata alone has not solved these fixtures. Keep #14/#16
open and the text-area tool unreleased; changing Folio's reader cannot establish
identical copying in external applications.

References: [MuPDF selection API](https://mupdf.readthedocs.io/en/1.27.2/reference/javascript/types/StructuredText.html),
[PDF Association tagging guidance](https://pdfa.org/download-area/publications/Tagged-PDF-Best-Practice-Guide.pdf),
[Foxit Reader manual](https://cdn01.foxitsoftware.com/pub/foxit/manual/reader/en_us/foxit-pdf-reader-user-manual-2026.1.1.pdf).
