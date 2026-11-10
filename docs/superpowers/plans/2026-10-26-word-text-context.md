# Word text context implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task, inline.

**Goal:** Edit original tagged and untagged Word text, including styled paragraph growth, while retaining clipping, transparency and semantic ownership across save and re-edit.

**Architecture:** Derive bounded context from actual operators and original structure objects. Carry a private source-selection proof through the existing isolated candidate and complete-source publication paths. Preserve native glyph/font/pixel checks and generic replacement exclusions.

**Tech Stack:** Rust, lopdf, PDFium, existing TypeScript/Tauri desktop, Python independent-reader audits.

**Spec:** `docs/superpowers/specs/2026-10-26-word-text-context-design.md`, including all builder-check resolutions.

## Execution status, 2026-10-26

Tasks 1-5 are complete as recorded in the execution ledger and final review response.
The detailed checklists below retain the original planned acceptance scope; evidence
and any narrower coverage limits are authoritative in those records. Final native
574/7ignored, explicit original Word2passed, independent saved-file audits, corrected
frontend/native build, actual Windows re-edit and Fit workflows all passed. No release
or parity claim. Product fingerprint7dbae672598b5b82d3486b5962ee97882fb39d1b09da2d62e94939f682381191.

## Global constraints

- Content: 16 MiB, 100,000 operations, 64 graphics/marked scopes; finite operands and transforms.
- Structure: 65,536 nodes, 100,000 number-tree entries, MCID 0..65,535, depth 64.
- Preserve original inputs, worktree changes, executables and apps. No commits, cleanup, releases or new dependencies.
- Original source is authority; metadata, UI geometry and projected catalogs cannot authorize tag ownership.
- Missing subset glyphs still require explicit supported substitution. No PDF/UA or Adobe/Foxit parity claim.
- One Cargo/build at a time, existing shared target directory, one build job.
- Use red/green regression cycles and persistent execution ledger; no routine approval pauses under standing authorization.

## Review focus

1. Paths and pending W survive Q; q/Q may restore CTM and clip without restoring the current path (Task 1 regression).
2. Hidden semantic changes where pixels stay identical: changed MCID/Lang/clip must invalidate hints (Task 4 regression).
3. ParentTree aliases and unrelated stream-bound MCRs must not evade ownership checks (Task 2 regression).
4. Generated whitespace and text object gaps must not shift selected/untouched context mapping (Task 3 regression).
5. Repeated edit wrappers must not amplify operations or consume surrounding content (Task 4 regression).

### Task 1: Bounded operator context

**Files:** Create `src-tauri/src/text_context.rs` and its focused test module; register under `text_edit.rs`.

**Interfaces:** `text_context::inspect(&[Operation]) -> EngineResult<PageContext>` produces show operation indices, per-show `ShowContext`, exact ordered marked events and shared bounded clip recipes. `ShowContext::contains([f64;4]) -> bool` uses raw page coordinates. `ShowContext::binding() -> EngineResult<String>` binds effective clip and full marked stack. `ShowContext::clip_prefix() -> EngineResult<Vec<Operation>>` emits an outer q and original rectangle/CTM recipes; caller emits closing Q.

- [ ] Add failing tests: nested intersection [10,20,90,80]; q/re/W/Q/n retains path and applies clip after restore; translated/reflected/quarter-turn rectangle; unknown curve and disjoint empty clips refuse containment; text clip applied at ET; malformed/over-limit state refuses; distinct same-MCID scope openings differ; cursor-only TJ does not create a show; emitted recipe reproduces exact effective clip.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib text_context` with configured shared target. Expected: meaningful failures before implementation.
- [ ] Implement bounded persistent recipe/state tracking; no product eligibility relaxation yet. Graphics state owns CTM/clip/Tr; path, pending W and marked stack are independent. Inline BDC admission requires MCID plus optional Lang; unsupported active context remains unprovable.
- [ ] Repeat focused command. Expected: all context regressions pass. Run whole library via task-done. Expected: all pass, existing ignored tests reported.

### Task 2: Source tag and Group proof

**Files:** Create focused `text_context_tags.rs`, `text_context_group.rs`; integrate at `text_edit.rs` page-copy boundary and `page_content_replace.rs` exact publication only.

**Interfaces:** `group::descriptor(&Document,ObjectId) -> EngineResult<Option<Group>>` compares resolved device-space/I/K semantics. `tags::prove(&Document,ObjectId,&[ShowContext]) -> EngineResult<()>` proves original reciprocal ownership. A private `TextContextProof` is constructible only from original selected native runs and parsed context; exact publication consumes it. Generic/legacy callers use existing restrictions.

- [ ] Add failing bounded synthetic tests for valid Word topology, inherited Pg, ParentTree Kids, missing/duplicate/reassigned owner, P cycle, selected ancestor Alt/A/C, unrelated Alt preserved, stream MCR refused, Group optional false equivalence and lost/changed Group refused.
- [ ] Run focused module tests. Expected: failures tied to absent proof.
- [ ] Implement resolved design contracts. Every source/baseline/candidate/final copy checks Group; duplicate tagged occurrence rejects before creation. Keep original catalog/page/tag objects, and strict exclusions for callers without selected top-level proof.
- [ ] Run focused tests and library task gate. Expected: pass; generic replacement guard regressions remain passing.

### Task 3: Single-run clipped/tagged edits

**Files:** Modify `text_edit.rs`, `occurrence_text_edit.rs`, `page_content_replace.rs`; add `src-tauri/tests/word_text_context.rs` using public synthetic fixture helpers.

**Interfaces:** private publication placement enum distinguishes Single, StyledAnchor, UniformAppend. Single maps source/candidate native ordinal identically and requires unchanged marked events, untouched contexts and selected clip containment. Context proof is source-derived, never an API request field.

- [ ] Add failing single insertion/save/reopen tests with Group, clip and tags, whitespace neighbors, second page and cropped/rotated/UserUnit variants. Negative: partially clipped replacement/source, context mismatch, stale text, duplicate tagged occurrence, permissions, cleanup.
- [ ] Run new integration target. Expected: old clipping/document-linked refusal before relaxation.
- [ ] Force stream-preserving replacement for clip/marked content; keep group merge refusal and unsupported substitute routes explicit. Verify selected glyph bounds before/after. Publish only with complete original proof and preserve tag/group graph.
- [ ] Run integration target plus occurrence and embedded text tests. Expected: insertion/re-edit succeeds, original and neighbor evidence unchanged; all negative cases refuse without leaked candidates.

### Task 4: Styled reflow, bounded replay and hints

**Files:** Modify `text_paragraph.rs`, `styled_paragraph_splice.rs`, `styled_paragraph_publish.rs`, `paragraph_highlight_publish.rs`, `styled_paragraph_memory.rs`, `paragraph_memory.rs`, `occurrence_paragraph.rs`; expand integration fixtures.

**Interfaces:** Prepared carries selected ShowContext; all selected source contexts agree. Splicer receives proved context and emits one clip envelope with block_start at canonical 13-op interior. Prefix replay omits marked events. New optional versioned hint binding is checked only against independently reproved context. Remembered envelope removal requires actual owned fragment/background blocks and no unrelated interior.

- [ ] Add failing styled growth, save/reopen, several real re-edits and bounded repeated envelope tests; unchanged neighboring native objects and pixels; highlighted backgrounds inside wrapper; altered MCID/Lang/clip hints invalidate with unchanged pixels. Uniform clipped path gets same containment/binding; uniform marked path remains explicit refusal.
- [ ] Run focused paragraph/splice/memory plus new integration tests. Expected: feature failures before implementation.
- [ ] Implement source-scope insertion, context mapping and exact owned-envelope removal. Preserve whitespace neighbors and context event sequence. Legacy hint versions remain limited to unmarked/unclipped context.
- [ ] Run native paragraph targets and library gate. Expected: all pass, no replay operation growth from obsolete envelopes.

### Task 5: Original Word acceptance and final review

**Files:** Evidence scripts/results under `artifacts/word-text-context-20261026`; update quality/continuation documents only with verified outcomes.

- [x] Run original `word-False.pdf` and `word-True.pdf`: insert this into first sentence, styled growth, save/reopen/re-edit. Expected: both originals accepted without preprocessing; missing CID h still refused; source hashes unchanged.
- [x] Audit saved text/pixels, second page, font bytes, Group and original structure relationships using MuPDF/pypdf. Expected: changed selected text only, intact unrelated graph and pixels.
- [x] Run affected native suites and fresh frontend/native debug build with source fingerprints. Expected: terminal passing results; no concurrent source drift.
- [x] Request one independent implementation review using frozen source/baseline/hashes, spec, plan, ledger rulings and this Review Focus. Resolve demonstrated important findings with red/green regressions; keep coverage limitations explicit.
- [x] Launch a fresh owned Windows profile using repository workflow; exercise original Word insert/reflow/save/reopen/re-edit. Expected: native UI flow passes; prior apps remain untouched.
- [x] Record exact counts, output/exe hashes and continuation checkpoint. Mark only this milestone complete if all required gates pass; overall product goal stays active.

## Plan self-review

All design sections map to Tasks 1–5. Context and proof interfaces are private and source-derived. Tag, Group, clip, replay and hint constraints are tested at their owning layer; end-to-end original Word acceptance is the milestone completion gate. Commit/cleanup steps are omitted under the existing user/worktree preservation policy.
