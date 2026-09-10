# Folio 0.3.0 verification — 2026-09-10

Release verification is in progress. The results below distinguish completed
component checks from the final packaged Windows checks still to run.

| Check | Result |
| --- | --- |
| Focused PDF selection, signature preview, drawing, page-drag and render-cache tests | 20 passed |
| Native text geometry and engine roundtrip integration tests | 7 passed |
| Browser mouse selection and Ctrl+C, forward and reverse, at 0/90/180/270 degrees and 150% zoom | Passed; exact spaces and CR/LF preserved |
| Browser editable overlay drag, drawing over PDF text, text insertion and signature placement/cancel | Passed |
| Complete frontend suite | 65 passed |
| TypeScript check and Vite production build | Passed |
| Rust formatting and complete native tests | Passed; 7 tests |
| Windows portable build | Pending |
| Packaged Windows tabs, text entry, PDF selection, signature preview, export/reopen and all-tab close guards | Pending |

The focused browser fixture uses known character geometry to exercise actual
mouse drag and clipboard copying. Native geometry fixtures separately compare
extracted bounds to rendered PDF ink for cropped pages and intrinsic rotations.
Review identified a remaining selection endpoint issue on PDFs with intrinsic
180-degree rotation; its correction and verification are pending. These checks
do not establish that the new portable executable has passed its
full native editing and export/reopen workflow.

To reproduce the focused browser check, start Vite on port 1422 and run:

```powershell
npx vite --host 127.0.0.1 --port 1422 --strictPort
# In another PowerShell window:
node tests/page-interactions.browser.mjs
```

Final release checks must cover separate tab histories, zoom/scroll restoration,
Open versus Add PDF, immediate Content typing, signature preview and Escape,
selection/copy from a real PDF, and unsaved-change protection for inactive tabs.
The native launcher now targets `artifacts/Folio-v0.3.0/Folio.exe` by default.

## Previous release: 0.2.0

The following results describe the previously verified 0.2.0 build, not 0.3.0.

Verified on Windows x64 with PDFium chromium/8044.

| Check | Result |
| --- | --- |
| Frontend state, drawing, viewport, preferences, dialogs and page-drag tests | 48 passed |
| TypeScript check and Vite production build | Passed |
| Rust formatting | Passed |
| Native integration tests, default parallel execution | 4 passed |
| Browser interaction and upgrade smoke scripts | Passed; no page errors or browser confirmation prompts |
| Windows portable release build | Passed |
| Standalone release desktop editing, export/reopen and close | Passed; no page errors |
| Independent source review and targeted browser regressions | Approved; findings corrected and rechecked |

Browser upgrade checks cover continuous-scroll selection, edits on the correct
page, drawing with undo/redo, real thumbnail drag with insertion markers,
cursor-anchored Ctrl+wheel zoom, theme and view-mode persistence, centered
confirmations, modal focus restoration, and Undo after an interrupted move.

Native regressions cover crop origins, intrinsic and additional rotation,
multiline text, the final segment of a saved stroke, duplicate annotated pages,
atomic replacement, source paths and hardlink aliases. Unsupported characters
preserve a pre-existing destination byte for byte.

The actual portable executable opened a sample PDF, added text, signature and
freehand ink, dragged and undid a page move, duplicated and merged pages,
rejected unsupported text, saved and reopened seven pages, and verified both
close-confirmation decisions. A separate PDFium probe rendered the saved PDF;
visual inspection confirmed text placement and complete freehand/signature paths.
See [the desktop screenshot](folio-desktop.png).

## Reproduce

Run scripts/verify.ps1 for frontend and native checks, and
scripts/build-portable.ps1 for the release distribution. Packaging uses a
separate versioned Cargo target directory, so a previously running development
executable does not lock the next release build.

For browser checks, install Playwright Chromium, start the Vite server, then
run node scripts/smoke-ui.mjs and node scripts/smoke-upgrades.mjs.

For native smoke, install PowerShell 7 (pwsh) and Playwright, then run
scripts/start-desktop-test.ps1 in a separate PowerShell process, set
FOLIO_APP_PID from artifacts/desktop-process.json, then run
node scripts/smoke-desktop.mjs. The test closes its own app after checking both
close-confirmation choices. A failed run may leave its recorded test process open.

Native dialog automation is restricted to the recorded process and English
Windows dialog titles. It uses accessibility controls and the default save filename in a fresh test directory. Leave the test window untouched while it runs. The launcher uses an isolated WebView profile and a
temporary loopback automation port; the normal launcher does not enable that port.

Detailed reports, saved output, screenshots and probe results are retained
under artifacts, which is intentionally ignored by Git.

## Evidence limits

Testing uses generated sample PDFs and targeted regression fixtures. This
prototype has not been evaluated against a broad real-world PDF corpus or
benchmarked against Foxit. Direct-engine timing on tiny samples is not an
end-to-end application performance benchmark.

See [README](../README.md) for supported operations and prototype boundaries.