# Page render scheduling implementation plan

> REQUIRED SUB-SKILL: superpowers:executing-plans inline. Standing autonomy replaces routine confirmation; one final scoped review after acceptance.

**Goal:** Keep navigation responsive by cancelling obsolete queued page renders without changing displayed PDF content.
**Architecture:** Lease-owned bounded PageImageCache; existing usePageImage adapter and native/SVG display preserved.
**Tech Stack:** TypeScript/React18/Vitest; nativeTauriWindows GUI acceptance.
**Spec:** docs/superpowers/specs/2026-11-11-page-render-scheduling-design.md

## Global Constraints

Two adapter renders in progress;60idle-cache entries/96MiB decoded estimate;
mounted ownership preserved. No display/PNG/native edits. Keep044selected and
all failed evidence. No commits/push/merge/release/worktree deletion. Native/build/
UI serialized; CARGO_BUILD_JOBS=1,D:/Folio-build-cache/t3-8962a327 and-temp.
Native reuse is frontend-only exact-native/shared equality with original profiles.

## Review Focus

- Rapid release/reacquisition, including ReactStrictMode, must not display revoked or old-adapter URLs.
- Rejected/synchronously throwing renders and cancelled queue heads must free slots without unhandled errors.
- Closing one consumer or source cannot revoke a still-mounted shared preview.
- Busy navigation must discard obsolete work without starving retained visible pages.
- Actual renderer/request limits and image fidelity must be observed without synthetic backend delays or altered memory/display budgets.

### Task 1: owned bounded raster cache

**Files:** src/editor/page-image-cache.ts;tests/page-image-cache.test.ts;tests/page-render-scheduling.test.tsx.
**Interfaces:** PageImageCache.key(adapter,page,pixels);acquire(adapter,page,pixels)->{key,request,release};clear(sourceIds). Production singleton used byTask2.

- [x] Write hook regression for many simultaneous page requests and abandoned entries. Expected: current code FAILS by issuing more than2adapter calls.
- [x] Write cache tests for concurrency, abandonment, sharing,clear,errors,retry,adapter identity and idle LRU. Expected: missing-module RED, then direct behavior RED as applicable.
- [x] Implement minimal lease scheduler preserving60/96MiB LRU and mounted exceptions. Expected: focused unit regressions pass; no display source changes.

### Task 2: integrate page views and frontend validation

**Files:** src/components/PageView.tsx;tests/render-cache.test.tsx;artifacts/page-render-scheduling-20261111 frontend/build/qualification helpers.
**Interfaces:** Existing usePageImage/clearRenderCache signatures remain stable; newcache owns requests/URLs while hook owns mounted result state.

- [x] Replace immediate render-cache implementation with scheduler leases. Expected: observed hook concurrency RED now GREEN; shared view tests remain green.
- [x] Run focused tests, fresh fullfrontend/default5000ms/two workers and TS/Vite. Expected: no failed tests/build; native/shared inputs unchanged.
- [x] Capture source/native-only equality and build normal optimized GUI candidate with26resources. Expected: actual new executable/source binding,044launcher unchanged; no fresh native claim.

### Task 3: actual navigation/editing and final scoped review

**Files:** artifacts/page-render-scheduling-20261111 baseline/candidate Windows drivers,audits,review,completion;repo quality status.
**Interfaces:** Bound current044 and candidateGUI undergo same public300page navigation; independent frame and raw native PNG comparisons and edit smoke bind their actual binaries.

- [x] Capture current044 baseline passiveIPC render concurrency during fast navigation. Expected: measured actual baseline; source immutable and owned cleanup.
- [x] Run matching candidate navigation and text/paragraph edit/history/save smoke. Expected: at most2page-image render requests in flight, obsolete work absent, final visible pages render, unchanged native rasters/source and no UI errors; exact cases/limits recorded.
- [x] Dispatch one final frozen scoped review, handle consequential findings once with RED/GREEN and required checks. Expected: honest scoped approval/limitations, no re-review.
- [x] Record completed component/evidence and leave044selected. Expected: concrete product fix with verified scope; broad sustained-memory/producer/parity work remains active.
