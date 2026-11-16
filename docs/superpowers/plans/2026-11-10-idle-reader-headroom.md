# Idle reader headroom implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing autonomy supplies routine decisions. No per-task agents; one final fresh review after concrete acceptance.

**Goal:** Test and verify two idle PDFium readers with intact source/history ownership and useful high-DPI memory headroom.

**Architecture:** Existing post-request LRU eviction/cold reload, default4to2; source/backing/crypto/rendering unchanged. Preserve040's failed first sample and039reference. A new source authority requires fresh native checks and a new optimized GUI; no metadata-only reuse of the changed core.

**Tech stack:** Rust/PDFium, existing source storage and Windows Python/PowerShell/Node acceptance.

**Spec:** docs/superpowers/specs/2026-11-10-idle-reader-headroom-design.md

## Global constraints

Keep original512MiB native/1GiB tree after105targets,100Undo/Redo and full source backing. Encoded64MiB/decoded64MiB source cache unchanged. No renderer/flags/styles/DPR/GC intervention, gate replacement, package overwrite, commit/push/publicrelease or userapp launch. Serialize compilers/tests/UI, oneCargojob, D:buildcache/temp. Preserve all prior failures and actual source/profile meanings. Existing linked worktree and all ledgers/evidence retained. Failed040qualification is not relabeled;039launcher stays until a separately qualified package.

## Review focus

- Trim only after request-local references drop; active multi-source operations can exceed the idle bound safely.
- Metadata-only reads cannot reopen readers or change source identity, raw bytes, permissions or annotation/font/image ownership.
- Recent-reader order, cold reload and failed-replay retry remain correct with only two idle slots.
- Native/profile/source/resource evidence must belong to the changed core and fresh GUI; no relabeling040native reuse.
- Original current-natural-display workload/first samples/fixed source-based fidelity masks and full history stay unchanged; finite selected results cannot certify general GPU ownership, peaks or leak freedom.

### Task 1: behavioral RED/GREEN for two idle readers

**Files:** `src-tauri/src/native_residency.rs`, `src-tauri/src/native_residency_tests.rs`, `artifacts/idle-reader-headroom-20261110/{baseline/,focused-red-1/,focused-green-1/,source/}`.

**Interfaces:** `trim_native_documents()` retains at most two idle native readers after each worker request while all `SourceHandle`s remain valid. `ResidentDocument::get()` preserves cold reload/replay semantics.

- [ ] Preserve exact6250prechange source/native/frontend maps, two source files, failed040functional/memory receipts, matching039reference and resource identities. Start this plan's ledger/brief.
- [ ] Change behavioral residency/recency expectations first: ten retained sources release eight native reader owners, exact byte leases and metadata remain; a recently rendered old source stays while the true older victim drops. Run focused residency tests against production4. Expected: only the tightened owner-count/recency requirements fail, with source/input guards passing.
- [ ] Change only idle default4to2. Run focused residency and retained-source tests. Expected: all pass, including protected metadata, imported overlays/popups, fonts/images, retry and cold-source authority. Record actual source/input fingerprints and whole delta.
- [ ] Complete task with a read-only RED/GREEN/unchanged-source-backend contract. No performance claim from correctness-profile tests.

### Task 2: fresh native and normal high-DPI GUI proof

**Files:** New `artifacts/idle-reader-headroom-20261110/{native-full-1/,gui-build-1/,desktop/,native-summary.json,desktop-quality.json}` and explicit helper/resource/frontend qualification.

**Interfaces:** Task1 source fingerprint feeds fresh native correctness and fresh optimized GUI. Exact unchanged frontend inputs qualify1517/226reuse. Candidate identity uses its own executable and all25verified040runtime resources; no claim that040CLI was rebuilt for this component.

- [ ] Run the full ordinary native gate under original optimized integration/Folioopt0 library correctness profiles with pre/post source and test-input checks. Expected: zero failures/unfinished targets; actual counts/skips recorded, not assumed.
- [ ] Qualify unchanged frontend and build normal optimized GUI with complete resources; preserve old binaries. Expected: source/native/frontend/resource gates0, candidate fingerprints exact.
- [ ] Run one unchanged300-page105/100/100case at current natural display; independently audit all300savedpages/glyphs/RGB plus eight controls and original after105native512MiB/tree1GiB targets. Expected: all fidelity/ownership gates pass and original memory targets pass. If memory fails, stop acceptance and investigate; no unchanged rerun or raised limit.
- [ ] Compare selected timing summaries cautiously and retain source/byte ownership, small/quiz/form/font cold-reload/recovery coverage, strict owned root/profile absence. Expected: actual complete evidence before final review; broader performance/printing/low-memory claims stay open.

### Task 3: final review and historical component closure

**Files:** Frozen public review/report/dispositions, final source/helper/output/resource/index, checkpoint and continuation notes.

**Interfaces:** Actual successful Task2 evidence becomes a historically verifiable component authority; eventual combined041qualification is separate.

- [ ] Request one fresh scoped review after concrete evidence; exclude private raw quiz content/targets/screenshots, preserve aggregate bindings. Fix consequential findings inline with RED/GREEN/affected checks; ledger minor/declined/ruling costs.
- [ ] Seal the component only after real acceptance/review, record supported scope and remaining memory/producer/complex-text/printing/accessibility/signature limits, retain039/failed040/worktree/evidence, leave broad goal active. Do not promote a combined package here.
