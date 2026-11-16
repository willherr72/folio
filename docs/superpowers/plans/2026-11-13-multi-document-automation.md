# Multi-document automation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. One final fresh implementation/evidence review; no per-task delegation.

**Goal:** Editable multi-document CLI/MCP composition with per-source authentication, native preservation and immutable derived document ownership.

**Architecture:** A typed composition module opens bounded unique source snapshots, builds source-owned PagePlans and uses the native export engine. Launch-owned credential capabilities follow documents, previews and derived results; transports never recreate PDF graph or encryption policies.

**Tech Stack:** Existing Rust/Tauri/PDFium, CLI/MCP protocols, Sensitive buffers, public PDF fixtures, independent MuPDF/pypdf auditors and Windows CDP.

**Spec:** `docs/superpowers/specs/2026-11-13-multi-document-automation-design.md`

## Global Constraints

-1..64 native input slots/authorized IDs;512MiB shared immutable input budget;10000 total input pages and10000 output occurrences;512MiB candidate.
- Optional version1 plan at most512KiB; source/page1-based, rotation0/90/180/270; omission concatenates. Aliases reuse logical source identity; byte-identical independent files do not.
- Credentials:128KiB JSON array aligned with slots; null or at most1024 UTF-8 bytes, no NUL/CR/LF. CLI stdin/MCP launch-only file; no tool credentials/paths, no plaintext worker credential files.
- Native assembly/encryption/metadata/form naming rules remain unchanged. Identity copies captured source bytes. Different protected contexts refuse; no implicit decryption/security change/flattening.
-512KiB result,1MiB complete CLI envelope,2MiB complete new MCP responses including newline;512KiB protocol input and16MiB worker stdout unchanged.
- Existing single-worker120s deadline,64document/512MiB aggregate session storage, cancellation and close/EOF semantics remain.
- Retain selected049, source baseline eb4f418a5d581014c5f5dd85d9ae014ce0b0856955303993bbd843ccfa73f693, prior snapshots/failures/private inputs/startup refactor. No commit/merge/push/delete/release/normal-user-app launch.
- One Cargo job, serialize native/build/UI; external D:/Folio-build-cache/t3-8962a327 and its sibling temp root. Opt0 correctness profiles are explicitly labeled; final integrations/GUI/default CLI ordinary optimized.

## Review Focus

- File aliases and repeated IDs must preserve logical field sharing, while separate byte-identical files retain independent forms and credentials.
- A protected source later in the output must determine preserved encryption and derived credentials; incompatible contexts/permissions refuse before retention.
- Closing an original source before applying a preview must not lose its credential or reveal it in protocol/retained files.
- Blocked/failed stdin feeding must remain cancellable and reap the child before secret/folder cleanup, without stalling the supervisor.
- Complete native-path/near-frame responses must be checked before output adoption/public retention; existing tools must remain compatible.

### Task1: Native composition and CLI

**Files:** Create `src-tauri/src/automation/composition.rs`, `src-tauri/src/automation/credentials.rs`, `src-tauri/tests/composition_automation.rs`; reuse `tests/support/page_organization.rs` and protected fixture. Modify `src-tauri/src/automation.rs`, `src-tauri/src/automation/original_snapshot.rs`, `src-tauri/src/automation/page_organization.rs` (shared publication helper only), `src-tauri/src/cli.rs`.

**Interfaces:** `CompositionPage {source:u8,page:u32,rotation:u16}` and `Composition {version:u8,pages:Vec<CompositionPage>}` with strict serde validation; `ComposeRequest {inputs:Vec<PathBuf>,request_file:Option<PathBuf>,output:PathBuf}`. `composition::execute(engine:&PdfEngine,request:ComposeRequest,credentials:&Credentials)->EngineResult<Value>`. `Credentials::read(reader:impl Read,count:usize)->Result<Self,&'static str>`, `none(count)`, `get(slot)->Option<&str>` and shared per-slot secret capability access. `OriginalSnapshot` gains same-open-file bounded capture/proof access without changing existing callers. CLI `Action::Compose` handles its repeated inputs and stdin secret array separately from existing one-input Invocation.

- [x] Capture scoped baseline sources/helpers, source/native inputs, selected049 and previous completion identity. Verify existing linked worktree; preserve dirty work.
- [x] Write CLI integration tests first: implicit concatenation; interleaved duplicate/rotation; independent colliding forms/tags/navigation; repeated paths/hardlinks vs independent identical files; exact one-source identity; protected late-source encryption, denied rights/different contexts, wrong/missing/invalid credentials; malformed requests/quotas/exclusive destinations/native paths. Capture public saved witnesses under `FOLIO_COMPOSITION_AUTOMATION_OUTPUTS`. Expected: new command rejected (behavioral RED), no overwritten source/output.
- [x] Run `cargo test --release --locked --offline --manifest-path src-tauri/Cargo.toml --config profile.release.package.folio.opt-level=0 --test composition_automation -- --test-threads=1`, one job with captured source/input guards. Expected: test binary compiles; missing command assertions fail.
- [x] Implement bounded credentials parser/strict CLI options/help and `Action::Compose`; reject unrelated/repeated flags and malformed secrets generically before native work. Add focused parser/secret-shape/quota tests. Expected: valid inputs preserve existing one-password commands and stdout/stderr conventions.
- [x] Implement source file identity grouping from held actual handles, credential equality for alias slots, bounded immutable capture and aggregate page/byte accounting. Reuse one native source per alias group; close every opened source on errors. Build source-owned occurrence plans, identity-copy optimization, native nonflattened export, output credential selection and reopen/page-count proof. Reuse exclusive prevalidated publication/wire limits.
- [x] Run the Task1 integration command plus focused composition/credentials library tests. Expected: all pass, source/input guards0, original bytes/late output sentinels preserved. Profile is correctness only; normal optimized gate is Task4.

### Task2: MCP authenticated document and preview lifetimes

**Files:** Modify `src-tauri/src/mcp/{mod.rs,config.rs,previews.rs,worker.rs,session.rs}`, `src-tauri/src/automation/text_preview.rs` and page-organization response construction (nonsecret protection routing), CLI MCP help, `src-tauri/tests/mcp.rs`; create `src-tauri/tests/support/credential_mcp.rs`.

**Interfaces:** `Document` and `Preview` hold private optional shared Sensitive credential capabilities; neither serializes them. `Config::load` reads optional `--credentials FILE` once aligned with input slots, recognizes same-file aliases while snapshotting, rejects credential conflicts, and retains shared launch snapshots as needed. Preview adoption and derived document adoption accept the originating credential. Worker owns feeder-thread handle/result and secret payload until reaping.

- [x] Add real MCP tests for protected info/inspect/preview/apply/save/re-edit, original closure before Apply, malformed/wrong/absent launch credentials, secret absence from protocol/export metadata, same-file launch alias consistency and no-credential backward compatibility. Expected: current launch option unsupported, behavior RED.
- [x] Add owned sleeper/early-exit feeder tests: cancellation and EOF remain responsive while a child does not read stdin; reap/join before stage cleanup. Retain unrelated public output. Expected: missing feeder behavior is demonstrated before fixing.
- [x] Read launch credentials into shared secret buffers; pass a single document password only through child stdin. Add owned bounded feeder thread, detect feed failures before retention, kill/reap then join on cancellation/drop. Preserve deadline and busy behavior.
- [x] Carry source credential in preview records, including after source closure; Apply carries it to the new ID; Save copies candidate bytes. Derived plain documents carry none. Do not serialize capabilities or write plaintext secret files.
- [x] Run focused lifecycle/credential library cases and real `--test mcp` using correctness profile. Expected: existing Apply/Save and new authenticated cases pass with no orphan stages/children or secret output.

### Task3: MCP editable merge and composition export

**Files:** Modify `src-tauri/src/mcp/{protocol.rs,worker.rs,session.rs,config.rs}`; create `src-tauri/tests/support/composition_mcp.rs` and register in `src-tauri/tests/mcp.rs`.

**Interfaces:** new `Call::MergeDocuments`/`Call::ExportComposition` containing authorized document indices and optional Composition; worker kinds route to Task1 CLI. `protectionSource` is validated against authorized source slots and used privately to select Task2 credential. New MCP responses omit this internal field.

- [x] Write both-protocol real chains: merge IDs→info/forms/text inspect→preview/apply→export/reopen; source IDs unchanged, later protected source credential survives, repeated IDs share fields, distinct same-name inputs remain independent. Strict schema/hints/unknown-ID/path/null/empty/capacity/refusal/repeated-export cases. Expected: missing tools RED.
- [x] Add actual valid page-tool request padded near512KiB using escaped IDs, complete response cap/rejection-before-retention tests, source slot/proof/count validation, cancel/EOF/full-session cleanup and unchanged ID counter on rejection.
- [x] Implement strict schema/tools and source mapping; create bounded parent-owned request JSON and private/public stages; pass native input paths via OS arguments and credentials only feeder stdin. Validate native result before removing request and adopting/retaining only its candidate. Reconstruct bounded public summaries; reject unrecognized protectionSource/pageCount/proof/unexpected files.
- [x] Run full real MCP integration and focused ownership/protocol/lifecycle cases. Expected: both protocols/new chains and old tools pass; snapshots immutable, outputs confined, session limits unchanged.

### Task4: Qualify exact candidate, document and review

**Files:** `artifacts/multi-document-automation-20261113/`, current spec/plan/ledger, `docs/automation.md`, `docs/product-quality-loop.md`.

**Interfaces:** source/native/helper/candidate fingerprints, normal optimized default CLI/GUI with26resources, qualified saved-PDF witnesses, one final scoped review/disposition and component verifier. Selected049 unchanged.

- [x] Run complete current library correctness with Folio opt0/release deps and ordinary optimized integration targets `composition_automation`,`page_automation`,`cli`,`mcp`,`text_automation`,`paragraph_automation`,`form_automation`,`automation_fonts`,`nested_text_automation`,`nested_paragraph_automation`, serial/no-fail-fast. Expected: zero failures, explicit skips and source/input guards0. Not fresh all-project integration/frontend acceptance.
- [x] Independently inspect all qualifying authored outputs: complete text, physical geometry, whole RGB outside fixed edited-source corridor, forms/values/defaults/widget ownership, nonempty tags/language, Info/XMP/bookmarks/link rectangles and destination coordinates. Include unchanged reserialization positive control and actual wrong-source/order/rotation/value/tag/navigation/security/source-mutation corruptions. Expected: valid cases accepted, corruptions rejected.
- [x] Build ordinary optimized default CLI/custom-protocol GUI, bind Cargo opt3/default checks, generated assets/26resources; run exact-candidate protected CLI/MCP chains including real near-frame/native-path exchanges. Expected: candidate identity/source guards match and output survives session closure.
- [x] Isolated actual Windows GUI opens merged output, verifies independent form answers/order/rotation, edits text/fills a field, saves/reopens; compare independent witnesses and owned cleanup. Expected: unchanged originals, correct saved state, no owned process left.
- [x] Document commands/tools, credentials/limits/native security and field naming behavior, current development qualification status and remaining portable work. Freeze scoped baseline/current sources and complete evidence for one fresh final review; resolve Important/Critical in one TDD fix pass, record every declined judgment and deferred minor, no re-review.
- [x] Freshly verify receipts/source/resources/selected049, archive reviewed docs before status updates, record component completion and quality status. Preserve branch/worktree/evidence and broad active goal. Separate portable qualification only after meaningful batch completion.
