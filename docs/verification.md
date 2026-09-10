# Folio verification — 2026-09-10

Verified on Windows x64 with PDFium chromium/8044.

| Check | Result |
| --- | --- |
| Frontend state, geometry, cache and interface tests | 15 passed |
| TypeScript check and Vite production build | Passed |
| Rust formatting and all-target check | Passed |
| Native integration tests, normal target, default parallel execution | 4 passed |
| Browser interaction smoke | Passed; no page errors |
| Standalone release desktop smoke, development server stopped | Passed; no page errors |
| Independent source review and targeted regression reruns | Approved; no unresolved important or critical findings |

The desktop smoke launched artifacts/Folio/Folio.exe with its bundled engine
and embedded interface. It opened a real PDF through the Windows dialog, added
text and a drawn signature, duplicated the annotated page, merged another PDF,
then saved and reopened the seven-page output. Unsupported text produced a
visible error, retained unsaved state, and created no damaged output file.

A separate PDFium probe rendered the saved output. Visual comparison confirmed
matching text placement and the complete signature.
See [the actual desktop screenshot](folio-desktop.png).

Native regressions cover crop origins, intrinsic and additional rotation,
multiline text, the final segment of a saved stroke, duplicate annotated pages,
atomic replacement, source paths and hardlink aliases. Unsupported characters
preserve a pre-existing destination byte for byte. The reviewer independently
checked all 216 accepted printable characters against PDFium.

Frontend review exercised 200% zoom, a 65-page preview cache stress case,
edge signature placement and repeated spaces. All findings were fixed and
independently rechecked.

## Reproduce

Run scripts/verify.ps1 for frontend and native checks, and
scripts/build-portable.ps1 for the release distribution.

For the desktop smoke, install Playwright Chromium, run
scripts/start-desktop-test.ps1 in a separate PowerShell process, set
FOLIO_APP_PID from artifacts/desktop-process.json, then run
node scripts/smoke-desktop.mjs. Close that test process afterward.

The native smoke uses English Windows dialog accelerators and a temporary
loopback WebView automation port. Leave keyboard focus with the test dialogs.
The normal launcher does not enable that port.

Detailed reports, saved output, screenshots and probe results are retained
under artifacts, which is intentionally ignored by Git.

## Evidence limits

This prototype was tested on generated sample PDFs and targeted regression
fixtures. It has not been evaluated against a broad real-world PDF corpus or
benchmarked against Foxit. Direct-engine timing on tiny samples is not an
end-to-end application performance benchmark.

See [README](../README.md) for supported operations and prototype boundaries.
