# Hollow callout editing implementation plan

> Use superpowers:executing-plans inline, TDD, and one final scoped review.

**Goal:** allow faithful existing text insertion within hollow rectangular borders.
**Architecture:** one conservative occupied-region helper shared by editing gates.
**Tech stack:** current Rust, PDFium, lopdf, frontend and Windows acceptance tools.
**Spec:** docs/superpowers/specs/2026-10-28-callout-editing-design.md

## Constraints and review focus

No dependency change, commit, publication or modification to original fixtures or
accepted packages. Serial locked/offline Cargo with one job; no input edits during
active gates. Keep full-bounds fallback for unproved paths and preserve raster and
source validation. Review numerical overflow/underflow, hairline/thick stroke
fringes, nonuniform transform semantics, compound/clipped paths and neighboring
content inside a proved box. Tests below cover each; arbitrary producer combinations
and actual printing remain explicit broader limits.

## Task 1 — native proof and regression

- [x] Add `src-tauri/tests/callout_editing.rs`: real minimal PDFs, public engine
  requests, two saves/reopens. Watch current text growth and styled fitting fail
  with recorded graphics collision errors; include unrelated-content assertions.
- [x] Create `src-tauri/src/text_graphics.rs`, included from text_edit.rs, exposing
  `occupied_regions(object: &PdfPageObject, geometry: PageGeometry) ->
  EngineResult<Vec<[f32;4]>>`. At most four regions, fallback one. Prove path shape
  and stroke margin as specified; finite arithmetic uses f64 and checked f32 output.
- [x] Use the helper in text_edit.rs, styled_paragraph_area.rs,
  styled_paragraph_publish.rs and text_group.rs. Preserve existing page-background
  and source-highlight exceptions. No edits to unrelated font/marked-content logic.
- [x] Focused tests for positive clear interiors and negative border/interior-art
  cases, uniform/styled/group routes, dashed/hairline/thick strokes, transforms and
  invalid geometry. Export saved fixtures for independent raster/text audit.

Final reviewer I1 correction: retain whole-object graphics refusal in the legacy
uniform API because it appends replacement text after neighbors. Current styled
desktop/CLI paragraphs retain the new admission. review-red-1 reproduced the
new refusal regression (6 pass, 1 expected failure); review-green-1 passed all 7.
full-native-1 was stopped by verified Cargo executable and raw UTC start ticks
after review established the required correction; terminal exit 1 and unchanged
source/test hashes are preserved. It is cancelled evidence, not an accepted run.
Use full-native-2 for corrected full-suite acceptance, followed by the existing
candidate/CLI/desktop plan. No second review round; record the single correction.

Integration dependency reproduced and corrected: opaque unselected marked-context
equality in text_context.rs/text_context_edit.rs, with source-preserving Word and
synthetic tests. Selected semantic edit admission is unchanged. Focused 7 pass;
text-engine unit gate 125 pass/1 ignored. See ledger for all failed runs and strict
audit exceptions. Full acceptance remains pending below.

## Task 2 — current native/producer/desktop acceptance

- [x] Run serial locked/offline focused gate, then full native suite and required
  ignored producer tests; bind source/test fingerprints and preserve failures.
- [x] Build current CLI/GUI and run the twelve original-producer callout workflows,
  including saves/re-edits and independent audit. Recheck all 36 representative
  workflows so other text/table behavior does not regress. Separate retained
  preexisting font-resource and tagged/table limitations from new defects.
- [x] Run frontend/build gates and owned Windows callout preview/apply/undo/save/
  reopen acceptance. Stop only owned exact executable/start-time identities.

## Task 3 — final review and milestone record

- [x] Freeze scoped source/baseline/test/evidence snapshots, dispatch one final
  reviewer. Resolve important findings with red/green coverage and required gates.
- [x] Update roadmap and ledger with exact accepted evidence, retained limitations
  and next priority. Keep the broad goal active; no commercial parity claim.

Completed: artifacts/callout-editing-20261028/checkpoint-final.json. Full native
1,622 passed/15 ignored; frontend 1,296 passed; current build passed. CLI 34/36
workflows, including all twelve callouts, with 68 saved outputs (65 strict audit
passes, three exact known unused-font resource differences). Four desktop outputs
pass all independent checks and nonempty tagged-graph preservation. Two Word table
paragraph fit refusals remain; legacy uniform bordered edits retain refusal.
Local debug candidate only; no optimized package replacement or parity claim.
