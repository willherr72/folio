# Text page admission implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, meaningful RED/GREEN, one fresh final review. Standing user autonomy replaces approval questions.

**Goal:** Remove unnecessary native page-content loading after supported text publications while preserving all source, annotation, security and history behavior.
**Architecture:** Writer-owned opaque byte/count/hash proof of no Annots keys; matching proof enables existing PDFium size-only API, otherwise full admission. No public schema or global cache change.
**Tech Stack:** Rust/lopdf/PDFium/Tauri, existing Python/PowerShell/Node evidence helpers.
**Spec:** docs/superpowers/specs/2026-11-10-text-page-admission-design.md

## Global Constraints

Preserve039CP24eb14/launcher and all originals/failed attempts. Proof eligibility32..100000pages/encoded<=8MiB/no encryption state, census<=100000Object values/depth64/no Annots key; failure means fallback. All existing parse/resource/mutation/raster/font/policy checks remain. Source decoded/encoded64MiB, encrypted backing and100history states unchanged. Native profiles retain original meanings; exact frontend qualification only. Serialize compilers/tests/UI; one Cargo job/D:cache/temp. No source edits during builds. One final reviewer only; no commit/push/publicrelease/app autolaunch. Private quiz coordinator-only.

## Review Focus

- Malformed, nested, null or indirect annotation data must never authorize empty overlays or lost imported content.
- A proof must be bound to the exact writer output and renderer page count; stale/cross-source or externally supplied metadata cannot authorize admission.
- Crop/origin/rotation/UserUnit and unusual renderer sizes must preserve existing PageInfo and fallback behavior.
- Uncertified and annotation-bearing publications must retain all font/image/import/policy/retention and rollback semantics.
- Performance and memory acceptance must use unchanged workloads, faithful outputs, honest profiles/order and original full history/sample bounds.

### Task 1: writer-owned proof and native geometry

**Files:** Create src-tauri/src/page_admission.rs and page_admission_tests.rs; declare module in engine.rs; artifacts/text-page-admission-20261110 baseline/RED/GREEN records.
**Interfaces:** Publication::save(&mut lopdf::Document, bool)->EngineResult<Publication>; into_bytes and into_parts; private Proof::matches(&[u8],usize); native_size(&PdfDocument, PdfPageIndex)->Option<PageInfo>. Private fields, no Serialize/Deserialize or public input authority.

- [ ] Preserve current source/test inputs and all modified existing files,039authority and diagnostic baseline. Verify existing isolated worktree. Record preflight Task1proof→Task2opener→Task3actual outputs→Task4seal interfaces.
- [ ] Write failing tests for eligible output, byte/count/tamper binding, small/oversized/encrypted cases, Annots in trailer/direct/nested/stream/null/indirect locations, limits, native crop/origin/rotation/inheritance/UserUnit equality and invalid-index fallback. Use a compilable no-proof/no-size stub; observe functional RED.
- [ ] Implement bounded census, opaque output proof and native size query. Run full page_admission tests GREEN using release dependencies/Folioopt0 library correctness; retain output and exact source/input fingerprints. Expected: all new tests pass without relaxing any input or budget.

### Task 2: exact-text publication and engine admission

**Files:** Modify src-tauri/src/page_content_replace.rs, occurrence_text_edit.rs, engine.rs; add focused integration assertions where existing coverage lacks the new path.
**Interfaces:** Exact text wrappers return Publication; general replacement extracts bytes. open_text_publication consumes opaque result; open_bytes_scoped delegates to common opener with no proof. Common opener validates proof once and tries native_size before unchanged full-page admission.

- [ ] Wire writer factory after all existing publication checks and pruning; preserve generic bytes API. Carry proof only through exact text publication. Compare old/new opener PageInfo using native matrix and annotation-bearing fallback fixtures; watch meaningful failing behavior before wiring, then GREEN.
- [ ] Run affected text/context/form/paragraph/physical-unit/overlay/import/retention tests and full native gate under original optimized integration/Folioopt0 library profiles. Check source/native inputs before/after; no stale native count or source reuse. Expected:0failures, explicit skips, no schema/dependency/renderer/history-policy change.

### Task 3: measured benefit and normal desktop acceptance

**Files:** New candidate probe/GUI build, balanced before/after native timings/fidelity, normal desktop resource/history/handoff evidence under artifacts/text-page-admission-20261110.
**Interfaces:** Task2source becomes candidate; baseline is diagnostic b977/source d327 and accepted039GUI3983, never recycled times. Outputs include exact source/binary/helper/fixture identities and independent audits.

- [ ] Build same optimized diagnostic probe on candidate; run fresh balanced-order baseline/candidate recorder-off/observed300/2page workloads, validate trace controls and all saved pages. Target>=10% large repeated replacement improvement and no meaningful small regression; inconclusive/failing evidence means investigate, not lower gates.
- [ ] Build fresh normal optimized profiling-disabled GUI with full accepted resource tree and strict source guards. Run unchanged quiz and separate300page105/100/100, all300saved-page source/golden glyph/style/RGB audits, original firstsample512MiB/1GiB limits, three handoff cases and exact ownership cleanup. Qualify unchanged frontend694inputs/1517tests explicitly; no fake fresh frontend claim.

### Task 4: review and immutable implementation checkpoint

**Files:** Frozen review package/report/dispositions, final source/input/helper/binary/output audit and historical verifier/checkpoint.
**Interfaces:** Consume terminal Task3evidence; produce reviewed implementation checkpoint. Accepted039launcher stays until a later independently qualified combined portable.

- [ ] One fresh final reviewer against concrete scope and five focus classes, public-only context and private coordinator aggregate. Re-grade effects, fix consequential issues inline with RED/GREEN/affected suites, record every minor/declined/ruling/limit.
- [ ] Seal historically independent evidence, verify all pins and unchanged039launcher/package. Preserve worktree/ledger and unsuccessful attempts; no git integration/public release. Keep broad goal active; do not certify general speed/memory/printing/parity.
