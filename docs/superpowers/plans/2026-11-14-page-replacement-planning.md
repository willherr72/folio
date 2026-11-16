# Page-wide replacement planning implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline under standing
> autonomous authorization. One fresh final review after the component; keep
> the managed worktree and artifacts. Do not invent a completed user workflow.

**Goal:** Map document Find matches to exact original source edits without
silently losing uneditable matches or changing content during planning.
**Architecture:** Pure page-wide matching/grouping core, then native ownership
proof and read-only PdfEngine API. Document aggregation and desktop Apply are
subsequent consumers, not a substitute for the broader objective.
**Tech stack:** Existing Rust/PDFium/lopdf and shared Unicode17 matching tables.
**Spec:** docs/superpowers/specs/2026-11-14-page-replacement-planning-design.md

## Global constraints

Query1..1024UTF8bytes/non-whitespace; replacement0..4096bytes/noNUL;
250000reading entries,100000runs,1000matches/page,16KiB/run,13path entries,
16MiB source text.64changed runs/512KiB expected+replacement bytes,2MiB page JSON.
No truncation, mutation, original-file changes, new dependencies, version/package/
launcher/Git/clipboard/private-input changes or cleanup. Serialize native with
oneCargoJob and D:/Folio-build-cache/t3-8962a327/corresponding-temp.

## Review focus

- Repeated/overlapping glyphs or shared forms must never bind the wrong source.
- Rotated reading copies and supplementary/ligature/source-text differences must
  retain identity or report an unmappable match, never guess.
- Cross-object neighbors control whole-word decisions; cross-object phrases must
  appear in the report even when a single-object edit cannot handle them.
- Selecting an eligible subset must not silently authorize another blocked match
  or truncate a document-level Replace all.
- Planning must retain original bytes, native IDs, credentials and permissions;
  no copy-protected text may leak through the read-only plan result.

### Task 1: pure page-wide replacement model

**Files:** new src-tauri/src/replacement_plan.rs; engine.rs module declaration;
module-local tests; artifact controllers and ledger.
**Interfaces:** internal PlanningRun,OwnedCharacter,SourceRange,PlanMatch,
PlannedRunEdit,PagePlan; `plan_page(&[OwnedCharacter],&[PlanningRun],&str,&str,
PageSearchOptions,Option<&[usize]>) -> EngineResult<PagePlan>`.

- [x] Capture prior atomic completion/checkpoint/current source/shared inputs
  and engine preimage before adding the module.
- [x] Add model tests and a minimal failing entry point. Observe genuine runtime
  RED for grouped edits, page-wide boundaries, cross-object reports, Unicode,
  whitespace, exact mapping, selected subsets, blockers/no-op/deletion and limits.
- [x] Implement matching through SearchMatches over reading entries; validate
  source boundaries/text, construct grouped literal final strings and suppress
  executable edits atomically when selected matches are blocked.
- [x] Run model and existing search_matching tests with source/shared guards.
  Expected: model passes; shared policy remains unchanged. Preserve profile:
  release dependencies/Folioopt0 correctness, not optimized/performance proof.

### Task 2: native page ownership and read-only plan API

**Files:** replacement_plan.rs, new replacement_plan_source.rs,
replacement_plan_native_tests.rs; engine.rs request/dispatch; lib.rs public
DTO exports. Existing reader extraction may be factored only with regression
coverage for exact reading output.
**Interfaces:** `PdfEngine::plan_text_replacement_page(source_id,page_index,
query,replacement,options,selected) -> EngineResult<TextReplacementPagePlan>`;
worker owns read-only mapping and source fingerprint. Public DTO carries
matches/blockers/grouped candidate edits, not trusted caller-authored paths.
Task1's `PlannedRunEdit.source_text` is the logical operand string. Preserve the
original native run's separate `text` for a later TextBatchEdit.expected_text;
never forward the logical string as that native selection identity by assumption.

- [x] Write root/shared-form/rotated/generated-separator/protected-source tests
  against a minimal API entry point and observe runtime RED.
- [x] Implement native character ownership using actual object handles/paths,
  disambiguated shared invocation geometry and complete text sequence proof;
  preserve canonical reading/surrogate behavior. Uncertain entries stay unmapped.
- [x] Require copy+edit permission, bind one source hash, and return physical
  displayed bounds plus exact source snapshot identity without registering any
  source or publishing any PDF.
- [x] Run focused/native-search/reading regressions, complete library correctness
  and affected optimized integration with exact input guards. Expected: original
  bytes/source counts unchanged; all supported mappings exact, uncertain cases
  visibly blocked and permissions refused without disclosure.
- [x] One fresh final component review, regrade feedback and rule on every
  declined behavior; one Important TDD correction pass, no re-review. Preserve
  checkpoint and record that automatic document/UI/deletion/reflow consumers
  remain unfinished; broad goal stays active.
