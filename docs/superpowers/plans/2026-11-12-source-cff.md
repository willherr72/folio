# Existing CFF source-font editing implementation plan

> For agentic workers: REQUIRED SUB-SKILL superpowers:executing-plans inline. One final scoped review after actual component acceptance; no per-task agents.

**Goal:** Make verified existing CFF-font text editable through Folio's existing native desktop/CLI/MCP transactions.
**Architecture:** Retain original font resources; extend the shared program/code/glyph/width proof with bounded CFF1 parser primitives. Preserve the TrueType path and all publication/ownership checks.
**Tech Stack:** Rust/Tauri/PDFium, vendored ttf-parser0.25.1, TypeScript, Python/fontTools/public OFL fixtures, native Windows acceptance.
**Spec:** docs/superpowers/specs/2026-11-12-source-cff-design.md

## Global Constraints

Keep selected045 immutable. Work in the existing managed dirty worktree.
Retain 16 MiB/program,128 MiB total-program,64resources,2 MiB ToUnicode,4096-byte
edit limits. Start with2,000,000charged CFF work units/program verification.
Keep the current unshaped character policy, width/pixel tolerances and runtime
renderer. No unrelated dependency upgrade; preserve parser licenses/provenance.
One native/build/UI process at a time; Cargo one job, target/temp on D:.
Profiles and reused/fresh results must be named accurately. Preserve failed
attempts. No commit/push/merge/publication/worktree-ledger deletion or normal
app automatic launch. Standing autonomy resolves routine design choices.

## Review Focus

- FontFile3 format/descendant confusion or CID/GID identity assumptions must never authorize the wrong glyph.
- Hostile charstrings/FDSelect/charset data must not create unbounded expansion or nondeterministic proof.
- Source aliases, generated spaces and reverse mappings must preserve exact source meaning on save and re-edit.
- A CFF addition must not change established TrueType/standard-font behavior or shared-object ownership.
- Audits must derive expected mappings, resource identities and allowed pixel regions independently of candidate output.

### Task 1: bounded CFF metadata and metrics foundation

**Files:** create `src-tauri/vendor/ttf-parser-0.25.1/` with upstream licenses and local patch notes; modify `src-tauri/Cargo.toml` path patch and lockfile; create `src-tauri/tests/source_cff_metrics.rs`; add licensed fixtures under `tests/fixtures/source-cff/`; component generators/provenance under `artifacts/source-cff-compatibility-20261112`.
**Interfaces:** consumes public pinned fonts and independent metrics; produces `ttf_parser::cff::Table::glyph_infos(&mut usize)`, `glyph_width_with_budget(GlyphId,&mut usize)->Option<f32>` and `glyph_index_exact(u8)->Option<GlyphId>`. Metadata iteration returns explicit errors, GID, optional name/CID and applicable matrix.

- [ ] Author licensed compact fixtures and expected metrics including reordered CID/GID, multiple FDs, fractional width, whitespace and malformed/budget counterparts. Expected: two independent readers agree; inputs hashed and licenses retained.
- [ ] Add native regression tests for the proposed APIs and run before production changes. Expected: missing APIs or demonstrably wrong CID/fractional/index behavior; record exact RED.
- [ ] Vendor unchanged pinned parser and add bounded zero-allocation metadata traversal, validated/binary FDSelect lookup and budgeted fractional SID/CID metric reading. Keep existing API behavior stable. Expected: correct values; no quadratic lookup or recursion-only work bound.
- [ ] Run new metrics tests plus available parser/table regressions with offline locked dependencies. Expected: all pass, adversarial cases terminate/reject and public-source hashes/provenance match.

### Task 2: source program/encoding verification

**Files:** modify `src-tauri/src/embedded_font.rs`; create focused `src-tauri/src/embedded_cff.rs` (or engine submodule path following existing module declarations), tests and `src-tauri/tests/source_cff.rs`; update engine module declarations as needed.
**Interfaces:** consumes Task1 primitives; preserves `FontProofs`, `VerifiedFont`, `verify`, `program`, exact encode/decode and existing callers' return types. Adds a private CFF verification branch rather than weakening TrueType proof.

- [ ] Test stock045/native failure for raw/wrapped simple and CID fixtures and add full resource/glyph/width proof assertions. Expected: current program/existing-font rejection, not fixture extraction failure.
- [ ] Implement validated container/descriptor selection, immutable program matching, CFF CID/SID and simple encoding mapping, ToUnicode/source-operand policies, bounded complete/partial repertoire and independent advances. Expected: all supported source flavors prove correctly; wrong widths, malformed mappings/matrices, missing glyphs and format conflicts reject.
- [ ] Test new source text eligibility and preview/save for permutations, explicit Differences, nonidentity CIDs, fractional/nondefault units and generated separators; retain standard/TrueType behavior. Expected: exact source program reuse, correct new text/advance, safe typed refusals.

### Task 3: complete editing/re-edit integration

**Files:** affected existing-text/group/paragraph/reusable-form integrations only where failing tests demonstrate need; add focused native integrations under `src-tauri/tests/source_cff*.rs` and authored fixtures/generators.
**Interfaces:** Task2 proof feeds existing edit transactions, source resource cloning, paragraph inspection/reflow and automation selections. No alternate writer or silent source-font replacement.

- [ ] Add RED tests for single/group/uniform/styled/nested-form and mixed CFF/TrueType edits, save/reopen/re-edit and occurrence isolation. Expected: failures trace to missing CFF integration while existing controls remain meaningful.
- [ ] Make only required shared-path corrections. Expected: original programs and unaffected objects survive; explicit substitute behavior and missing-character errors retain drafts/inputs.
- [ ] Validate tagged/clipped/aliased inputs, generated whitespace, no-op/re-edit, Undo/Redo state and encoding-preserving source operations; compare independent readers and fixed outside-area rasters. Expected: all pass without tolerance or ownership exemptions.

### Task 4: normal-build acceptance and final component review

**Files:** component source/input manifests, fresh native/frontend/build/runtime drivers, independent audits, scoped review/dispositions and completion checkpoint; relevant repository status docs.
**Interfaces:** all implemented paths -> complete fresh evidence and a reviewed development candidate; accepted045 stays the normal launcher until later portable qualification.

- [ ] Capture current source/native/shared/dependency/resource inputs and run complete fresh native library-correctness and optimized integration/bin/doc gates. Expected: no failed/unfinished/filtered targets; explicit skips/profiles retained. Run appropriate complete frontend gate with unchanged default timeout/two workers.
- [ ] Build normal optimized GUI and CLI candidates without mutating045 and bind complete unchanged resources. Expected: source/input/build guards pass and binaries match receipts.
- [ ] Exercise actual desktop and CLI/MCP CFF selection, preview/apply/discard, paragraphs, reusable objects, save/reopen/re-edit and workspace history. Independently verify source font hashes, glyphs/widths/reading order, ownership/tags/clips and fixed outside-area pixels; run corruption controls and owned cleanup. Expected: all required flavors/routes pass; exclusions explicit.
- [ ] Freeze manifest-relative scope and request one final fresh review. Disposition each finding/decline; fix consequential findings once using RED/GREEN and appropriate regression gates. Expected: no unresolved consequential defects; no re-review.
- [ ] Seal component evidence and record source/candidate/limits/rulings. Expected: actual standalone integrity/relocation controls pass; no parity or final-portable claim. Continue the broader goal, then integrate the reviewed improvement through the normal portable workflow.
