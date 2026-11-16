# Page-image headroom implementation plan

> Required skill: superpowers:executing-plans inline. Standing autonomy; no routine
> approval questions or per-task agents. One combined final review with the next
> actual portable integration, after verification; no separate duplicate review.

**Goal:** Improve high-DPI resource headroom while preserving rendered quality/history.
**Architecture:** Keep the existing decoded-byte LRU, leases and two-slot queue;
reduce only retained-image bytes, then measure the actual whole-editor workload.
**Tech Stack:** TypeScript/Vitest, normal Tauri build and owned Windows/WebView QA.
**Spec:** docs/superpowers/specs/2026-11-12-page-image-headroom-design.md

## Global Constraints

Baseline source0ae866d973ad18129343521bbb56ca220dbfc93f7372f371b30f14e13b919458
is failed047 code, not an accepted portable. Only page-image-cache.ts changes
96MiB->32MiB and styles.css loses the modal backdrop blur(3px) property, plus meaningful frontend tests. Full resolution/mounted leases,
60entry/tworender bounds/history100 remain. Native512MiB/tree1GiB target unchanged.
Preserve047failure/046selection; no product/native/shared/renderer/dependency drift,
forcedGC/DPR emulation/style injection/deletion/commits/publication/normal app launch.
D: caches/temp, serialize native/build/UI. Native full2784/32 remains prior reuse.

## Review Focus

- Byte pressure must remove oldest idle URLs, not merely count compressed PNG bytes.
- Active/shared borrowers retain exact URLs and pixels above the idle allowance.
- Cold revisits remain valid, with at most two native renders and safe late cleanup.
- Modal dimming, geometry, focus and keyboard behavior remain; temporary injected
  diagnostics cannot stand in for fresh production styling/build acceptance.
- Actual resource assertions retain failed attempts and current natural display;
  a green unit test or passing repeat is not a causal or broad memory-quality claim.
- New candidates/packages bind exact source and repeat full history, independent
  saved output and existing editing/form/OCR checks before local selection.

### Task 1: constrain retained decoded images

**Files:** src/editor/page-image-cache.ts, tests/page-image-cache.test.ts;
artifacts/page-image-headroom-20261112 baselines/tests/records.
**Interfaces:** unchanged acquire/key/clear/lease contracts, cache pruning only.

- [ ] Capture failed047 source and exact cache/tests before edits. Expected: known
  source and unchanged native/shared resources; no live native/UI session.
- [ ] Add decoded-pressure/recent-reuse and active-large/shared-lease tests. Run
  first. Expected: RED from excess idle retention, not configuration/import errors.
- [ ] Change only96MiB to32MiB; run cache and PageView/resource suites. Expected:
  GREEN, recent reuse, mounted protection, two-slot scheduling and cleanup intact.

### Task 2: verify actual resource headroom and integrate

**Files:** guarded next-candidate/portable build and current-display QA records.
**Interfaces:** exact tested cache delta -> normal optimized binary -> same full
105edit/100Undo/100Redo, independent300page fidelity and original resource targets.

- [ ] Run fresh full frontend/default5s/two workers and guarded normal build.
  Expected: all pass, exact cache delta and explicit native reuse.
- [ ] Run current-display full workload, source/font/pixels/history/cleanup checks.
  Expected: same controls, native<512MiB and tree<1GiB with headroom; retain failures.
- [ ] Complete fresh extracted integration and one combined scoped final review,
  standalone seal/controls and local selection. Expected: no use of failed047 as
  acceptance, no parity/goal-completion claim.
