# Nested Text Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline, task-by-task. The user authorized autonomous execution without routine approval questions. One final independent scoped implementation review follows completed runtime acceptance.

**Goal:** Make existing nested PDF text and paragraphs discoverable and editable through CLI/MCP using Folio's native fidelity and occurrence-isolation proofs.

**Architecture:** Add dedicated nested discovery and source-bound selectors. Dispatch the existing preview lifecycle to existing native reusable-object text/paragraph APIs. Preserve old page-text identities and bundle semantics; describe native outer paragraph scope explicitly.

**Tech Stack:** Rust, existing PDFium/lopdf engine, serde/SHA256, current CLI/stdio MCP, existing Windows native/CDP acceptance harnesses and independent Python PDF readers.

**Spec:** `docs/superpowers/specs/2026-10-27-nested-text-automation-design.md`

## Global constraints

- Wait for terminal native session13326 in `artifacts/native-regression-20261027-rerun-2`; preserve its result/fingerprints and triage failures before product changes. Do not infer terminal state from a slow log or start duplicate Cargo.
- Existing linked worktree; preserve accumulated dirty/untracked work. No commits, push, release, source-PDF overwrite, accepted-package replacement or unrelated process shutdown.
- One Cargo, jobs1, locked/offline, serial tests; shared target directory `C:/Users/WilliamHerr/Desktop/Code/Folio/src-tauri/target`.
- Native behavior remains authoritative; no weaker glyph/geometry/ownership/permission/collision proof and no font substitution.
- Pages1..10000, offset0..100000,100 records,16 KiB complete run,2 MiB final nested-discovery frame including trailing LF,256 KiB final paragraph frame.
- Edit JSON32 KiB; text1..4096 UTF-8 bytes; paragraph1..1024 runs/4096 aggregate UTF-8 bytes/style IDs0..63/LF/line spacing0.6..3.
- Existing preview8-entry/512 MiB/15-minute limits and120-second worker deadline remain. New manifest4/5 retain64 KiB caps; versions1..3 retain original meaning.
- Paragraph grouping scope is the outermost page Form invocation. Only inspected membership may change; disclose derived outer scope and protect other text, outer occurrences and pages.

## Review focus

1. Repeated inner Forms within one outer Form: close lines may join inspected membership; distant lines and all other outer uses must stay unchanged (Tasks1–3).
2. A nested paragraph becomes page text after promotion: re-inspection and subsequent editing must follow actual source representation; prior selectors must fail (Tasks2–3).
3. Equal native paragraph metadata at different paths: full seed provenance must distinguish paragraph identities; mixed selector/version pairs must fail (Task2).
4. Generated separators, subset fonts and physical scaling: no trimmed identity or unproved text substitution; native success/refusal and saved output must agree (Tasks1–3).
5. Escaped/non-ASCII text and old bundle consumers: count actual wire bytes, keep versions1..3 valid, reject forged version4/5 metadata and release private resources on cancellation (Tasks1–3).

## Task 1: nested discovery and single-run edit through CLI/MCP

**Files:**
- Create `src-tauri/src/automation/nested_inspection.rs`.
- Modify `src-tauri/src/automation/{inspection,text_preview}.rs`, `automation.rs`, `cli.rs`, `mcp/{protocol,worker}.rs`; touch `mcp/mod.rs` only if its exhaustive routing requires it.
- Create `src-tauri/tests/support/nested_automation.rs` and `src-tauri/tests/nested_text_automation.rs`.
- Extend `src-tauri/tests/mcp.rs`; retain existing text-inspection/text-automation/CLI tests.

**Interfaces:**
- `nested_inspection::inspect(&PdfEngine,&DocumentInfo,u32,usize)->EngineResult<Value>`; `resolve(&PdfEngine,&DocumentInfo,u32,&str)->EngineResult<(FormTextRun,String)>`; `valid_selection(&str)->bool`; `check_response(&Value)->EngineResult<()>`.
- `inspection::ResolvedText::{Page(EditableTextRun),Nested(FormTextRun)}` and `resolve_any_seed(...)` / `resolve_any(...)` with the existing resolve argument types, returning `(ResolvedText,String)`. The latter requires native single-run support; the seed route does not. Existing page-only hash and resolve functions retain their behavior.
- `automation::Request::InspectNestedText {input:PathBuf,page:u32,offset:usize}`; matching CLI command, MCP Call and Worker Kind. `nested_inspection` is crate-visible for transport frame checks.
- Shared fixture helper: `write_fixture(path:&Path, mode:Mode, user_unit:f32)`, where `Mode::{RepeatedOuter,RepeatedInnerClose,RepeatedInnerFar}` generates independent page text plus nested two-line source text; no private inputs.

- [x] Preserve terminal full-regression evidence and verify the corrected checkpoint source. If the full gate reveals a product failure, resolve it as an explicit preceding correction, not hidden in this feature.
- [x] Add CLI integration regressions `nested_discovery_is_stable_and_page_discovery_unchanged`, `nested_text_save_reedit_isolates_reused_occurrences`, and `nested_selection_stale_cross_page_and_permission_failures`. Reuse the three-page geometry in `artifacts/nested-text-automation-20261027/probe.py`; assert two outer uses on page1, one on pages2/3, distinct paths/IDs, exact original bytes and exclusive output publication. Capture existing `inspect-text` JSON before the feature to pin compatibility.
- [x] Add bounded-serialization unit cases for16 KiB run including appended ID,2 MiB complete CLI/MCP frame including escaped duplicate content/request IDs/LF, and normal non-ASCII text. Add strict version1/4 and schema tests: page IDs cannot masquerade as nested, raw `objectPath` input is rejected, old bundles remain valid.
- [x] Pin content identity explicitly: a byte-identical authorized copy returns the same ID; altered source bytes or page produce a different ID. MCP document authorization/copy/edit checks still apply independently. Do not add worker/session UUIDs to manufacture cross-copy differences.
- [x] Run RED with `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --test nested_text_automation -- --test-threads=1`; preserve expected missing-command failures before adding product behavior.
- [x] Implement native nested listing, bounded source-hash/domain-separated `nested-text-v1-` IDs and exact-match resolution. Use `list_occurrence_form_text_runs`; enforce copy/edit rights before listing/resolving. Keep unsupported records and native reasons intact; reject oversized records without truncation.
- [x] Add dedicated command/tool wiring and final-frame checking. Preserve default `inspect-text` pagination/order/IDs. New discovery must never create a preview or document mutation.
- [x] Dispatch `preview-text` by resolved kind: page uses existing API; nested uses `replace_form_text_occurrence` with complete page order and the exact resolved path/text, no font override. Keep page `source_text` semantics unchanged. Publish nested previews as manifest4; require exact version/selector pairing in readers, with existing save/apply byte validation.
- [x] Add MCP `nested_text_apply_reinspect_save_keeps_prior_document` using existing Session helpers: inspect → preview → apply → re-inspect → preview → save; verify exact candidate hash, prior document unchanged, stale selector refusal, other occurrences/pages, discard/close cleanup and source bytes.
- [x] Run GREEN: `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --lib --test nested_text_automation --test text_inspection --test text_automation --test cli --test mcp -- --test-threads=1`. Inspect terminal results and source fingerprints; preserve failures and fix their actual causes.

## Task 2: nested paragraph scope, reflow and promotion-aware re-editing

**Files:**
- Modify `src-tauri/src/automation/{paragraph_inspection,paragraph_preview,text_preview}.rs` and `src-tauri/src/mcp/protocol.rs`.
- Create `src-tauri/tests/nested_paragraph_automation.rs`; extend shared nested fixture and `src-tauri/tests/mcp.rs`.
- Native `form_paragraph.rs` and `form_paragraph_projection.rs` should remain unchanged unless a reproduced native defect requires a separately documented correction.

**Interfaces:**
- `ParagraphContext::{Page,Nested{path:Vec<usize>}}` and `ResolvedParagraph {selection:StyledParagraphSelection, source_fingerprint:String, context:ParagraphContext}` in `paragraph_inspection`; its `resolve` returns this structure.
- `OccurrenceScope` serializes exactly `{kind:"outer-form",objectPath:[outerIndex]}`. Define once in `paragraph_preview` (or a small shared module), use for nested inspection and optional `ParagraphMetadata.occurrence_scope`. Reject null, wrong kind and any path length other than one. Caller edit schemas do not accept this field.
- Nested paragraph identity is `paragraph-v2-` with distinct domain and source fingerprint/page/full nested seed/native paragraph metadata. Existing page paragraph-v1 identity stays byte-identical.
- Nested suggestion derives promoted anchor via `list_form_paragraph_runs`, then `suggest_form_styled_paragraph`; fit/preview use existing form paragraph APIs with the regenerated path/context. Version5 requires nested seed, paragraph-v2 and scope; version2 requires page seed, paragraph-v1 and no scope.

- [x] Add `nested_paragraph_inspect_fit_save_and_promoted_reedit`, `inner_form_membership_and_outer_scope_are_explicit`, `equal_metadata_different_seed_paths_have_distinct_identity`, and `nested_paragraph_stale_palette_scope_and_version_pairs_are_rejected`. Assert full inspected text/style membership, derived outer scope, unchanged unrelated same-page text/other outer uses/pages, and raw-path/scope override refusal.
- [x] The close-inner fixture must show four adjacent lines in inspected membership. The distant-inner fixture must show only the chosen paragraph; saved edits must preserve the distant inner text/pixels. Test the same outer Form reused on another page. Native geometry determines grouping; do not force a fixture to pass by suppressing a source member.
- [x] Run RED: `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --test nested_paragraph_automation -- --test-threads=1`; missing nested paragraph support must fail before implementation.
- [x] Implement typed paragraph resolution and exact kind pairing. Use the promoted anchor's actual text/index; do not assume nested path indices are projected paragraph indices. Derive scope anew from path[0], expose it in inspection, and include full seed provenance in v2 identity.
- [x] Dispatch native fit and paragraph preview using regenerated membership/styles. Preserve original palette, optional controls and fit behavior. Publish nested metadata/version5, propagate scope through CLI/MCP responses and validate it without treating it as edit authority.
- [x] Extend MCP chaining to nested paragraph Apply, then inspect both page/nested routes on the new document, locate the actual promoted paragraph, re-edit and save. Verify prior source remains unchanged and old selectors fail. Test both modern and one legacy MCP protocol frame; keep every advertised schema strict and consistent with parsing.
- [x] Run GREEN: `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --lib --test nested_text_automation --test nested_paragraph_automation --test paragraph_inspection --test paragraph_automation --test form_paragraph --test mcp --test text_automation -- --test-threads=1`.

## Task 3: compatibility corpus, fresh binary acceptance and final review

**Files:**
- Extend the two new native integration suites and `src-tauri/tests/mcp.rs` with meaningful uncovered cases.
- Add artifacts/harnesses only under `artifacts/nested-text-automation-20261027/`.
- Update `docs/automation.md`, `docs/product-quality-loop.md`, this plan and `.superpowers/sdd/2026-10-27-nested-text-automation/progress.md`.

**Interfaces:** existing CLI commands, MCP stdio JSON-RPC and native APIs from Tasks1/2; no new product interface in this task.

- [x] Reuse/adapt existing embedded-form, generated-separator, UserUnit1.7, rotated/affine, protected-owner and restrictive-permission fixtures. For each selected fixture, record native baseline success/refusal and require automation equivalence. Include clipped/overlapping/malformed/deep-path negatives and preserve source/output guards. Do not broadly add tests that merely mirror implementation.
- [x] Exercise old versions1..3 and new4/5 through valid save, tampered bundle, unknown keys, wrong prefix/scope/version, cancellation/expiry/quotas and cleanup. Check actual private directories and saved candidate hashes, not only JSON success.
- [x] Run the final affected native/transport gate with exact source/test fingerprints; include existing page-text, paragraph, form-automation and MCP suites to detect old-contract regressions. Run appropriate TypeScript/build checks only if frontend/build inputs changed; otherwise re-use unchanged frontend evidence and record that scope honestly.
- [x] Build fresh debug custom-protocol desktop/CLI binaries into a new local candidate folder using `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --bin folio --bin folio-cli --features tauri/custom-protocol`. Preserve accepted portable0.22 and prior candidates.
- [x] Run fresh CLI and MCP scripted workflows across repeated-outer, close/distant-inner, transformed and protected cases. Independently compare saved extraction and rendering in MuPDF/pypdf, nonblank changed-area witnesses, outside-selection/unselected-page pixels, notes/links/widgets/defaults and original source hashes. Demonstrate a deliberate wrong-selection or missing-image oracle is rejected. Verify exact candidate bytes after Save and continued editing after re-open/Apply.
- [x] Open representative saved outputs in an isolated owned desktop instance with native dialogs; verify rendering and a subsequent existing paragraph edit, Undo/Redo and saved reopen. Use the documented WebView2/CDP workflow; stop only recorded process identity. Preserve any failed runtime run and correct the harness only when evidence proves the failure belongs to it.
- [x] Update automation examples/descriptions with discovery distinctions, explicit outer paragraph scope, source-bound IDs, promotion re-inspection, manifests4/5 and all unchanged constraints. Create a frozen source/delta/evidence snapshot for one independent final scoped review. Require clear findings versus coverage limits; no private original access or source/test changes by reviewer.
- [x] Resolve demonstrated review findings with appropriate RED/GREEN and affected fresh runtime checks; preserve original review and record coordinator dispositions. Checkpoint exact source/binary/output/review hashes and remaining limitations. Leave overall parity goal active; do not equate this milestone or a scoped green suite with whole-product parity.

Final review found no demonstrated defect. Its response and checkpoint explicitly
retain unverified nested no-op Save/Apply identity and live version-4/5 interruption
coverage. The lifecycle checkbox above represents shared implementation/existing
tests plus live discard/close/shutdown, not proof of those missing runtime classes.

## Self-review and handoff

Spec identity/discovery/transport requirements map to Task1; paragraph grouping,
scope, fitting and promotion map to Task2; cross-format/runtime/lifecycle/fidelity
acceptance maps to Task3. All five review-focus classes have named owning tests.
No product/native test file has been changed while the full baseline gate runs.
The design review's sole boundary question is resolved in its response and in
Task2's close/distant-inner tests. Inline execution and autonomous decisions are
preserved; no new permission checkpoint is needed.
