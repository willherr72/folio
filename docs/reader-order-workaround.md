# Rotated-page reading order

The #14 font-bank failure reduces to ordinary Courier text: one text-show
operation containing `ABCDEF` copies correctly, while three adjacent operations
containing `AB`, `CD`, and `EF` copy as `EF CD AB` with `/Rotate 180`.
No font switch, Type3 font, shaping, or embedded font is required.

## Cause and controls

In pinned PDFium revision
`f91ca5a72358bb0b00b4da9481b21fe668157614` (chromium/8044),
[ProcessTextObject sorts objects by display X](https://pdfium.googlesource.com/pdfium/+/f91ca5a72358bb0b00b4da9481b21fe668157614/core/fpdftext/cpdf_textpage.cpp#920)
after applying the rotated page display matrix. Each text-show operation creates
an object. Rotation can reverse object order while leaving characters inside
each object unchanged; subsequent gap processing inserts spaces.

The [48-case matrix](reader-order-minimal.json) varies Courier/Type3, one or
several TJ operations, unchanged font state, aliased or distinct font resources,
explicit positioning, and all four quarter turns. Raw PDFium is exact in 38/48
cases; MuPDF and pypdf are exact in 48/48. Removing rotation from an independently
loaded diagnostic page makes PDFium exact in 48/48. Original files are untouched.

That is not a general fix. The [eight counterrotation controls](reader-order-counterrotation.json)
use a negative text matrix: raw PDFium is exact in six cases, but unconditionally
removing page rotation breaks all eight. The native regression also catches
multi-line ordering differences at 90/270 degrees between visually equivalent
single-show and split-show lines.

## Folio's conservative workaround

`page_text`, used by the selectable reader and search, reads an eligible rotated
page from a temporary one-page document with rotation zero. It maps character
boxes using the **original** crop and rotation. The open source document is never
modified, including its inherited page dictionary, and the temporary document
and all imported resources are dropped after the request.

Eligibility deliberately requires:

- At most 64 page objects and 4,096 extracted characters, bounding the wrapper's
  per-object character scans.
- At least two text objects; every text object has positive finite font size and
  a finite, positive, axis-aligned matrix.
- Every text object has at least two printable ASCII characters with finite,
  constant-Y origins, nondecreasing X, and a positive total advance.
- No form XObjects or unknown object types. Paths, images and shading do not
  establish text orientation.

Counterrotation, negative advances, vertical/mixed orientation, singleton text
objects, non-ASCII text and ambiguous/unavailable geometry retain the existing
PDFium path. This is a bounded workaround for simple forward ASCII text, not
general reading-order reconstruction or newly enabled complex-script input.

Unrotated pages avoid the scan/copy path. Rotated-page eligibility and copying
occur on backend requests; the existing frontend page-text cache avoids repeated
requests for warm pages. Copying an eligible page also copies its resource graph,
so image-heavy pages can incur additional transient memory and latency even with
the object/character caps. No new large-document performance claim is made.

## Verification and reproduction

Native regressions cover split versus joined text and each character's box,
multiple lines, all source rotations, nonzero crop origins, inherited rotation,
exact pre/post renders, unchanged raw extraction and source bytes, and native
save/reopen. Counterrotated text, negative spacing, singletons and mixed
orientation have explicit fallback checks. Three retained minimal PDFs are also
read repeatedly without changing their raw extraction.

The reader, annotation and text-measurement cache frontend tests pass (19 tests).
The complete native suite with `shaped-text` enabled passes 124 tests, with three
existing opt-in checks ignored. The subsequently added retained-PDF regression
is verified with a focused rerun of the geometry suite. Rust formatting and
whitespace checks pass. Independent review identified the counterrotation and
negative-advance hazards; the final conservative gate addresses both.
No frontend contract or rendering code changes. No packaged desktop smoke or new
release is claimed for this development checkpoint.

```powershell
python -B -X utf8 scripts/probe-reader-font-order.py --output artifacts/reader-order-new
python -B -X utf8 scripts/probe-reader-font-order.py --counterrotation --output artifacts/reader-counterrotation-new
cargo test -j 1 --offline --locked --manifest-path src-tauri/Cargo.toml --test text_geometry -- --nocapture
cargo test -j 1 --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --tests --lib
```

Probe output directories must be new. The three tiny retained PDFs have hashes
and provenance in [their fixture README](../tests/fixtures/reader-order/README.md).
Readers are bundled PDFium chromium/8044, PyMuPDF 1.27.1 and pypdf 6.14.2.

`extract_text` intentionally remains **raw PDFium** for interoperability checks.
Folio's reader correction must not turn external-reader failures into passing
writer evidence. The 255-definition writer limit, RTL boundaries, and the other
portable complex-text gates remain unchanged. No upstream PDFium change or
upstream issue was submitted by this work.
