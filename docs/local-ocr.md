# Local PDF recognition

Current source builds include **Recognize text**. Choose the current page, a range
or all pages, select the languages present, then start recognition. Review words on
the scan, use **Next uncertain word** or the score filter, and save individual
corrections before applying. Words can be removed and restored. Corrections change
searchable/copyable text; the scanned lettering stays unchanged.

Applying is one undo step. Copies of the same original page update together,
including copies outside the selected range. Native text, comments, form fields and
editor additions are preserved; annotations and forms are excluded from recognition.
Verified Folio OCR opens for review without running the worker again and can be
corrected or removed. Externally changed/unrecognized OCR is retained as native text.

Recognition requires permission to edit and copy PDF content. Owner unlock follows
the existing protected-PDF workflow; saved copies retain the original protection.
Signed, certified and XFA documents are unsupported for OCR. Cancel discards the
pending result; bounded native verification may finish after the dialog closes.

Run `npm ci`, then `npm run setup:ocr` once to prepare the local bundle. Setup downloads
the fixed public English, French and Spanish models and verifies every model, worker
and WASM file against `scripts/ocr-assets.json`. `npm run dev` and `npm run build`
only verify local assets and fail with setup instructions if files are missing or
corrupt. They do not download models. An offline build works after preparation.

Runtime recognition uses Tesseract.js6.0.1/core6.0.0 with tessdata_best4.1.0. The
worker restricts loads to the app's own OCR assets and uses no browser model cache.
One session recognizes pages sequentially and can be canceled even during startup.
Both initialization and each page have a120-second deadline. Input is PNG, at most
6000pixels on either side and20million pixels,32MiB encoded,72–600DPI. These admission
limits are not a claim about peak whole-process memory. Native rendering chooses
300 DPI or scales down to those pixel limits. Each job admits at most100 distinct
source pages,500,000 words and2,000,000 Unicode scalars; each page admits at most
20,000 words and100,000 scalars. Large documents require separate recognition jobs.

Select only languages present in the document. Single-language synthetic samples
recognized English, accented French and accented Spanish exactly. With all three
languages enabled, the Spanish word `tomó` became `tomé` in the tested sample. That
counterexample is retained in the acceptance reports. Confidence values are engine
scores, not probabilities; real scans still need review and correction.

Recognition produces validated word boxes and text in the input raster's pixel
coordinates. It does not rotate or deskew the image implicitly. The prototype passes
a2.5-degree skew sample, which does not establish arbitrary-angle recognition.
The initial language models cover Latin printed text; handwriting, other scripts and
automatic page orientation are not claimed by this milestone.

Validation commands:

```powershell
node --test scripts/ocr-assets.test.mjs scripts/ocr-worker.test.mjs
npx vitest run tests/ocr-result.test.ts tests/ocr-session.test.ts tests/ocr-workflow.test.ts tests/ocr-hook.test.tsx tests/ocr-dialog.test.tsx tests/ocr-app.test.tsx
cargo test --manifest-path src-tauri/Cargo.toml --test ocr_engine --locked
node scripts/ocr-browser-acceptance.mjs
node scripts/ocr-browser-acceptance.mjs --non-simd
```

The acceptance script uses the real session module and packaged assets, blocks external
requests, checks recognition/cancellation/missing models, and records results in
`artifacts/ocr-engine`. It can also attach to an isolated, owned Windows test app with
`--cdp=http://127.0.0.1:PORT`; never point it at the user's working instance. The
module is bundled and injected for that subsystem test. The app's own embedded
`/ocr/` assets are exercised in that mode. The separate `smoke-pdf-ocr-*.mjs` scripts
exercise the actual desktop UI and native dialogs in a recorded isolated test app.
`verify-ocr-desktop.py` checks its saved PDFs with independent readers.

Folio preserves OCR word and line order internally. Other readers may differ in
selection geometry and whitespace; tested PDFium bounds differ by up to0.25pt from
the intended boxes. Recognition scores are not a guarantee of accuracy. Reading
order across complex mixed layouts and general Adobe/Foxit parity are not established.
The local portable 0.15.0-dev.1 includes this workflow. Its acceptance script is
`scripts/smoke-ocr-portable.mjs`; results are recorded separately under
`artifacts/pdf-ocr-portable`. The earlier 0.14.0-dev.1 package is preserved.
