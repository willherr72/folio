# Issue 9: thumbnail retention investigation

The sidebar kept every visited thumbnail mounted. Its visibility observer disconnected on first entry, so leaving the viewport never released the thumbnail's render-cache reference. All mounted images are intentionally protected from eviction; consequently, visiting 300 thumbnails retained 302 image URLs despite the cache's 60-entry limit.

The fix keeps observing visibility and unmounts the raster outside the existing 300-pixel preload margin. It follows the last event when entry and exit arrive together. The existing cache then evicts idle images, reuses recent images on return, and continues protecting images still on screen. Document close still revokes pending renders when they finish.

## Evidence and scope

The baseline is the shipped v0.6.0 binary, SHA-256 `0bab799243f9173f20196b154a91159e8caa0c8f604a9ba55668794ee1b50ea3`. The v0.6.1 candidate is SHA-256 `51e312cf0a94ec4db3cdc95fefd9c4e5df27f64ebc895ab80c74b5688b93870d`. The fixtures are the generated corpus documented in [the corpus README](corpus/README.md).

The initial scan investigation observed three complete 40-page scroll passes with natural ten-second idle intervals. The cache retained exactly 33 PNG URLs / 54.42 MiB after each pass; all were revoked after close. In the clean-profile repeat, private-memory snapshots after those idle intervals were 1,037, 1,114, and 1,144 MiB, then 703 MiB after close. Earlier role sampling identified the WebView GPU process as the largest contributor. These observations do not establish a scan-memory leak or justify a broad cache rewrite.

Actual `render_page` responses used `application/octet-stream`. Blob samples had the PNG signature and dimensions of 826×1069 for full pages and 240×311 for thumbnails; scan PNGs were approximately 2 MB each. The Rust command already returns a binary IPC response. The extra frontend byte copy remains unchanged: this investigation did not establish it as the dominant allocation.

The focused desktop workload starts an isolated profile, opens only `dense-300-pages.pdf`, settles for at least ten seconds, scrolls the sidebar through all 300 thumbnails, samples immediately and after ten seconds idle, closes the clean document, and samples again after ten seconds. Raster creation and revocation are counted without retaining image payloads; only PNG header metadata is inspected. No garbage collection is forced.

| Observation | v0.6.0 | v0.6.1 |
|---|---:|---:|
| Mounted thumbnails after all 300 visits | 300 | 5 |
| Live image URLs after traversal | 302 | 60 |
| Retained encoded PNG data | 22.85 MiB | 5.16 MiB |
| Decoded RGBA size estimate from PNG dimensions | 92.16 MiB | 23.25 MiB |
| Tree private memory immediately after traversal | 957.57 MiB | 989.97 MiB |
| Tree private memory after ten seconds idle | 747.99 MiB | 774.93 MiB |
| Tree private memory after close and ten seconds idle | 636.70 MiB | 802.36 MiB |
| Live image URLs after close | 0 | 0 |

The candidate revoked 242 images during traversal and the remaining 60 after close. Both runs ended with zero live image URLs; the candidate passed explicit maximum-60-image and zero-after-close checks. Cache retention is bounded, but these samples **do not demonstrate a total-RAM reduction**. Actual 8 GB hardware validation remains tracked in issue 10.

The [machine-readable report](issue-9-memory.json) preserves exact byte counts, process-role samples, fixture/binary hashes, timestamps, and limitations. Full local artifacts are retained under `artifacts/issue9-memory`, `artifacts/issue9-memory-baseline-clean`, `artifacts/issue9-thumbnails-baseline`, and `artifacts/issue9-thumbnails-candidate`. The baseline needed a resumed dense phase after Windows dialog automation stalled; the same app and image instrumentation remained active. Dialog delays precede the scrolling phase. These are single runs on successive dates with other development work sharing the machine. Process totals varied substantially with earlier work and natural GPU/allocator retention, so they are not a promised RAM reduction or a low-memory guarantee. Working-set sums may double-count shared pages; private-memory sums are process snapshots, not simultaneous unique physical-memory peaks.

## Verification

The regression failed on the original behavior: exiting the viewport left the SVG mounted, and 300 visits retained 300 thumbnail URLs. A separate batched-entry/exit regression failed when the observer used `some(isIntersecting)`. After the fix, all four focused tests pass: ordinary entry/exit/reentry, 300 visits with eviction and revisit, batched visibility, and a render finishing after source close. Existing render-cache tests protect mounted previews even above the cache limit.

```powershell
npx vitest run tests/thumbnail-viewport.test.tsx tests/render-cache.test.tsx tests/tab-cache.test.tsx
```

For the packaged diagnostic, launch an owned isolated instance with `scripts/corpus-start-desktop.ps1`, set `FOLIO_APP_PID`, `FOLIO_CDP_URL`, `FOLIO_CORPUS_DIR`, and `FOLIO_CORPUS_UI_OUTPUT`, then run:

```powershell
$env:FOLIO_MEMORY_WORKLOAD = 'dense'
$env:FOLIO_DIALOG_MANUAL = '1'
$env:FOLIO_EXPECT_BOUNDED_THUMBNAILS = '1' # Candidate only; the baseline exceeds the limit.
node scripts/issue9-memory-profile.mjs
```

When the harness prints `OPEN_DIALOG`, run the process-scoped helper directly in a second PowerShell session in the same worktree. Adjust the fixture path if the corpus is stored elsewhere:

```powershell
$owned = Get-Content -Raw artifacts/corpus-ui/process.json | ConvertFrom-Json
& ./scripts/set-native-dialog.ps1 -AppProcessId $owned.pid -FilePath (Resolve-Path artifacts/corpus/dense-300-pages.pdf) -Action Open
```

On this Windows session, both nested `pwsh` and some direct helper invocations failed to inspect the dialog. The measured runs used a process-scoped UI Automation fallback when needed: find the exact owned dialog and filename edit, set and verify the fixture path, verify the Open button's process ID, then submit that button. These delays are outside the measured traversal. No other application windows were operated on.
