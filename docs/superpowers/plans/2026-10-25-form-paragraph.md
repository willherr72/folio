# Reusable Object Paragraph Editing Implementation Plan

> Execute inline with superpowers:executing-plans. Use TDD and one final scoped
> correctness review; fix demonstrated findings with regression evidence.

**Goal:** expose the existing paragraph workflow for a verified reusable object
occurrence, preserving other copies and the complete document.
**Architecture:** private selective promotion, scoped native facade, bound frontend
adapter, existing paragraph layout/publication and saved memory.
**Stack:** Rust/PDFium/lopdf; React/TypeScript/Tauri; Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-25-form-paragraph-design.md.

## Global constraints

No source-PDF overwrite, commits, merges or release. Existing isolated worktree.
Keep 16 MiB content/copy, 100,000 operation/value, depth12, 5,000 form-run and
512 MiB source bounds. Permissions and publication ownership stay authoritative.
Private projected source IDs never escape. One Apply/Undo transaction. Physical
points and unchanged-value replay follow existing UserUnit rules.

## Review focus

Inherited root fonts and aliases: projection collision/inheritance fixtures.
Shared nested occurrences: only selected top-level Do promoted; native structure checks.
Page geometry: noncommuting CTM, rotation/crop/UserUnit fixtures and replay tests.
Scope confusion: reject indices outside promoted occurrence before inspect/preview.
Resource/ownership limits: direct-copy bounds, stale path, abandoned reply and cancel tests.

## Task 1: verified private projection

Files: form_text.rs, new form_paragraph_projection.rs and
form_paragraph_projection_tests.rs (children of form_text module).
Interfaces: Model::promote_paragraph(path:&[usize])->EngineResult<PromotedPage>;
PromotedPage owns bytes, selected root ordinal and original full anchor path.
WorkerRuntime::with_form_paragraph_projection<T>(source:&str,page:usize,
path:&[usize],use_page:impl FnOnce(&mut Self,&str,&FormParagraphScope)->EngineResult<T>)
where FormParagraphScope owns anchor_index and allowed object_indices.

- [x] Write failing unit fixtures for nested shared calls, root text before/after,
  inherited fonts/colors/spacing and colliding root font aliases. Assert complete
  glyph/raster equality, preserved unselected Do/resources and exact anchor mapping.
- [x] Record the expected failing run (`cargo test --lib form_paragraph_projection`).
- [x] Record root flattened ranges without altering ordinary form editing behavior;
  promote only the chosen Do and install collision-free local font aliases with
  bounded copies. Preserve root content and unused resource categories.
- [x] Reopen/prove geometry, glyphs, exact raster and scoped mapping; register a
  callback-owned source preserving page policy, original path and legacy flag.
  Verify cleanup on callback success/error. No client source registration.
- [x] Add rotation/crop/UserUnit/noncommuting transform, stale/clipped/malformed and
  resource-limit regressions. Run focused tests, existing form/state/flow/affine tests.

## Task 2: scoped occurrence paragraph facade

Files: new form_paragraph.rs; engine.rs, types.rs, physical_units.rs, lib.rs/main.rs
command registration; tests/form_paragraph.rs and occurrence ownership tests.
Public methods: list_form_paragraph_runs(source,page,path,expected)->FormParagraphRuns
{anchor_index,runs:Vec<EditableTextRun>}; inspect_form_styled_paragraph(source,page,
path,indices); suggest_form_styled_paragraph(source,page,path,index,expected);
fit_form_styled_paragraph(source,page,path,request); preview_form_styled_paragraph_occurrence
(source,occurrences,selected,path,request)->PageContentReplacement.
Inspect/suggest return StyledParagraphSelection; fit returns AnnotationRect.

- [x] Add failing integration tests: multi-line/mixed reflow, shared invocation and
  duplicate page isolation; stale/out-of-scope indices must fail without source growth.
- [x] Implement worker request dispatch over private source-authorized projection;
  reject scope violations on every call. Filter suggested membership to scope and
  re-inspect it; anchor remains original path/text bound.
- [x] Preserve physical-point API conversion and exact unchanged source values.
  Reuse existing styled paragraph layout/fit/preview; complete-source publication
  through publish_text_occurrence with abandoned-reply cleanup.
- [x] Add annotated/widget source, unchanged replay, save/reopen and font-substitution
  regressions. Run related paragraph/form/occurrence suites.

## Task 3: paragraph dialog integration

Files: editor types/adapter/tauri-adapter; paragraph-transaction.ts (bound adapter
factory); App.tsx; ExistingTextDialog.tsx; focused frontend tests.

- [x] Add failing tests for form entry, mapped anchor, retained original occurrence,
  source release on failure/cancel, stale prepare response and complete-source Apply.
- [x] Create a bound per-dialog adapter for the five native methods, preserving
  original source/page identity. Preparation loads the mapped anchor under the
  existing busy guard. Existing paragraph dialog supplies editing/fitting/preview.
- [x] Expose Edit paragraph only with the complete adapter contract. Keep ordinary
  paragraph behavior and legacy adapter compatibility. No workspace mutation on open.
- [x] Verify manual-selection scope, one-step Undo/Redo and saved top-level re-edit.
  Run focused then full frontend tests and typecheck/build.

## Task 4: review and desktop checkpoint

- [x] Freeze changed-source hashes; independent scoped correctness review.
- [x] Correct demonstrated findings once with red/green regression evidence.
- [x] Fresh source-fingerprinted debug build; synthetic Windows nested/shared
  paragraph: open, type, Fit, Preview, Apply, Undo/Redo, save/reopen and re-edit.
- [x] MuPDF/pypdf checks: edited text/style, untouched copy/page pixels, resource
  bindings, original hashes and exact unchanged replay. Stop only owned test app.
- [x] Record exact evidence/gaps and update quality ledger. Feature remains incomplete
  until all four tasks pass; no Adobe/Foxit parity or release claim.
