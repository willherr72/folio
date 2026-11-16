# Desktop asset build boundary implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. One combined final review, no per-task dispatch.

**Goal:** Prevent frontend-only changes from recompiling the optimized PDF engine.

**Architecture:** Construct Tauri context in the GUI binary and pass it to the
existing library startup. Prove the boundary through actual Cargo build artifacts
and dep-info, then exercise the rebuilt GUI using public fixtures.

**Tech Stack:** Rust, Tauri 2, Cargo, Python diagnostic drivers, Windows CDP QA.

**Spec:** `docs/superpowers/specs/2026-11-13-desktop-build-boundary-design.md`

## Global Constraints

- Normal optimized custom-protocol GUI, one Cargo job, existing external cache.
- No version, feature, dependency, renderer, native editing or resource changes.
- Preserve selected 0.49 and its launcher; no commit, deletion or public release.
- Restore generated CSS exactly; bind source, generated assets and all resources.
- A warm same-host build comparison is not general performance certification.

## Review Focus

- Default CLI and headless users must not require GUI asset context construction.
- Generated frontend updates must reach the binary and actual rendered page.
- Tauri resources, commands and startup error handling must remain intact.
- A reused library must be proved by Cargo fresh status, bytes and timestamp.
- Candidate and runtime receipts must identify the final restored assets.

### Task 1: Establish the controlled baseline

**Files:** diagnostic drivers under `artifacts/desktop-build-boundary-20261113/`;
scoped source snapshots and `.superpowers/sdd/2026-11-13-desktop-build-boundary/progress.md`.

**Interfaces:** Produces immutable baseline/asset snapshots and Cargo receipts.

- [x] Capture current three Rust startup files and full product/asset inputs.
- [x] Run the build-boundary check against current dep-info. Expected: RED because engine embeds dist assets.
- [x] Apply a harmless generated CSS custom property, run ordinary optimized GUI build, preserve log, duration, engine and binary evidence; restore CSS. Expected: engine rebuilt.

### Task 2: Move context and measure library reuse

**Files:** Modify `src-tauri/src/main.rs`, `src-tauri/src/app.rs`; diagnostic driver and assertions.

**Interfaces:** `pub fn run(context: tauri::Context<tauri::Wry>)`; caller provides `tauri::generate_context!()`.

- [x] Make the minimal startup change; leave builder/error handling intact.
- [x] Build restored assets normally. Expected: compiled GUI, engine dep-info contains no dist assets, binary dep-info does.
- [x] Change the same generated CSS property, rebuild, and verify Cargo engine freshness plus unchanged library hash/time and binary asset dependency. Expected: GREEN, exact marker rendered in GUI.
- [x] Restore generated CSS and rebuild final candidate. Expected: unmodified generated asset hashes, source stable, no probe marker.
- [x] Build default CLI normally and inspect a public saved PDF. Expected: exit 0.

### Task 3: Verify runtime and review

**Files:** Candidate with exact complete resources, runtime/audit receipts, final review report.

**Interfaces:** Consumes final source/generated-asset map and actual candidate.

- [x] Run actual isolated desktop text/paragraph/save/reopen and offline OCR workflow, keeping existing assertions. Expected: native receipts, saved-PDF independent checks and owned cleanup pass.
- [x] Run one fresh scoped final review of implementation and evidence. Resolve Important findings in one test-first fix pass; preserve all original evidence.
- [x] Record bounded build measurements, limitations and next work in completion/ledger and project quality log. Keep broad goal active and 0.49 selected.
