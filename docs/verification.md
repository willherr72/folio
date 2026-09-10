# Folio 0.4.0 verification — 2026-09-10

Verified on Windows x64 with PDFium chromium/8044.

| Check | Result |
| --- | --- |
| Frontend unit/integration tests | 100 passed |
| TypeScript and Vite production build | Passed |
| Rust formatting and standard native tests | Passed; 27 tests |
| Explicit native printing tests | 9 passed, including the 2 normally ignored desktop/printer tests |
| Text selection and editing browser regressions | Passed; all 16 intrinsic/editor rotation combinations |
| Search navigation browser checks | Passed; 4 rotations at 90% and 170% zoom |
| Actual desktop crash/restart | Passed; two dirty tabs restored after both original PDFs moved |
| Standalone editing / save / reopen / close | Passed; seven-page output, no page errors |
| Independent code review | Ready to merge; no outstanding important findings |

Tested executable SHA-256: 94ce3cfbcf1fa5606bbc967b77a01137dff9623e9c69b91ee6d977a3087bbcc6.

## Recovery evidence

The standalone release candidate restored added text, duplicated annotated pages,
110% zoom and the active tab. Search found both restored additions. Confirmed normal
closing cleared recovery, and a subsequent start offered no stale workspace.
The test generated all source files and used FOLIO_DATA_DIR to isolate recovery.
It terminated only its own verified process; no user documents or app instance were changed.

Native regressions cover interrupted writes, last-good fallback, interrupted discard,
missing/corrupt source copies, exclusive instance locks, source-ID remapping, duplicated
annotation IDs, original-path/hardlink export protection and oversized sparse sources.
Frontend regressions cover startup gating, independent tab searches, save/close ordering,
scrolling during successful and failed close checkpoints, and abandoned recovery handles.

## Printing evidence

The Windows implementation exports the current plan, then spools bounded rasters through
GDI. A test uses only Microsoft Print to PDF, reopens its output and verifies edited color
pixels. Another opens and cancels the actual Windows Print dialog without submitting a job.
The standalone UI's Ctrl+P/options/Windows dialog/cancellation path was also exercised.
Physical printers were not tested.

Combined crash-and-print automation encountered desktop window-enumeration/timeouts.
Recovery and printer checks were therefore run independently; the passing crash test is
scripts/smoke-recovery-desktop.mjs, and explicit printer checks use:
cargo test --manifest-path src-tauri/Cargo.toml --lib printing -- --include-ignored
The normally ignored tests are intentional: ordinary verification must not open a dialog
or submit even a virtual-printer job.

## Performance

The 1,000-character browser fixture averaged 66 ms across four warm tab switches,
with 16 layout passes and zero repeated render or text-extraction requests.
This controlled result is not a guarantee for every document or a Foxit comparison.
Recent page images still use the 60-entry / 96 MiB estimated decoded-image cache.
Search shares a bounded text cache and yields between page scans.
Recovery reuses existing source snapshots without rereading every PDF on each checkpoint.

## Reproducing checks

Run scripts/verify.ps1 for frontend/native checks. For browser checks, serve the repo
with Vite and run tests/tab-performance.browser.mjs, tests/productivity.browser.mjs,
tests/page-interactions.browser.mjs and tests/search-navigation.browser.mjs.
Each script documents its default local port or environment override.

For isolated crash/restart verification, set FOLIO_TEST_BINARY to a relative path to
the portable executable and run node scripts/smoke-recovery-desktop.mjs.
This intentionally terminates the test's own process and moves generated source fixtures.
Reports and screenshots are retained under artifacts.

## Limits

Recovery checkpoints after one second of inactivity or at most five seconds during
continuous activity; changes since the last successful checkpoint can be lost.
Undo history starts fresh after recovery. PDF sources are capped at 512 MiB each and
checkpoint metadata at 32 MiB. Recovery source bytes add to process memory.
Printing is rasterized (up to 2400 pixels wide); Save a copy preserves source vectors.
Saved text/ink additions still flatten; their editable save/reopen is milestone v0.6.0.
Highlights and comments are editable standard PDF annotations starting in v0.5.0.


## v0.5.0 verification

- 129 frontend tests across 27 files; TypeScript and production Vite build passed.
- 38 ordinary native tests passed, plus two explicit Windows dialog/virtual-printer tests.
- Browser highlighting passed 32 source/editor rotation and zoom combinations, forward/reverse multiline selections, copying over highlights, and scrolling selections across two/five pages.
- Full-opacity highlights retain underlying black text pixels in both the page and thumbnail with multiply compositing.
- Local signature draw/save/reload/reuse/rename/delete and narrow-dialog layouts passed browser checks.
- The built Windows app passed signature library reuse, cursor preview/Escape, proportional resizing/undo, highlight and Unicode comment creation, export/reopen, deletion/re-export and independent tabs.
- A real crash/restart restored two dirty tabs, duplicated editable comments, zoom and search after source fixtures were moved. The named signature persisted across restarts; confirmed normal close cleared recovery.
- MuPDF and pypdf independently verified standard indirect annotation objects, note contents, highlight geometry, appearance, deletion and retained unsupported annotations. A separate raster comparison verified exported vector signature ink.
- The 1,000-character tab benchmark averaged 49 ms, with 16 layouts and no repeated render/text requests.

Evidence is retained under the v0.5 development worktree's artifacts directory, including
review-desktop-1789070302266, recovery-desktop-1789070340860, annotation-tests,
annotations-browser.txt, highlight-compositing-browser.txt, and final-review.md.

Reproduce review UI checks with scripts/smoke-annotations-browser.mjs and
scripts/smoke-highlight-compositing-browser.mjs against Vite. The packaged-app check is
scripts/smoke-review-desktop.mjs, followed by scripts/smoke-recovery-desktop.mjs.
Independent-reader verification uses Python with PyMuPDF/pypdf installed:
`python -X utf8 scripts/verify-review-pdf.py path/to/reviewed.pdf`.

The Windows test helper accepts the default save filename; the review test then renames
only its generated output before reopening. Custom save-name UI automation could not
reliably commit edits in this desktop environment. Physical printers were not tested.
Source annotations with unsupported visibility, locking, or zoom/rotation flags remain
native instead of being silently restyled as editable overlays.
