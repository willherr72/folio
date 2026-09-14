# Issue 14: wide semantic CID font probe

The tested single-font representation restores exact Unicode across all 28 cases, including the bank-boundary samples rotated 180 degrees, but **fails the existing cluster geometry tolerance**. A follow-up scale16 probe establishes precise coincident cluster boxes with source-explained PDFium top padding (see below). This probe changes no production code; widening still requires a native implementation and verification.

`scripts/probe-wide-semantic.py` copies retained native PDFs, preserves their visible content prefix and original embedded font, and replaces all blank Type3 banks with one Type0 Identity-H / CIDFontType2 font. A synthetic TrueType rectangle represents each original d1 bounding box; its advance is the original width. An explicit CIDToGIDMap and per-code ToUnicode map connect 16-bit codes to glyphs and Unicode. One TJ array preserves the original cells and numeric adjustments. `3 Tr` hides the synthetic rectangles.

Run `python -B -X utf8 scripts/probe-wide-semantic.py`. The default output is `artifacts/issue14-wide`. The tracked compact result is `docs/issue14-wide-semantic-probe.json`; the artifact results include unmodified reader strings, every character box, expected native cluster boxes, source hashes and copied PDF hashes. Geometry is compared in unrotated PDF coordinates; quarter-turn display transforms preserve the maximum edge error used here. PDFium uses the bundled DLL directly: both FPDFText_GetText and individual FPDFText_GetUnicode/GetCharBox are inspected. MuPDF uses raw character boxes; pypdf supplies text only.

The matrix uses retained `semantic-banks-native` samples labelled 255/256/511 (actually 256/257/512 total definitions), plus ligatures-on, decomposed marks, two-axis marks, and Arabic from `semantic-native`. Every case runs at 0/90/180/270 degrees.

| Measure | Result |
| --- | --- |
| Exact raw Unicode in PDFium, MuPDF and pypdf, ignoring final CR/LF only | 28/28 |
| MuPDF visible raster unchanged | 28/28 |
| PDFium native cluster edge error <= 0.06pt | 8/28 |
| MuPDF native cluster edge error <= 0.06pt | 0/28 |

Baseline original Type3 PDFium errors are 0.00189–0.00272pt for bank cases and 0.00562–0.01201pt for controls. The candidate errors are 0.05734pt for the 255/256 labels, 0.06439pt for 511, 0.26962pt for ligatures and two-axis marks, 0.29269pt for decomposed text, and 0.26400pt for Arabic. These values are identical across page rotations. MuPDF uses substantially different vertical/advance geometry (up to 38.832pt in these synthetic metrics), so its geometry does not pass either.

For a concrete regression, the first f in the ligature control has original PDFium box `[62.449219,80.953217,48.339844,66.579842]`; the candidate top edge becomes `66.843842`. The next f's left edge shifts from `62.454609` to `62.478607`; coincident cluster geometry also becomes less reliable. The original Type3 baseline confirms this is added by the candidate, not a layout comparator mismatch.

Further bounded probes also failed: `--upm 16384` produced exactly the same PDFium boxes; enlarging all glyph metrics, PDF widths and TJ adjustments by 10 or 16 while inversely reducing font size did not remove the expansion (scale16 511 error 0.06512pt; controls 0.26700–0.29869pt). Reversing rectangle contour winding also left boxes unchanged. Historical results are retained as `results-upm1000.json`, `results-upm16384.json`, `results-scale10.json`, and the `scale16`/`clockwise` artifact directories. Historical runs before the final raw-text instrumentation store PDFium per-character Unicode concatenation; the final default results additionally measure actual FPDFText_GetText. These variants establish negative evidence only. No PDFium resave or physical reader/UI acceptance is claimed.


## Source-explained padding and scale16 follow-up

The parent investigation located the cause in the pinned [PDFium GetCharBBox implementation](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fxge/cfx_face.cpp): after normalizing a non-tricky glyph box, PDFium adds integer `rect.top / 64` to its top edge. Higher UPM cannot remove this deliberate reader padding. We did not compensate by distorting the glyphs.

The scale16 candidate retains proper synthetic glyph rectangles. Its non-top native edge error is at most 0.002125pt in all 28 cases, and the maximum pairwise edge difference within one original cluster is also 0.002125pt, comfortably below the existing 0.02pt cluster grouping threshold. The original 0.06pt all-edge comparator continues to report failure because it compares the padded reader top against the unpadded original native box; that result is preserved rather than silently relabelled.

`artifacts/issue14-wide/model-pdfium-bounds.py` independently reads the actual embedded TTF rectangles and PDF widths/TJ positions. Normalizing each metric with truncation after adding 0.5, then adding integer top/64, predicts the observed scale16 short-control boxes within 0.0000046pt; long bank sample residual is at most 0.000676pt. This remaining long-line difference is consistent with accumulated reader float precision, but that cause is not proven by this probe. The model does not modify PDFs and does not increase the original native tolerance. Raw source FPDFText_GetText, all three readers' exact Unicode, and unchanged MuPDF rasters were rechecked for scale16: 28/28 each.

A native implementation could therefore evaluate scale16 against the documented reader padding and original-cluster coincidence, while retaining original Type3 for already supported text and preserving unsupported bidi guards. This is a candidate representation with bounded evidence, not complete reader acceptance. MuPDF raw boxes remain different and are not asserted equivalent. The actual scale16 model and per-case geometry metrics are included in the tracked JSON under `scale16_source_model`.


## Reproducible verifier and consistent global font metrics

The tracked verifier is `scripts/verify-wide-semantic-geometry.py`. It reads each actual PDF's TTF glyphs, CMap, widths and TJ positions, verifies the PDF hash against the reader report, and derives pure-direction logical order from the emitted Unicode. It contains no per-case expected-box templates. Run:

```
python -B -X utf8 scripts/probe-wide-semantic.py --metric-scale 16 --output artifacts/issue14-wide/scale16-consistent
python -B -X utf8 scripts/verify-wide-semantic-geometry.py
```

The verifier records fixed chromium/8044 primary-source URLs and SHA256 hashes for `cfx_face.cpp` and `fx_font.cpp`. The latter defines the exact metric normalization `(value * 1000.0 + upem / 2) / upem` followed by saturated integer conversion. The observed negative-coordinate behavior is therefore source-backed, not an inferred correction. Its measured checks are model residual <=0.001pt, original-cluster coincidence <=0.02pt, and non-top native edge residual <=0.006pt; it does not relabel the original all-edge geometry results.

The newest probe also makes hhea, OS/2, FontDescriptor ascent/descent and FontBBox agree with the magnified glyph bounds. Earlier variants had arbitrary global vertical metrics, which exaggerated MuPDF's discrepancies. Fresh `scale16-consistent` results retain all PDFium geometry measurements, exact Unicode 28/28, and unchanged MuPDF visible rasters 28/28. MuPDF raw-box errors improve to 3.53–3.98pt for bank samples and 15.94–18.00pt for controls, but remain different from original per-cluster geometry and fail that gate in 28/28 cases. Earlier evidence remains retained.


## Actual native writer and export verification

The native candidate is independently inspected by `scripts/inspect-native-wide-semantic.py`. Its 24 cases include 255/256/511 bank boundaries and long prefixes followed by a ligature, combining marks, or supplementary Unicode, each at all four rotations. The inspector reads both the native original and native exported PDF (48 PDFs), checks exact text in bundled PDFium, MuPDF and pypdf, retains original font bytes by SHA256, validates one invisible wide semantic font and one TJ, and compares exported character boxes, semantic font bytes, visible content prefix, and MuPDF rasters. Native PDFium raster preservation is also recorded from the generator.

```
python -B -X utf8 scripts/inspect-native-wide-semantic.py --input artifacts/shaped-text/issue14-wide-native --output artifacts/issue14-wide/native-inspection
```

Native supplementary text revealed that FPDFText_GetUnicode exposes a valid UTF-16 surrogate pair as two entries. The inspector strictly decodes valid pairs only, requires their boxes to be exactly equal, and retains their raw units; it never reverses or normalizes text. All raw FPDFText_GetText strings were already exact.

A first model using Python double-precision cursor arithmetic failed its 0.001pt limit on the longer 24pt native lines (maximum 0.001323pt). That negative evidence is retained in `geometry-model-double-negative.json`. The pinned [CPDF_TextObject::CalcPositionDataInternal](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fpdfapi/page/cpdf_textobject.cpp) performs each cursor operation in float32 starting at zero, then applies the text origin. Implementing those literal source operations reduces the model residual to **0.00000763pt**, without changing the threshold. Native cluster coincidence is at most **0.00219727pt** and native non-top edge error at most **0.00222852pt**. The tracked verifier includes this third source URL and SHA256.

The native builder's hhea/OS2 envelope includes 1000×16 ascent / -500×16 descent expanded to contain its global glyph bounds; its PDF FontDescriptor bounds are tight. This is not the same global-metric policy as the Python consistent-metrics experiment. The native report therefore does not claim identical MuPDF geometry or an identical synthetic font. MuPDF exact Unicode and unchanged export geometry/raster are checked; PDFium's source-modeled original-cluster geometry is the qualified geometry result. The compact actual-native evidence is `docs/issue14-native-wide-verification.json`.


## Bounded direction-format control

The same scale16 converter was applied to the retained mixed-bidi logical-TJ and visual-TJ pages in `semantic-rotations/semantic.pdf` at all four rotations, and to the pre-refusal `rtl-word-order.pdf` fixture. No direction reordering or new encoding strategy was introduced. Artifacts and a rerunnable script are in `artifacts/issue14-wide/direction/`.

Changing font format did **not** repair direction: all eight mixed cases fail exact Unicode in all three readers. For source `ABC سلام 123 DEF`, PDFium yields `ABC مالس 123 DEF` with logical TJ and `ABC 123 سلام DEF` with visual TJ, identically at each rotation. MuPDF adds/repositions breaks and spaces; pypdf drops portions of the mixed text. The compact JSON retains every unmodified string.

The RTL fixture contains 64 identical `سلام` words separated by spaces. Its raw copied text passes all three readers at all rotations, which is insufficient evidence because repeated words mask word-order corruption. The first PDFium logical character remains near the left (`x=73.992004`), while the rightmost character starts at `x=3290.011963`; its expected logical first word is at the right. The original Type3 failure therefore survives the wide CID conversion. Both mixed-direction and RTL-separator production guards remain justified by this bounded control.
