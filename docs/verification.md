# Folio 0.3.1 verification — 2026-09-10

Verified on Windows x64 with PDFium chromium/8044.

| Check | Result |
| --- | --- |
| Frontend tests | 68 passed |
| TypeScript and Vite production build | Passed |
| Rust formatting and native integration tests | Passed; 7 tests |
| Independent performance-fix review | Approved |
| Text selection and editing browser regressions | Passed, including all 16 intrinsic/editor rotation combinations |
| Native standalone editing, export/reopen and close workflow | Passed; no page errors |

## Tab performance

Both versions switched four times between the same two already-open documents.
The Windows test uses actual PDFs containing 1,038 extracted characters per page.
The browser fixture uses 1,000 characters and a generated page image.

| Measurement | 0.3.0 | 0.3.1 |
| --- | --- | --- |
| Windows mean tab switch | 1,602 ms | 92 ms |
| Windows layout passes across four switches | 4,164 | 16 |
| Browser mean tab switch | 1,436 ms | 47 ms |
| Browser repeated render requests | 4 | 0 |

These are controlled fixture results on this machine, not a guarantee for every
document or a Foxit comparison. Timing runs from the tab click to the second
animation frame; each sample also verifies that its page image and text appear.
A separate 4,500-character browser page averaged 281 ms after the fix.

The original text layer alternated reading a character's width with writing its
transform, causing a layout pass for nearly every character. Reading all widths
before writing transforms removes that repeated work. Full-size page images now
survive normal unmounts and are reused when returning to a tab or scrolling back.

The image cache has a 60-entry limit and a 96 MiB estimate of decoded image memory.
It evicts the least recently used idle images and protects mounted consumers.
This is an estimated cache budget, not a process-RAM limit; mounted images may
exceed the budget. Closing a PDF releases its entries, including pending renders
that resolve later. Regression tests cover reuse, eviction and late disposal.

## Native workflow

The release executable opened real PDFs, accepted immediate keyboard input after
text placement, matched signature preview and placement geometry, copied normal
and intrinsically upside-down PDF text, drew ink, dragged thumbnails, duplicated
and merged pages, rejected unsupported text without creating output, saved and
reopened seven pages, and protected unsaved changes in an inactive tab.

Native-tested executable SHA-256:
`f902894dd9211839c7d3c93288f90049c7eae7e7e7ea0c1b2649b081a0bcb907`.

Packaging checks that the ZIP contains this same executable, PDFium, examples,
documentation and notices for 513 dependency packages.

## Reproduce

Run `scripts/verify.ps1` for the complete suite. Build a portable release with
`scripts/build-portable.ps1` from a normal dependency installation.

For the browser performance regression:

```powershell
npx vite --host 127.0.0.1 --port 1423 --strictPort
# In another window:
$env:FOLIO_PERF_ASSERT='1'
node tests/tab-performance.browser.mjs
# Optional dense page:
$env:FOLIO_PERF_CHARACTERS='4500'
node tests/tab-performance.browser.mjs
```

For Windows, launch a separate test instance with
`scripts/start-desktop-test.ps1`, set `FOLIO_APP_PID` from
`artifacts/desktop-process.json`, then run
`node scripts/benchmark-tabs-desktop.mjs`.
The benchmark opens generated fixtures and closes its test instance.
Launch another test instance to run `node scripts/smoke-desktop.mjs`.
Both scripts require Playwright and PowerShell 7.

Dialog automation targets only the recorded process. Leave automated test
windows untouched while a run is active. The normal launcher does not enable
the temporary loopback debugging port used by these scripts.

Reports and PDFs are retained under ignored `artifacts` folders.
See [README](../README.md) for supported operations and prototype boundaries.
