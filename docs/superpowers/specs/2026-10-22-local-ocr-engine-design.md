# Local OCR engine

This is the recognition subsystem of the continuing Adobe/Foxit usefulness goal.
It must provide dependable offline recognition before the PDF modification and review
workflow is connected. It does not make scanned ink visibly editable, and the UI must
not advertise OCR until the complete workflow is verified. The user's autonomous-work
instruction authorizes implementation without another design approval.

## Decision and evidence

Use Tesseract.js **6.0.1**, tesseract.js-core **6.0.0**, and the float
`tessdata_best` **4.1.0** English (`eng`), French (`fra`) and Spanish (`spa`) models.
Node and Chromium probes recognize the synthetic clean/skew pages; blank produces no
words. Version7 with the float model aborts on this Windows machine, matching
https://github.com/naptha/tesseract.js/issues/1080. The faster integer English model
introduced a spurious period in the clean sample. These are selection evidence, not
general accuracy claims. Windows.Media.Ocr requires package identity; portable Folio
must not depend on that undocumented availability.

Own the browser Worker directly using the pinned worker message protocol. The upstream
createWorker promise does not expose termination until initialization succeeds and can
remain pending after loadLanguage failure. An immediate Worker handle lets Folio settle
every pending call and terminate on abort, timeout, worker error or protocol failure.
There is one worker per session and at most one in-flight recognition; no job queue or
parallel page raster buffers. Languages cannot change mid-session.

## Contract

`startOcrSession(languages, {signal?, onProgress?})` returns a promise of an `OcrSession`.
`recognize(page, signal?)` returns `OcrPage`; `dispose()` is synchronous and idempotent.
The page is PNG bytes, width, height and DPI from a future permission-checked native
raster command. Text/line/word geometry is measured in the exact input pixel space.
No implicit deskew or orientation transform; the existing small-skew recognition is
tested, and page orientation handling belongs to the later native/UI integration.
Output contains ordered blocks/paragraphs/lines, word text, confidence and boxes.
Confidence is an engine score, not a probability. Blank pages are valid empty results.

Only distinct `eng`, `fra`, `spa` codes are accepted, 1–3 per session. Validate inputs
before worker creation/posting: finite positive integer dimensions at most **6000**,
at most **20,000,000 pixels**, PNG payload at most **32 MiB**, and DPI **72–600**.
Check PNG signature and IHDR dimensions match the declared raster before decoding in
the worker. Reject unsupported formats and oversized/malformed declarations.
Initialization has a **120-second** deadline; a page has a **120-second** deadline.
On error/timeout/cancel the session terminates; retry means a new session.
Reject a concurrent recognize call without disturbing the active page.

Normalize only structurally valid output: at most **20,000 words**, **100,000 Unicode
scalars** and **2 MiB UTF-8 text** per page, **10,000 lines**, finite confidence **0–100**,
and finite ordered boxes inside the raster. Preserve recognized Unicode and reading
order. Do not silently drop invalid words or clip invalid geometry. Native PDF writing
will repeat validation at the trust boundary. Empty blocks are omitted; malformed
containers fail. Remove symbols and other unused rich OCR output before returning.

## Local assets and reproducibility

Runtime loads only same-origin `/ocr/` assets. No remote URL options, CDN fallback,
language downloads, browser text cache or persistent scan buffers. Explicit corePath,
langPath, cacheMethod `none` and gzip `false`; only LSTM core paths are bundled, both
SIMD and non-SIMD. A wrapper worker restricts network/importScripts to its own OCR asset
directory before loading upstream code. This restriction is local to OCR, not a new
global application CSP that might disrupt unrelated workflows.

`scripts/setup-ocr.mjs` materializes generated `public/ocr/` from installed pinned npm
packages and model downloads verified against `scripts/ocr-assets.json` size/SHA256.
Normal dev/build performs **offline verification only** and fails with setup instructions
when assets are missing or changed. Only explicit `npm run setup:ocr` may download the
fixed public models. Assets and checksums are independently checked before publication;
temporary partial files never count as prepared. Keep models, upstream worker/core
licenses and model provenance in the shipped assets and portable license collection.
No change to the existing portable checkpoint; package a new version after the entire
OCR workflow is verified.

## Acceptance and next integration

Unit tests cover bounds, Unicode, blank output, stale replies, failure at every
initialization stage, cancellation before/during startup and recognition, timeout,
concurrent calls, progress, idempotent disposal and rejected remote paths. Browser
acceptance uses real shipped assets with external requests denied; English, accented
French/Spanish and all three together must recognize representative printed samples.
Exercise missing models, cancellation and reuse across pages in Chromium and the real
Windows WebView2. Check both SIMD and non-SIMD assets load. Full frontend suite and
production build must pass; one fresh review closes this subsystem.

The subsequent PDF workflow must add native permission-checked high-resolution raster,
page/language choice, review/correction, invisible text preservation, owned-layer
replacement, mixed native-text filtering, immutable source versions, undo/recovery,
cancel/stale-source cleanup and encrypted save/reopen. Do not mark OCR or parity done
from recognition alone. Prototype evidence in `artifacts/ocr-probe` shows pixel-identical
invisible layers and correct Folio/MuPDF/pypdf extraction at all quarter-turns; raw
PDFium reverses line order at90 degrees for the line-based prototype. That external
reader limitation needs an explicit decision and regression in the integration spec.
