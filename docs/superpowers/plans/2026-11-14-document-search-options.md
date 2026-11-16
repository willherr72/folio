# Document search options implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline under the
> standing autonomous authorization. Preserve this managed worktree and all old
> artifacts. One final fresh feature review, one Important fix pass, no re-review.

**Goal:** Reliable document-wide Match case and Whole words Find across source,
overlay and protected text, including correct Unicode case-expansion geometry.
**Architecture:** Shared pinned data, equivalent pure match iterators, existing
native location-only search, per-tab React controls and stale-result ownership.
**Tech stack:** TypeScript/React, Rust/Tauri, Python data generation, Windows QA.
**Spec:** docs/superpowers/specs/2026-11-14-document-search-options-design.md

## Global constraints

Unicode17 default full C+F folding, no Turkic T; L/M/N/Pc plus ZWNJ/ZWJ words and
internal ASCII/curly apostrophes; source-scalar expansion edges; literal matching;
existing ECMAScript whitespace;1,024rawUTF8query bytes; no new runtime dependency.
Protected250,000characters/1,000matches/10,000rectangles; preserve permissions and
geometry. No clipboard/private inputs/promotion/Git mutation/cleanup/version bump.
Two frontend workers/default5000ms; serialized Cargo/build/GUI; one Cargo job;
D:/Folio-build-cache/t3-8962a327 and corresponding-temp. Existing052 stays frozen.

## Review focus

- Query/source folding must agree beyond ASCII and retain original glyph bounds
  when folding expands; rejected candidates must not hide later valid starts.
- Protected transport must carry both options without extracted-text disclosure,
  permission bypass, result-budget changes or no-default-feature degradation.
- Option/query changes during source or shaped-overlay work must reject stale
  responses immediately and preserve tab-local queries, flags and navigation.
- Unicode tables must be reproducible from pinned licensed sources, and both
  runtimes must interpret range edges, supplementary text and word joins alike.
- F3/Enter/Escape, composition, tab returns and760px layout must retain correct
  focus, counts, navigation and document history without changing PDF content.

### Task1: shared matching and native options

**Files:** create scripts/generate-search-unicode.py, src/shared/search-unicode.json,
src/shared/UNICODE-LICENSE.txt, src/editor/search-matching.ts,
src-tauri/src/search_matching.rs, tests/fixtures/search/matching.json;
modify src-tauri/src/{engine,page_search,types,lib,app}.rs;
test tests/search-matching.test.ts, src-tauri/tests/restricted_search.rs and
native search_matching unit tests. Retain official inputs/provenance under
tests/fixtures/search/unicode/ and artifact baseline under the new component root.

**Interfaces:** TS `normalizeSearchQuery(query,options?)`,
`searchCharacterRanges(parts,query,options?)` yields `{start,end,offset}` with
glyph-index start/inclusive,end/exclusive and normalized UTF16 offset. Rust
`SearchMatches::new(parts,query,options)` yields glyph-index ranges lazily.
`PageSearchOptions` false-default flags; `PdfEngine::search_page_with_options`.

- [x] Capture052 source/native/checkpoint anchors and exact planned preimages.
- [x] Write shared fixture/model tests; demonstrate existing Greek mismatch and
  absent option behavior RED. Download only pinned Unicode17data/license and
  record hashes; deterministic generator rejects changed inputs under --check.
- [x] Implement the shared tables and matchers; exercise every fold mapping,
  partial-expansion refusal, sigma, literal punctuation and word/range boundaries.
- [x] Add native request/options and compatibility wrapper. RED/GREEN actual
  protected PDF matches with each flag; serialized results contain no characters;
  existing empty/limit/invalid-page/prepared-print/rotation tests remain meaningful.

### Task2: frontend integration and tab-local controls

**Files:** modify src/editor/{types,adapter,search}.ts,
src/components/{SearchBar.tsx,search.css},src/App.tsx;
test tests/{search,restricted-search,search-options-app}.test.tsx and adapter tests.

**Interfaces:** optional `options` fourth adapter argument; optional fifth hook
argument. SearchBar receives flags/onOptionsChange. Hook state keys normalized
query plus both flags and rejects stale source/shaped work. App stores flags per
tab, resets index/reading-follow state on query or option edits, supports F3 wrap.

- [x] RED option controls, hook stale results, raw protected query/options,
  expansion/overlay rectangles and invalid-query no-dispatch cases.
- [x] Use the pure matcher for source/plain/shaped overlays; preserve cache and
  crop/rotation behavior. Add option-sensitive asynchronous ownership.
- [x] RED/GREEN two-tab isolation, reset after option change, keyboard close/wrap
  and composition handling; add responsive labeled checkboxes without modal flow.
- [x] Run focused regressions, full frontend two-worker suite and production
  TypeScript/Vite build. Capture exact commands/results; no timeout increases.

### Task3: native/runtime acceptance and final review

**Files:** authored fixture/controller/receipts under
artifacts/document-search-options-20261114; ledger at
.superpowers/sdd/2026-11-14-document-search-options/progress.md.

- [x] Freeze final product/native/shared inputs; complete library correctness
  and affected optimized restricted_search/text_geometry/ocr_engine targets;
  check default/no-default features, source/input guards and table reproducibility.
- [x] Build a normal optimized current GUI with exact26resources and record its
  source/binary identity; no portable build or selected-launcher change.
- [x] Run owned isolated Windows workflows on authored PDFs: case/word counts,
  sigma/expansion bounds, protected location-only search, tab isolation, Enter/F3,
  Escape/composition, rotation/zoom and760px layout; assert geometry/count/focus,
  no document edits and owned cleanup. Preserve all failed attempts.
- [x] Freeze exact scoped diff/evidence; obtain one fresh final feature review.
  Rule on declined judgments, defer minors, fix Critical/Important findings once
  with RED/GREEN and the complete relevant suite; no re-review.
- [x] Record verified component completion and remaining limits with preserved
 052package/selected050/prior051 identities; keep broad goal active.
