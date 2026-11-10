# Local OCR engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Provide a bounded, cancellable, offline OCR engine ready for the PDF review workflow.

**Architecture:** A pinned Tesseract worker owns its WASM/model memory. Folio controls
the worker lifecycle and validates input/output; deterministic asset preparation keeps
all runtime loads local. This is a coherent recognition subsystem, not finished PDF OCR.

**Tech Stack:** TypeScript, Web Worker, Tesseract.js6.0.1/core6.0.0, tessdata_best4.1.0,
Node asset preparation, Vitest and real Chromium/WebView2 acceptance.

**Spec:** `docs/superpowers/specs/2026-10-22-local-ocr-engine-design.md`

## Global Constraints

- Only distinct eng/fra/spa, 1–3 languages; no runtime network outside local OCR assets.
- PNG <=32MiB, dimensions1–6000, <=20,000,000pixels, DPI72–600, matching IHDR.
- Initialization120seconds, recognition120seconds; one worker/session, one active page.
- <=20,000words,100,000Unicode scalars,2MiB UTF-8 text,10,000lines per page.
- Finite confidence0–100 and boxes wholly inside the input raster; preserve Unicode/order.
- Keep work uncommitted in the existing authorized worktree; do not overwrite checkpoints.

## Review Focus

- Model initialization failure must reject and terminate immediately, not hang until timeout.
- Already-aborted and late-canceled jobs must never leak a worker or publish a result.
- Worker protocol corruption and out-of-bounds OCR geometry must fail the session.
- Asset corruption/missing fallback core must fail builds, not reach a user's first scan.
- Non-English accents, blank pages and multi-language selection must survive the actual engine.

### Task1: Reproducible offline assets

**Files:** `scripts/ocr-assets.json`, `scripts/ocr-assets.mjs`, `scripts/setup-ocr.mjs`,
`scripts/ocr-assets.test.mjs`, `scripts/ocr-worker.js`, `package.json`, `package-lock.json`,
`.gitignore`, `scripts/collect-licenses.ps1`, `THIRD_PARTY_NOTICES.md`.
**Interfaces:** `prepareOcrAssets({root, download, fetchImpl?})` Promise manifest; CLI
`node scripts/setup-ocr.mjs [--offline]`. Produces `/ocr/worker.js`, upstream worker,
SIMD/non-SIMD LSTM WASM wrappers, `models/{eng,fra,spa}.traineddata`, licenses, manifest.

- [x] Write node tests for offline missing/corrupt assets, verified download size/hash,
  mismatched installed version, idempotent preparation and complete asset manifest.
- [x] Run `node --test scripts/ocr-assets.test.mjs`; observe missing implementation.
- [x] Implement hash-checked preparation and local worker network restrictions; pin npm
  packages, wire offline dev/build verification and license distribution.
- [x] Run node tests, explicit setup, offline verification; inspect generated manifest.

### Task2: Bounded recognition and lifecycle

**Files:** `src/editor/ocr/types.ts`, `result.ts`, `session.ts`,
`tests/ocr-result.test.ts`, `tests/ocr-session.test.ts`.
**Interfaces:** `OcrLanguage='eng'|'fra'|'spa'`; `OcrRaster={png:Uint8Array,width:number,
height:number,dpi:number}`; `OcrPage={width,height,blocks:OcrBlock[]}`; block paragraphs
contain lines of `{words:OcrWord[]}`; word `{text,confidence,bounds:{x,y,width,height}}`.
`normalizeOcrResult(raw:unknown,width:number,height:number):OcrPage` validates rich output.
`startOcrSession(languages:readonly OcrLanguage[],options?:{signal?:AbortSignal,
onProgress?:(progress:OcrProgress)=>void}):Promise<OcrSession>`;
session `recognize(raster:OcrRaster,signal?:AbortSignal):Promise<OcrPage>` and `dispose():void`.
Progress `{stage:'initializing'|'recognizing',fraction:number}` only finite0–1 values.

- [x] Add failing structural/Unicode/boundary and lifecycle tests, including each Review
  Focus condition, late reply, worker error/messageerror, failed postMessage and concurrent call.
- [x] Run `npx vitest run tests/ocr-result.test.ts tests/ocr-session.test.ts`; observe RED.
- [x] Implement types, input/output validators and pinned worker requests with explicit
  cleanup of deadlines/listeners/pending promises at every terminal path.
- [x] Run targeted tests and `npm run build`; correct failures and record GREEN evidence.

### Task3: Actual engine acceptance and handoff

**Files:** `scripts/ocr-browser-acceptance.mjs`, `docs/local-ocr.md`,
`artifacts/ocr-engine-progress.md`, `docs/product-quality-loop.md`.
**Interfaces:** acceptance bundles/injects the actual session module, routes external
requests to failure, uses generated synthetic scans from local fonts and records JSON.
Same module runs in a fresh owned WebView2 process; stop only the owned process.

- [x] Prove shipped engine output on clean/skew/blank, French accents, Spanish accents,
  combined languages, cancellation during load/recognition and missing models; exercise
  SIMD and non-SIMD independently. Record errors, network requests and actual timing.
- [x] Run full frontend tests, asset tests and production build; inspect all results.
- [x] Request one fresh whole-subsystem review with spec, test evidence and known gaps.
  Fix material findings with RED/GREEN and relevant regression suite.
- [x] Document exact recognition scope and remaining PDF integration; update ledger and
  broad checklist. Keep the persistent overall goal active and continue native OCR work.

Completion evidence: `artifacts/ocr-engine-progress.md`. Reviewed foundation complete;
the broad goal remains active and the next subsystem is native PDF OCR integration.
