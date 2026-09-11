# Folio v0.6.1 performance verification

Issue #9 targets two measured costs: rebuilding dense selectable text layers on a warm tab switch, and retaining every thumbnail ever visited. v0.6.1 caches glyph scale factors alongside the lifetime of extracted text and releases thumbnail raster references outside the existing preload margin.

## Text geometry

CPU profiling of the 6,000-character fixture identified repeated glyph width reads in the text layer as a hot path. The first mount still batches reads before transform writes. A WeakMap reuses Float64 scale factors for the same extraction object on later mounts. The bounded source-text cache controls that object's lifetime; the new map does not keep an evicted source alive. Simultaneous duplicate pages share the geometry even if both render before either layout effect commits.

No absolute latency threshold is imposed in tests. Stable checks assert that a warm remount does not remeasure glyph widths, duplicated pages receive the correct transforms, a new extraction is measured again, and warm switches issue no native rendering or text-extraction requests.

## Validation

- All 153 frontend unit/integration tests pass.
- Production TypeScript/Vite build and Rust formatting checks pass.
- Browser selection, exact copied whitespace, and highlight rectangles pass on cold and cached remounts across 32 source/editor rotation and zoom combinations; two-page and five-page continuous selections also pass.
- Independent runtime review found one batched observer delivery edge case. The fix uses the latest visibility record; its regression and the complete suite pass. No unresolved runtime review findings remain.
- Native PDF editing/export code is unchanged by this release.

See [the memory investigation](issue-9-memory.md) for retained-image counts, process roles and natural reclamation. The available development machine has 95.3 GiB RAM. Physical lower-memory validation is tracked in [issue #10](https://github.com/willherr72/folio/issues/10); CPU throttling is not a substitute for that test.

## Packaged comparison — 2026-09-11

Same Windows 11 / Ryzen AI 9 HX 370 / 95.3 GiB host, WebView2 152.0.4191.66, 1280×820 viewport and 1.5 display scale. Both versions mounted 9,880 dense text glyphs and zero scan glyphs. Runs proceeded baseline/candidate, then an unthrottled baseline/candidate repeat, after builds and other performance tests had finished.

| Document | CPU rate | v0.6.0 mean | v0.6.1 mean | Samples per version |
| --- | --- | --- | --- | --- |
| dense-300-pages.pdf | 1x | 848 ms | 632 ms | 12 |
| scan-40-pages.pdf | 1x | 84 ms | 74 ms | 12 |
| dense-300-pages.pdf | 4x | 4986 ms | 2610 ms | 6 |
| scan-40-pages.pdf | 4x | 421 ms | 236 ms | 6 |

All 72 measured warm switches made zero repeated native render/text requests, with a positive control observing both command types before each run. The unthrottled dense result is an improvement on this workload; timings still vary between samples, and dense pages still incur DOM creation/layout costs. This is not a Foxit comparison or an instant-switch guarantee. The 4x rows are artificial CPU slowdown only.

[Exact samples, ranges, metrics, fixture and binary hashes](performance-v0.6.1.json) preserve the comparison. The earlier development and September 10 exploratory timings are excluded from this final comparison. The thumbnail workload retained 60 images instead of 302; [process snapshots](issue-9-memory.md) do not demonstrate a total-RAM reduction.

Tested/shipped executable SHA-256: `51e312cf0a94ec4db3cdc95fefd9c4e5df27f64ebc895ab80c74b5688b93870d`.

## Reproduction

Build each version as a production desktop executable and start it with `scripts/corpus-start-desktop.ps1` using a unique port and profile. Generate the corpus described in [the corpus README](corpus/README.md). Set `FOLIO_APP_PID`, `FOLIO_CDP_URL`, `FOLIO_CORPUS_DIR` and `FOLIO_PROFILE_OUTPUT`, then run `node scripts/issue9-tabs-desktop.mjs`.

The harness measures inside WebView from a tab click until the selected tab contains its raster image element and calibrated text, plus two animation frames. This is DOM/text readiness, not proof of image decoding or pixels presented to the display. It records six samples per document for CPU rates 1 and 4, with zero repeated native `render_page`/`page_text` requests required. `FOLIO_PROFILE_RATES=1` restricts a repeat to the unthrottled run. `FOLIO_PROFILE_REUSE_TABS=1` permits already-open test corpus tabs, useful when submitting the native dialog directly with the process-scoped helper. Before measuring, the harness visits a previously untouched dense page and requires the same collector to observe both native command types. Set `FOLIO_PROFILE_PROBE_PAGE` to a different unvisited page index when repeating on an existing app.

For browser CPU sampling, serve the fixture with Vite and run `node scripts/profile-text-tabs.mjs`. `FOLIO_PROFILE_URL`, `FOLIO_PROFILE_CHARACTERS`, `FOLIO_PROFILE_ROUNDS` and `FOLIO_PROFILE_LABEL` select the fixture and output. It writes a Chrome CPU profile and metrics, and asserts zero repeated render and text requests. Browser development timings are diagnostic and are not directly comparable to packaged release results.
