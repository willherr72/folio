# Compatibility and performance corpus

The corpus is generated locally and redistributable. The versioned set covers dense tables, non-default page sizes, all four intrinsic rotations, crop boxes, source highlights/comments, embedded Unicode TrueType, and image-only scans. Bulk workloads add a 300-page table document and a 96 MiB synthetic scan. Font provenance and its full redistribution license accompany the fixtures; no external PDF, photo, private record, or third-party document text is used.

The included unmodified DejaVu Serif font exercises Latin accents, Greek, Cyrillic and mathematical symbols. Its licensing notice is preserved under `tests/fixtures/corpus/fonts`; the [upstream license](https://dejavu-fonts.github.io/License.html) permits redistribution with its notices. This is not a comprehensive PDF standard, complex-script shaping, OCR, malicious-document, or arbitrary third-party-reader conformance suite.

## Coverage for issue 6

| Fixture | Size / pages | Checks exercised |
|---|---|---|
| `dense-tables.pdf` | 36 KiB / 3 | Table lines, dense source text, copy/extraction, reverse order and editable export/reopen |
| `mixed-geometry.pdf` | 6.5 KiB / 4 | Different page sizes, crop insets, intrinsic 0/90/180/270 rotations, source highlight/comment geometry and color/opacity preservation |
| `embedded-font.pdf` | 224 KiB / 2 | Bundled TrueType embedding, Latin accents, Greek, Cyrillic and math extraction before/after export; source visual inspection |
| `scan-small.pdf` | 644 KiB / 1 | Image decoding, empty source text, editable overlays on an image page |
| `dense-300-pages.pdf` | 3.47 MiB / 300 | Page-count pressure, 1.482M extracted characters, sequential render workload, all-page reorder/export/reopen and process memory |
| `scan-40-pages.pdf` | 96.19 MiB / 40 | Separate 200dpi image pages, large file/buffer pressure, sequential decoding, edited export/reopen and process memory |

The small PDFs and their font/license inputs are versioned. The two bulk PDFs are generated into ignored artifacts. The packaged test uses the two bulk fixtures for measured scroll and warm-tab/cache behavior; its results are reported separately from the native measurements.
## Reproduce

Development dependencies are separate from the app:

```powershell
python -m pip install -r scripts/corpus-requirements.txt
python tests/corpus_generator_test.py
python scripts/corpus-generate.py
python scripts/corpus-generate.py --large
python scripts/corpus-verify.py tests/fixtures/corpus
python scripts/corpus-verify.py artifacts/corpus
cargo test --manifest-path src-tauri/Cargo.toml --test corpus --offline
pwsh -NoProfile -File scripts/corpus-benchmark.ps1 -Runs 2
```

Regeneration uses the exact bundled font, fixed random seed, fixed PDF metadata and no random PDF IDs. Tests verify repeated output bytes, embedded fonts, expected source text, image-only scan semantics, rotations/crop and versioned SHA-256 hashes. Toolchain versions are pinned and recorded: Python 3.14.0, PyMuPDF 1.27.1, Pillow 12.1.0, and pypdf 6.14.2 for independent verification in this baseline (the manifest is authoritative). The generator normalizes supplementary glyph ToUnicode mappings to UTF-16BE surrogate pairs; an independent-reader warning exposed this PyMuPDF serialization issue, and a regression now checks complete code units. Compression or serialization can change under another toolchain; review manifest changes before accepting regenerated fixtures.

`tests/fixtures/corpus` contains the small PDFs, font, licenses and manifest. `artifacts/corpus` is ignored and contains the large generated files. The large generator also includes font/content licenses so the output directory can be redistributed intact. The scan uses forty independent seeded paper-noise images at 1700x2200 pixels, with synthetic raster text and no OCR layer. Independent images avoid a misleading tiny PDF that reuses one image forty times.

## Native checks and timing method

The default Rust corpus integration test covers the four small fixtures. It opens the real Pdfium worker, verifies page sizes and sampled source text, renders at 900 pixels wide, adds highlight/comment/text/ink edits, reverses all pages, exports, reopens, checks source text and annotation geometry, and confirms the source file is unchanged.

The ignored bulk test additionally renders 24 consecutive pages sequentially, repeats the first render, extracts every page, and records open/export/reopen timing. These are native operations through Folio's worker queue, including PNG encoding; sequential render timings approximate the engine work during scrolling. They do not measure WebView layout or end-to-end scroll readiness.

The PowerShell runner validates every input against its SHA-256 manifest before and after testing, builds a release test executable, and launches a fresh process for each fixture and run. It records the test executable hash, repository revision, CPU, RAM, OS, Rust version and power plan. Memory fields follow Microsoft's [PROCESS_MEMORY_COUNTERS_EX definitions](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex). The Windows test samples private memory every 10 ms and records the OS process peak working set. Peaks cover the entire workflow, including simultaneous original/exported documents during reopen. Private-memory peaks can miss allocations shorter than the sampling interval. Working-set peaks include the test harness and PDF library. No physical printing occurs.

Results are written to the chosen output directory as `summary.json`, per-run JSON files and `machine.json`. The final release implementation was measured in `artifacts/corpus-results-final`; its results and implementation/test source hashes are versioned in `baseline-v0.6.json`. The six measured runs use strict editable text/ink roundtrip checks. The default small test also verifies source annotations and embedded Unicode extraction after reordering/export.

## Measured v0.6 native baseline

Windows 11 Pro build 26200, AMD Ryzen AI 9 HX 370 (12 cores / 24 logical processors), 95.3 GiB physical RAM, Balanced power plan, release Rust build. Three fresh processes per fixture. Filesystem caches were warm and other release work shared the machine, so these are observed ranges, not promised performance limits.

| Workload | Open | First 900px render | Extract every page | Reverse + edit export | Reopen | Peak working set |
|---|---:|---:|---:|---:|---:|---:|
| 300 table pages, 3.47 MiB, 1.482M characters | 201–210 ms | 8–11 ms | 592–1,017 ms | 567–767 ms | 387–501 ms | 86.6 MiB |
| 40 image-only pages, 96.19 MiB | 75–131 ms | 36–52 ms | 0.9–2 ms | 263–453 ms | 104–113 ms | 528.9 MiB |

All six measured native runs passed source integrity, editable text/ink roundtrip, annotation geometry and reordering checks. Across 72 sequential renders per fixture, mean/p95 were 9.17/11.52 ms for tables and 39.07/51.11 ms for scans. Peak sampled private memory was approximately 75.5 MiB for the table document and 518.8 MiB for the scan. The larger scan peak reflects the full export/reopen workflow, rather than only a single visible page; it is a baseline to compare and investigate, not a low-memory guarantee.

No absolute latency or memory pass/fail threshold is imposed before establishing repeatable baselines. For regression review, use the same machine/power profile, fixture checksums, release profile and sample counts, run multiple samples without concurrent heavy work, and compare individual phases as well as total peak memory. Keep correctness failures as release blockers; investigate material performance changes with their raw samples and workload sizes.

## Packaged desktop workload

After staging the candidate under `artifacts/Folio-test-0.6`, launch an isolated test app. The launcher uses a new recovery directory and WebView profile, and records the binary SHA-256 and process ID:

```powershell
pwsh -NoProfile -File scripts/corpus-start-desktop.ps1
$testApp = Get-Content -Raw artifacts/corpus-ui/process.json | ConvertFrom-Json
$env:FOLIO_APP_PID = $testApp.pid
$env:FOLIO_CDP_URL = 'http://127.0.0.1:9239'
node scripts/corpus-tabs-desktop.mjs
node scripts/corpus-small-desktop.mjs
node scripts/corpus-retention-desktop.mjs
```

This opens both bulk documents through the native file dialog, checks raster and selectable text readiness, measures twelve warm tab switches, observes native render/text IPC requests to verify cache reuse, scrolls through six target pages and back, checks that page virtualization remains active, and records WebView performance counters and errors. Memory snapshots include the native app and descendant WebView processes before opening, after each open, after warm switching and after scrolling; these are current-tree samples, not a claimed simultaneous peak. It leaves the app open for further smoke tests. Measurements include Playwright polling/IPC; open time also includes dialog automation, so compare them only with the same harness. It does not silently claim native render latency as UI latency. Results and a screenshot are written to `artifacts/corpus-ui`.

## Measured packaged desktop baseline

The candidate SHA-256 was `0bab799243f9173f20196b154a91159e8caa0c8f604a9ba55668794ee1b50ea3`. This was one complete run in a new recovery/WebView profile, with both bulk documents open. Measurements include Playwright/IPC/polling overhead and two animation frames; opening also includes the native dialog helper. They are distinct from the native operation timings above.

| Workload | Dialog + open to first page | Six warm switches, range / mean | Seven scroll targets, range / mean |
|---|---:|---:|---:|
| 300-page table document | 4,732 ms | 666–803 / 746 ms | 192–898 / 534 ms |
| 40-page scan document | 2,699 ms | 109–161 / 131 ms | 36–215 / 91 ms |

The observer saw real native render/text IPC while opening and **zero repeat render/text requests across all twelve warm switches**. Only two or three page canvases were mounted while scrolling. The dense view mounted 9,880–14,820 source glyphs; scans contained no selectable source text. No JavaScript errors occurred. These measurements establish a reproducible dense-text responsiveness baseline; they do not claim every warm switch completes within one frame.

The subsequent small-fixture run used actual mouse selection and Control+C. The text/plain copy payload matched a dense table cell, Latin accents (`café naïve Straße Ångström`), Greek, Cyrillic, mathematical symbols, and a mixed-page title. All four mixed-page SVG view boxes matched independently checked crop/rotation dimensions. Screenshots are under `artifacts/corpus-ui`.

Memory snapshots sum the native app and its WebView descendants. Working sets may count shared pages more than once; process/renderer/GPU allocations and allocator retention are included. These are current snapshots, not a measured simultaneous unique-memory peak.

| Stage | Tree working-set sum | Tree private-memory sum | Native private memory | JS heap used |
|---|---:|---:|---:|---:|
| Before opening corpus | 656 MiB | 276 MiB | 6 MiB | Not sampled |
| After bulk scrolling | 2,165 MiB | 1,743 MiB | 240 MiB | 29.5 MiB |
| After small checks, before closing five clean tabs | 1,221 MiB | 744.5 MiB | 244.8 MiB | 11.89 MiB |
| All tabs closed, about 3.6 seconds later | 998 MiB | 516.8 MiB | 22.3 MiB | 6.86 MiB |
| After another 10 seconds idle, about 16.3 seconds since close | 920 MiB | 437.3 MiB | 21.8 MiB | 6.88 MiB |

No garbage collection was forced. The footprint had already fallen between the bulk and small runs, and fell further after closing the tabs. The final empty view contained 857 DOM nodes. The high active-workload footprint and dense tab latency warrant follow-up profiling; these snapshots alone do not establish a memory leak. No performance threshold or low-memory guarantee is inferred from this machine.
## Verification evidence

- `python tests/corpus_generator_test.py`: 2 tests passed, including repeated byte identity, committed checksums, font/image/geometry semantics and UTF-16BE ToUnicode regression.
- `scripts/corpus-verify.py`: independent pypdf 6.14.2 verification passed all six fixtures; SHA-256, page geometry, embedded Latin/Greek/Cyrillic/math extraction, source annotations, and scan image/no-text semantics checked. Reports: `artifacts/corpus-results/independent-small.json` and `independent-large.json`.
- `cargo test --test corpus --offline`: small corpus passed (4 fixtures), bulk benchmark ignored by default; included in the native owner's complete 45-test ordinary suite.
- Final release bulk benchmark: 6 passed runs, 300 pages / 96 MiB workloads; results in `baseline-v0.6.json`.
- Packaged desktop: bulk tab/cache/scroll checks passed, six real copy checks passed, four mixed geometry checks passed, and natural post-close memory snapshots completed. Versioned evidence: `desktop-v0.6.json`, `desktop-small-v0.6.json`, `retention-v0.6.json`. The isolated empty app was closed normally afterward.
