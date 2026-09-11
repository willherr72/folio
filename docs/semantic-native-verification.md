# Native semantic PDF: bounded verification

The actual native outline-plus-Type3 representation passes exact logical Unicode
and approximate PDFium cluster bounds for the tested unmixed fixtures. It still
does not provide identical cluster selection geometry in MuPDF. Mixed-direction
text is explicitly refused. This is an experimental serialization result, not
production editor support or complete support for any script.

## Reproduce and inspect

Generate fresh native artifacts with the `shaped-text` feature and
`shaped-text-probe --semantic`, then run:

```powershell
python scripts/inspect-semantic-native.py --check-invariants
```

Defaults read `artifacts/shaped-text/semantic-native` and write the detailed
`artifacts/shaped-text/semantic-native-independent.json` plus the tracked compact
[`semantic-native-verification.json`](semantic-native-verification.json).
The compact report contains every raw reader string before/after export, PDF
hashes, retained source font hashes, MuPDF raster hashes, and maximum geometry
differences for all 32 cases. The detailed report retains individual boxes and
structure checks. Input PDFs are not modified by this inspector.

Native shaping is HarfRust 0.13.3 with Folio's bounded-status patch, tracking
HarfBuzz 14.3.1. Independent readers are PyMuPDF 1.27.1 and pypdf 6.14.2. The
bundled PDFium DLL SHA256 is
`04100c03e41cac1f979e36e5e26fb860bcb5a7461f53830d3c098716624a27a9`.
PDFium text, displayed geometry, and raster hashes come from the actual engine
example; the inspector independently reads MuPDF/pypdf and PDF object structure.

## Measured results

Eight samples, each at 0°, 90°, 180°, and 270°: ligatures on/off, composed and
decomposed accents, two-axis marks, Arabic, Devanagari, and supplementary U+1D434.
The four mixed Latin/Arabic/digit requests are recorded separately as refusals.

| Check | Result |
| --- | --- |
| Exact Unicode in PDFium, MuPDF, and pypdf, before and after engine export | 32/32 |
| Complete original font bytes retained, matching native layout SHA256, as an unused Type0 resource | 32/32 original and 32/32 exported PDFs |
| One Type3 font, only blank `d1` character programs, one text object and one `TJ`, no ActualText | 32/32 original and 32/32 exported PDFs |
| Visible vector fills present; original font never selected by a text operator | 32/32 original and 32/32 exported PDFs |
| Logical scalar count/order, including one combined supplementary scalar in engine geometry | 32/32 |
| PDFium displayed cluster-box tolerance, correctly transformed at each rotation | 32/32 |
| Raw strings and PDFium/MuPDF geometry unchanged after export | 32/32 |
| PDFium engine raster hashes and independently measured MuPDF 2x raster hashes unchanged after export | 32/32 |
| Mixed-direction refusal at all quarter turns | 4/4 |

Exactness ignores only final CR/LF reader terminators. It does not reverse text,
remove internal spaces, normalize accents, replace controls, or otherwise repair
reader output. MuPDF adds its normal trailing newline; raw values remain in JSON.

The maximum PDFium cluster-box edge difference is **0.012010 points**, rounding
up the measured 0.01200901-point maximum. This compares the engine's displayed
rectangles against native cluster advance-plus-ink unions using the actual page
box and quarter-turn transform. The check uses a 0.06-point tolerance for these
24-point fixtures; it is not a scale-independent production guarantee. Blank
whitespace glyphs are recorded separately because the engine intentionally uses
loose font bounds for whitespace. They do not establish tight visible-ink bounds.

Repeated scalar boxes within combining clusters and ligature interiors represent
coarse cluster selection. They are not separate caret positions. The supplementary
fixture demonstrates that UTF-16 surrogate pairs become one source scalar in
the engine's selection geometry without changing the reader text.

## Remaining MuPDF geometry difference

The native Type3 font uses a bounded font-wide rectangle instead of the Python
prototype's oversized FontBBox. The previous 72-point height at a 24-point font
is gone. That fixes the exaggerated line height; it does not force MuPDF's
default text boxes to repeat each full cluster rectangle.

For `q\u0307\u0323`, the first MuPDF cell is about **5.088 points wide**, while
the full cluster union is **15.234375 points wide**. Its vertical bounds now match
the 27.65625-point native cluster height. For `किताब`, the first scalar cell is
about **12.264 points wide**, compared with the first cluster's **24.864 points**.
The largest default MuPDF cluster edge difference is **12.601 points**, rounding
up the measured 12.6000001-point maximum across these fixtures.

Therefore this gate establishes exact cross-reader Unicode, retained font
identity, preserved native geometry, and approximate PDFium cluster boxes.
It does not establish cross-reader cluster-selection equivalence, mixed bidi,
arbitrary long text, accessibility behavior, or production UI readiness. The
semantic encoding remains capped at 255 Unicode scalars and the visible outlines
remain separate from the blank Type3 text layer. Keeping original font bytes as
an unused resource preserves provenance; it does not make the outlined content
an ordinary existing font-backed text run suitable for replacement.

Raster preservation here compares each candidate before and after engine export.
It does not claim pixel identity between filled vector outlines and the original
hinted TrueType text rasterization. Resource-limit and hostile-font tests are a
separate native verification responsibility; this report does not replace them.
