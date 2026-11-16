# Page organization automation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. One fresh final implementation/evidence review; no per-task delegation.

**Goal:** Expose faithful single-document page organization through native CLI and MCP, including continued editing and exclusive saved copies.

**Architecture:** A bounded typed page-selection module creates source-owned PagePlans and calls the existing native nonflattened export path. CLI owns new-file publication; MCP owns worker folders and immutable derived document IDs.

**Tech Stack:** Rust/Tauri/PDFium, existing CLI/MCP protocols, authored PDF tests, Python MuPDF/pypdf audits and Windows CDP smoke.

**Spec:** `docs/superpowers/specs/2026-11-13-page-organization-automation-design.md`

## Global Constraints

- Exactly one source document; optional explicit version1 page order with1..10000 occurrences and0/90/180/270 additional clockwise rotation.
-512KiB regular request files;512MiB immutable input/candidate limits; native result512KiB, CLI envelope1MiB, MCP envelope2MiB including newline.
- MCP input frame remains512KiB, stdout ceiling16MiB; do not change global ID policy or existing-tool limits.
- Preserve source crop/UserUnit/rotation/overlays and native forms/links/tags/metadata/security rules; never flatten or weaken native refusals.
- Preserve selected049, baseline5314279b84f12953d5f2b70e5ccff237db678441999860825994b1d95259e52c startup refactor, dirty worktree and all failed/private evidence.
- One Cargo job; external D: cache/temp; serialize native/build/UI. No commit, deletion, publication or normal-user-app launch.

## Review Focus

- Repeated pages must retain native internal-link and form occurrence ownership, with source-derived geometry at nonunit scaling and rotation.
- No-op copies must preserve exact protected bytes; changed plans must enforce assembly permission and retain encryption.
- A native-path response or oversized request/response must not fail only after public output or session state has been retained.
- Derived documents must match held-file bytes/proof; cancellation, capacity rejection, close and EOF must release only owned private files.
- Existing preview apply/save and legacy/modern MCP must remain compatible with the strengthened adoption boundary.

### Task1: Native page export and CLI

**Files:** Create `src-tauri/src/automation/page_organization.rs`, `src-tauri/tests/page_automation.rs`, `src-tauri/tests/support/page_organization.rs`. Modify `src-tauri/src/automation.rs`, `src-tauri/src/cli.rs`.

**Interfaces:** `PageSelection {page:u32,rotation:u16}`; `PageOrganization {version:u8,pages:Vec<PageSelection>}` (Serialize/Deserialize, deny unknown fields).
`PageOrganization::read(&Path)->EngineResult<Self>`, `validate(&self)->EngineResult<()>`.
`Request::ExportPages {input:PathBuf,request_file:Option<PathBuf>,output:PathBuf}`.
Module export consumes engine, source DocumentInfo, optional validated organization, original immutable snapshot and destination; returns bounded JSON `path`/`pathNative`,bytes,sha256,pageCount.

- [x] Capture baseline source/helper/native input evidence, existing public CLI executable and selected049 identity.
- [x] Write actual CLI integration tests first: identity exact bytes, ordered duplicate/rotated export, invalid/unknown/null/version/range/request-size refusals, exclusive existing/source-alias output, protected allowed/denied and native-path publication. Public fixture retains distinguishable full text/ink plus links/bookmarks/tags/forms. Expected: current CLI rejects new command, not a compile failure.
- [x] Run `cargo test --release --locked --offline --manifest-path src-tauri/Cargo.toml --test page_automation -- --test-threads=1` with one job and preserved RED log. Expected: missing-command failures, no source changes/output.
- [x] Implement native types/validation and exactly bounded immutable-snapshot capture before native work; identity copies exact bytes; changed plans source geometry/unique IDs/overlays and native `export_pdf(flatten:false)`. Add core boundary/late-target unit tests and verify RED before correcting the boundary where applicable.
- [x] Add CLI command/help/options and versioned request-file parsing. Reject unrelated/repeated options; retain password/exit behavior. Stage/hash/check result before persist_noclobber; add CLI wire check before writing success.
- [x] Run the Task1 integration command and `--lib page_organization` with `--config profile.release.package.folio.opt-level=0`. Expected: GREEN; no stale-source/renderer/input drift. Opt0 library is correctness only, never a speed claim.

### Task2: MCP organization, export and verified document ownership

**Files:** Modify `src-tauri/src/mcp/{protocol.rs,worker.rs,session.rs,config.rs}`, `src-tauri/tests/mcp.rs`; create `src-tauri/tests/support/page_organization_mcp.rs`.

**Interfaces:** `Call::OrganizePages {document:usize,organization:Option<PageOrganization>}` and `Call::ExportPages` with the same fields; new worker kinds. `folio_organize_pages` adopts a private `Edited.pdf` and returns documentId/name/bytes/sha256/pageCount. `folio_export_pages` retains only an owned `Pages.pdf`, returning path/bytes/sha256/pageCount. Source IDs and output paths never come from tools.

- [x] Write MCP integration tests in a child module reusing the real Session helper: both protocols; tools/schema/hints; organize→info/inspect→text edit→save; export exact organized bytes; original ID/content unchanged; close/new-ID monotonicity; unauthorized IDs/paths; optional/null/empty pages; repeated exports. Expected: missing-tool RED on current CLI.
- [x] Add Config adoption tests for wrong hash/same-size changed bytes, unchanged state on rejection, held-file mutation denial and close cleanup. Expected: current wrong-hash adoption is a behavioral RED. Existing success fixtures using fake hashes must be corrected to the actual fixture digest, preserving their capacity assertions.
- [x] Implement same-handle bounded hashing and read-shared Windows derived-snapshot ownership; retain existing preview application behavior and capacity64/512MiB.
- [x] Implement strict tools/schema/parser and worker routing to Task1 CLI. Private organization stages outside public output; public export uses owned child folder. Reconstruct fixed response fields, verify actual candidate proof and source-derived page count, remove owned request file before public retention.
- [x] Validate complete new-tool MCP responses against2MiB before retaining exports/adopting state; test exact limits with escaped/non-ASCII data and near-maximum frame IDs. Preserve source/candidate guards and native paths.
- [x] Add full-session rejection/recovery, worker error, cancellation/EOF cleanup tests with actual owned worker identities. Extend sleeping-worker test support only for new kinds. Keep single-worker/120s behavior unchanged.
- [x] Run full actual `--test mcp` and new ownership/wire library tests with appropriate profiles. Expected: GREEN, no output outside configured folder, unchanged earlier documents and no orphaned private candidates. Existing preview/apply/save cases remain green.

### Task3: Qualify the real component and review

**Files:** Diagnostic evidence under `artifacts/page-organization-automation-20261113/`; specification/plan ledger, `docs/automation.md`, quality status, optional README development note.

**Interfaces:** Final product/native/helper fingerprints, normal optimized GUI/default CLI with26 exact resources, real automation and independent output receipts. No portable promotion.

- [x] Run the complete current library correctness suite with Folio opt0/release dependencies, then ordinary optimized integration targets `page_automation`, `cli`, `mcp`, `text_automation`, `paragraph_automation`, `form_automation`, `automation_fonts`, `nested_text_automation`, `nested_paragraph_automation`, all serial and no-fail-fast. Expected: all pass, explicit ignores retained and target membership/source/input guards match. This is not a fresh all-integration-target or frontend run.
- [x] Build ordinary optimized default CLI and GUI/custom protocol; capture generated assets/resources/source, keeping selected049. Expected: both compile with exact resource map.
- [x] Run real CLI/MCP page-order and protected cases using these binaries, then organize→inspect→edit→save chaining; audit output physical geometry, complete text/glyph/raster, fields, links/bookmarks/nonempty tags and editable Folio overlay behavior with independent readers. Add actual negative controls for wrong order/rotation/destination/widget/tag or changed source. Expected: valid outputs accepted and corruptions rejected.
- [x] Open a qualified organized result in an isolated actual Windows GUI; inspect new order/rotation/form answer and edit text/save/reopen. Expected: source preserved, expected saved content and owned process/port cleanup.
- [x] Document exact CLI/MCP schema, examples, capacity/security/refusal rules and current development-candidate status. The selected049 package does not yet contain this feature.
- [x] Freeze scoped baseline/current sources and complete evidence for one final fresh review. Resolve Important findings in one test-first fix pass, no re-review; record all limits/dispositions.
- [x] Verify completion evidence and record component/plan/ledger/quality status; keep broad goal active and existing launcher unchanged. Batch next multi-input composition work before separate portable qualification.
