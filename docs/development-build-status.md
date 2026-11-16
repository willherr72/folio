# Development build status

Repository status as of 2026-11-16. Documents inside an existing archive remain
its integration-time snapshot; this guide does not alter those frozen bytes.

| Build | Current state | Evidence |
| --- | --- | --- |
| 0.56.0-dev.1 portable | Reviewed separate development portable; full finite history/memory/fidelity passed | [Completion](../artifacts/portable-056-20261116/completion.json), [checkpoint](../artifacts/portable-056-20261116/sealed/checkpoint.json), [ZIP](../artifacts/Folio-v0.56.0-dev.1-windows-x64.zip) |
| Compact reading source candidate | Reviewed separate normal GUI; finite full-history memory/fidelity workload passed | [Completion](../artifacts/compact-reading-data-20261115/completion.json), [review](../artifacts/compact-reading-data-20261115/review/review.md), [GUI](../artifacts/compact-reading-data-20261115/build-2/candidate/Folio.exe) |
| 0.50.0-dev.1 | Qualified local portable selected by Launch Folio.cmd | [Selected checkpoint](../artifacts/portable-050-20261113/sealed/checkpoint.json) and [launcher manifest](../artifacts/folio-launch-target.json) |
| 0.51.0-dev.1 | Preserved reviewed separate candidate | [Checkpoint](../artifacts/portable-051-20261113/sealed/checkpoint.json) |
| 0.52.0-dev.1 portable | Completed reviewed separate candidate; not selected | [Completion](../artifacts/portable-052-20261114/completion.json), [checkpoint](../artifacts/portable-052-20261114/sealed/checkpoint.json), [ZIP](../artifacts/Folio-v0.52.0-dev.1-windows-x64.zip) |
| 0.53.0-dev.1 portable | Completed reviewed separate development candidate; not selected | [Completion](../artifacts/portable-053-20261115/completion.json), [checkpoint](../artifacts/portable-053-20261115/sealed/checkpoint.json), [ZIP](../artifacts/Folio-v0.53.0-dev.1-windows-x64.zip) |
| 0.54.0-dev.1 portable | Completed reviewed separate candidate; not selected | [Completion](../artifacts/portable-054-20261115/completion.json), [checkpoint](../artifacts/portable-054-20261115/checkpoint.json) |
| 0.55.0-dev.1 portable | Reviewed separate development portable; not selected | [Completion](../artifacts/portable-055-20261115/completion.json), [checkpoint](../artifacts/portable-055-20261115/sealed/checkpoint.json), [ZIP](../artifacts/Folio-v0.55.0-dev.1-windows-x64.zip) |
| 0.55 sustained editing observation | 105/100/100 history and 300-page fidelity passed; lifetime memory target failed (1,044.44 MiB versus 1,024 MiB) | [Measured finding](../artifacts/packaged-memory-highwater-20261115/completion.json) |
| Deferred reading-geometry source candidate | Reviewed normal GUI and full fidelity passed; memory target still missed by 1.35 MiB | [Completion](../artifacts/editing-memory-attribution-20261115/completion.json), [GUI](../artifacts/editing-memory-attribution-20261115/build-1/candidate/Folio.exe) |
| Workspace Find/Replace source candidate | Completed reviewed GUI/CLI test build; separate from portable packages | [Completion](../artifacts/desktop-replacement-20261114/completion.json), [checkpoint](../artifacts/desktop-replacement-20261114/checkpoint.json) |

The selected 0.50 already includes CLI/MCP page organization and editable
multi-document composition. It retains the earlier editing, forms, image, OCR,
recovery and protection workflows within their recorded native admission limits.

The 0.52 portable adds verified source-bound ligature editing in eligible CID
TrueType fonts, source-font support without an intrinsic Unicode cmap and clearer
keep/substitute guidance. It retains paragraph Find/Replace and staged multiline
Paste as paragraph. Native Preview/Apply still proves font coverage, fitting and
publication. See the [source-font scope](source-font-editing-status.md).

The 0.52 package completed actual extracted GUI/CLI/MCP acceptance. Its reviewed
7,452-file seal passed 50 final corruption/omission controls, relocation and
archive-only verification. Fresh frontend checks passed 1,663 tests in 236 files.
Native evidence is qualified reuse: 1,655 library passes/11 ignored with release
dependencies and Folio opt-level 0 correctness, plus 152 ordinary optimized
integration passes/2 ignored. Separate 38/0 ligature and 41/1 paragraph correctness
results retain their original scopes. These are not fresh 0.52 native tests.

The newer source candidate adds document Find/Replace across the current workspace,
including imported/duplicate pages, editable added text, crops/rotations, checked
preview and one-step Undo. Atomic CLI/MCP replacement uses the same native
preservation rules. Unsupported selected matches block the whole request until
an explicit subset is checked. See [Find/Replace](document-find-replace.md).

Current-source verification passed 1,735 fresh frontend tests in 245 files and193
ordinary optimized integration tests across 15 targets (one diagnostic ignored).
Actual Windows workflows and independent3 PDF/8 page checks passed, with full 144 dpi
raster equality against source-authored references and11corruption controls.
The reviewed 28-file component and 533-member seal include the corrected Save
language preservation. The complete1745/11 library result predates that two-file
Save correction; it is prior correctness evidence, not a current-full-library or
fresh 0.53 result. Native version-only reuse in053 retains these exact meanings.

The separate 0.53 portable completed extracted runtime, packaging review and
standalone acceptance. It retains 28 base stages, eight page/composition stages,
font and paragraph workflows, and adds document Find and desktop/CLI/MCP
replacement. Replacement evidence covers 13 saved PDFs/28 pages, 37 CLI calls,
11 refusals and 18 corruption controls. The full 105-edit/100-Undo/100-Redo and
300 saved-page checks passed the original sampled 512 MiB native/1 GiB process-tree
limits. This does not establish continuous peaks or leak freedom.

The reviewed 8,534-file seal passed 58 final rejection checks and both relocated
directory and archive-only verification. The single final review approved its
pinned scope; one subsequent archive-path correction retains Windows-spelled
review pins and the exact existing non-UTF filename proof. The correction passed
61 focused and seven standalone tests. The selected 0.50, reviewed 0.51, completed
0.52 and launcher remain unchanged. Each archive retains its build-time
documentation snapshot; this live guide supplies the completed integration state.

Physical system Ctrl+V and installed IME acceptance remain open. Earlier
clipboard-access observations were historical and do not establish current
availability or acceptance. The preservation helper could not retain all observed
formats, so the clipboard was left unchanged. Compiled injected-event checks
cover paste handling, caret/font ownership, hard/blank lines, Undo and save/reopen;
they do not prove real system input. No clipboard probe was performed for this
guide refresh, and no user clipboard data belongs in a package.

Broader producers/scripts/readers/DPI, physical printing, accessibility,
signatures, continuous memory peaks/leaks and low-memory hardware remain
unqualified. The historical OCR reopen timeout's cause remains unproved despite
later successful runs. No public-release or Adobe/Foxit-parity approval is claimed.

Current source repair — 2026-11-15: the separately built CID cursor component
restores safe original-font retained-code deletions and saved re-edit while
preserving original/shared font resources and following text. Actual optimized
GUI/CLI checks, restored whole-page pixels, 15 native saved-pair audits and fresh
1,735/245 frontend tests passed. Native qualification is an explicit 3,043-pass,
32-skip composite: the full current-production run's sole obsolete refusal oracle
was replaced by its complete corrected target, without a production change.
One final scoped review found no blocking product issue; a reproduced selected
spacing oracle gap was strengthened afterward, and a stale comment is deferred.
[Component completion](../artifacts/cid-dependent-cursor-20261115/completion.json)
and [review dispositions](../artifacts/cid-dependent-cursor-20261115/review/disposition.json)
retain exact meanings and limits. This source candidate is separate from the
unchanged portable builds and selected launcher; physical-input and parity gates
remain open.

Current reader source component — 2026-11-15: the newer normal optimized GUI
also separates selected mounted page text during copying, retaining original
CR/LF/CRLF/spaces, scalar safety, crop visibility and permission/viewport ownership.
Fresh frontend1,747/246 and nine Windows DOM-selection/detached-copy observations
passed. The final scoped review found no findings. Native/shared input hashes
match the prior CID component; its qualified native composite is reused with
unchanged meanings. This does not qualify physical clipboard/IME, arbitrary
multiple ranges, virtualized/offscreen page completeness or platform parity.
[Reader-copy completion](../artifacts/reader-copy-pages-20261115/completion.json)
and [review](../artifacts/reader-copy-pages-20261115/review/review.md) identify the
newer source test build, which includes the CID repair. Portable versions and
the selected launcher remain unchanged.

Separate development **0.54.0-dev.1** completed its scoped local integration
review and extracted-package acceptance on 2026-11-15. The normal GUI/default CLI
portable includes the reviewed CID deletion/re-edit and reader-copy fixes. All
1,742 extracted files match the archive; 12 offline OCR assets and 567 dependency
records were verified. Fresh frontend passed 1,747 tests across 246 files.

Actual extracted acceptance covers 13 CLI calls (including an older 0.53 saved
CID re-edit), 17 MCP requests and four Windows CID, reader, paragraph/font and
OCR workflows. Independent checks cover actual saved text/font/pixels, a nonempty
10-object tag graph and seven OCR pages with comments/bookmarks. Every owned GUI
loaded the extracted renderer and stopped. No system clipboard operation occurred.

The native 3,043-pass/32-ignore qualified composite retains its original profiles
and test-only correction scope; it is not a new 0.54 native run. This fresh runtime
matrix is representative, not a rerun of every historical 0.53 stage. The archive
retains its build-time pending documentation snapshot; this live guide records the
terminal result. [Completion](../artifacts/portable-054-20261115/completion.json)
and [final review](../artifacts/portable-054-20261115/review/review.md) bind exact
scope. Selected 0.50 and the launcher, reviewed 0.51 and completed 0.52/0.53 remain
unchanged. No public-release or commercial-parity approval is claimed.


Reviewed desktop compatibility candidate — 2026-11-15: exact redundant accented
WinAnsi/MacRoman glyph names, including unused assignments, now retain verified
original-font editing. The normal optimized diagnostic Windows candidate passed
seven workflows and 14 exact saved-output audits after the core native/CLI gates.
The final scoped review approved with no findings. This source candidate is newer
than the completed 0.54 portable and remains separate from the selected launcher.
[Candidate executable](../artifacts/accented-encoding-desktop-20261115/build-1/candidate/Folio.exe),
[completion and limits](../artifacts/accented-equivalent-encoding-20261115/completion.json).

Current direct-paragraph source candidate — 2026-11-15: automatic fitting before
Preview, fixed-area opt-out and corrected provisional-input/history ownership
passed fresh 1,793 frontend tests/249 files, normal optimized build, six Windows
workflows and 12 independently audited saved PDFs. Native evidence is qualified
reuse with exact backend/shared equality; physical IME/input and broader quality
gates remain open. [Completion](../artifacts/paragraph-preview-fit-20261115/completion.json)
binds the diagnostic candidate and one reviewed correction pass. Existing
packages and selected launcher remain unchanged.

Reviewed source-accent-cluster candidate — 2026-11-15: source `1df5fdb303f78978325620a1d66c4c7ffde3216628468de5f74c0dc84dd8ad82`
passed complete library correctness, 35 ordinary optimized affected integration
targets, fresh 1,793/249 frontend, normal optimized GUI/CLI and actual CLI/MCP/Windows
workflows, with 100 independently audited saved outputs. Original Unicode and
physical glyphs are retained; imported-font/general shaping and legacy scalar
paragraph limits remain. [Candidate executable](../artifacts/source-accent-clusters-20261115/build-1/candidate/Folio.exe),
[completion and exact limits](../artifacts/source-accent-clusters-20261115/completion.json).
This is a diagnostic candidate newer than 0.54, not a package/launcher promotion.


Completed separate development portable — 2026-11-15: **0.55.0-dev.1** combines
the reviewed equivalent-encoding, CFF ligature, source-bound accent and
automatically fitted paragraph improvements. Normal optimized GUI/default CLI,
all 1,754 package files, 26 renderer resources, 12 offline OCR assets and 567
dependency records passed ZIP/outside-extraction verification. Fresh frontend
passed 1,793 tests/249 files. The actual extracted 20-stage matrix plus eight
mixed-alias CLI calls passed, with independent font/pixel checks, paragraph
Preview (12 saved PDFs, three real corruption controls), forms (48 saved PDFs,
96 rasters), document Find/Replace (three saved PDFs, eight pages), reader page
boundaries, a nonempty ten-object tag graph and offline OCR (two PDFs, seven pages).
All owned processes stopped.

Native qualification retains exact pre-version source: complete 1,759/11 library
opt0 correctness evidence plus fresh all 150 ordinary optimized integration
targets and two binaries (1,312/21), and separately corrected documentation (0/0).
Total 3,071 passed/32 ignored is composed evidence, not a monolithic green
invocation or fresh 0.55 native execution. Exactly six metadata version tokens
differ. The original failed documentation-selector receipt remains preserved.

The single fresh review identified missing transitive auditor preservation.
The correction captured all six material modules and recorded authorities;
omission/mutation controls and the complete 15-test component suite passed.
The live 0.54 checkpoint link was corrected during documentation finalization.
The finite checkpoint verifies historical bytes after relocation and from the
evidence/package ZIPs; it does not rerun behavior or establish publisher identity.
Archive documents remain their earlier pending-acceptance snapshot; this live
guide records terminal acceptance. Historical long-history/resource workloads
were not freshly rerun. Physical input/clipboard/IME, printing, accessibility,
arbitrary producers/readers/DPI, continuous memory/leaks and 8 GB hardware remain
open. The broad goal continues; selected 0.50 and prior packages are preserved.

Compact reading source milestone — 2026-11-16: inactive uncropped pages receive exact reading strings; Select/Highlight/crops retain rich geometry. The Copy listener becomes ready with newly loaded glyphs. Corrected fresh frontend passed 1,805 tests/252 files. The normal optimized GUI completed 105 edits, 100 Undo, 100 Redo, exact synthetic Copy/Highlight and save/close/reopen. All 300 saved pages and 600 independent raster comparisons passed, with byte-identical PDF output. Original-observer lifetime job commitment was 996.5390625 MiB, below the unchanged 1,024 MiB threshold. The observer used an explicit 8 GiB operational cap; this is a finite job-commit result, with broader memory/hardware/input/printing/parity gates open. Component native execution (1,759/11 complete library correctness with Folio opt0/release dependencies, 82/0 across eight ordinary optimized integration targets) is reused only after proving the final Copy correction changed one frontend effect and no native/shared byte. The scoped review found no blocking defect; one inherited rich-provider replacement cache edge case is deferred. This separate source GUI is linked above; package/launcher selection is unchanged.


Reviewed Folio 0.56 development integration - 2026-11-16: compact reading and commit-ready Copy passed 23 actual extracted editing/automation/forms/OCR/page/composition stages plus eight alias calls. Original full105-edit/100Undo/100Redo/save/reopen workload passed all300glyph pages/600RGB comparisons/eightcorruptioncontrols. Lifetime job commitment1010.6796875MiB<unchanged1024MiB, original8GiBoperationalcap, finaljobempty; finite host commitment, not leak/resident/physical8GB acceptance. Freshfrontend1805/252; native3073/32 retains opt0library reuse and pre-version optimized integration/bin/doc qualifications. Solefreshreview approves developmentmerge, one width-QA validation minor deferred. Targeted Git attributes preserve all288pinned members on Windows checkout. [Completion and dispositions](../artifacts/portable-056-20261116/completion.json). Selected0.50/olderpackages/workspace preserved; broad goal active.

