# Folio 0.3.0 verification — 2026-09-10

Verified on Windows x64 with PDFium chromium/8044.

| Check | Result |
| --- | --- |
| Complete frontend suite | 65 passed |
| TypeScript check and Vite production build | Passed |
| Rust formatting and native integration tests | Passed; 7 tests |
| Browser editing and upgrade workflows | Passed |
| Real page click followed by immediate typing | Passed; Content retains focus and replaces the placeholder |
| Browser forward/reverse mouse selection and Ctrl+C | Passed at all 16 intrinsic/editor quarter-turn combinations and 150% zoom |
| Native standalone release build | Passed |
| Packaged Windows editing, tabs, export/reopen and close | Passed; no page errors or browser confirmation prompts |
| Independent implementation review | Approved; rotation and click-focus findings corrected and rechecked |

The native executable opened actual PDFs through Windows dialogs. It copied text
from a normal PDF and the complete two-line text from a cropped PDF with intrinsic
180-degree rotation. It added text through immediate keyboard typing, drew a
signature, verified that preview points exactly matched the placed signature,
drew freehand ink, dragged thumbnails, duplicated and merged pages, rejected
unsupported text without creating output, saved and reopened the seven-page PDF.

The same run verified separate tabs, restored zoom and scroll position, a
cancellable tab-close dialog, and app-close protection for unsaved edits in an
inactive tab. The final discard decision closed the test app.

Native geometry tests compare character bounds against rendered ink for nonzero
crop origins and all four intrinsic rotations. Export regressions cover crop and
rotation, multiline text, the final stroke segment, duplicate annotated pages,
atomic replacement, source-path safety and hardlink aliases.

The native-tested executable SHA-256 is
`b12373f8048d36c3b3fd2953e798571de92d9942f9700611bf9a567f5d753c57`.
Release packaging checks that the executable inside the ZIP matches this hash.
The distribution includes notices for 513 dependency packages.

## Reproduce

Run `scripts/verify.ps1` for frontend and native checks and
`scripts/build-portable.ps1` for a standalone build and portable distribution.
Use a normal dependency installation for license collection; npm's dependency
listing does not handle a shared node_modules directory junction reliably.

For browser checks, install Playwright Chromium and start Vite:

```powershell
npx vite --host 127.0.0.1 --port 1422 --strictPort
# In another PowerShell window:
$env:FOLIO_UI_URL='http://127.0.0.1:1422/?demo=1'
node scripts/smoke-ui.mjs
node scripts/smoke-upgrades.mjs
node tests/productivity.browser.mjs
node tests/page-interactions.browser.mjs
```

For native smoke, install PowerShell 7 (pwsh) and Playwright. Run
`scripts/start-desktop-test.ps1` in a separate PowerShell process, set
`FOLIO_APP_PID` from `artifacts/desktop-process.json`, then run
`node scripts/smoke-desktop.mjs`. The test closes its own app after checking
both close-confirmation decisions. A failed run may leave its recorded test
process open.

Native dialog automation is restricted to the recorded process and English
Windows dialog titles. It uses accessibility controls and the default save
filename in a fresh test directory. First-run Explorer initialization is allowed
up to 45 seconds. Leave the test window untouched while it runs. The launcher
uses an isolated WebView profile and a temporary loopback automation port;
the normal launcher does not enable that port.

Reports, saved output, screenshots and probe results are retained under
`artifacts`, which is intentionally ignored by Git.
See [the native desktop screenshot](folio-desktop.png).

## Evidence limits

These checks use generated samples and targeted regression PDFs. Folio has not
been evaluated against a broad real-world PDF corpus or benchmarked against
Foxit. The browser fixtures exercise selection mechanics with known geometry;
the native run additionally checks real PDFium geometry and Windows behavior.
Image-only scans have no selectable text without OCR.

See [README](../README.md) for supported operations and prototype boundaries.
