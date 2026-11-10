# Form filling implementation plan

Spec: ../specs/2026-10-22-form-filling-design.md
Execute inline with superpowers:executing-plans and one final fresh review.
User autonomous authorization overrides approval/commit gates. Preserve the existing
isolated worktree and earlier changes; use local progress artifacts, no release.

- [x] Task 1: Native field inspection/editing. New form_fill.rs owns raw PDFium
  lifetimes and DTOs. PdfEngine list_form_fields(source,page) and fill_form_field
  (source,request) -> new full DocumentInfo. Engine worker retains existing bindings
  and registers candidates after imported annotation cleanup. Reuse AcroForm graph
  validation. Add tests/form_filling.rs fixtures covering the spec before code.
  Expected RED missing API; GREEN exact values and changed AP on all shared widgets,
  untouched source, readonly/invalid/truncating edits refused. Cargo integration
  suite plus existing AcroForm/export/recovery/text tests must pass.
- [x] Task 2: Frontend workflow. Add native adapter/types, FormFillDialog and
  commitFormReplacement helper. App toolbar/dialog own page/tab/source snapshot;
  apply one field atomically and reopen updated field values. Use existing modal
  keyboard/focus patterns. Tests first: whole-source updates preserve unrelated
  pages, bookmarks, overlays, selection, rotation, undo/redo; stale/unmounted result
  cleanup; accessible inputs/errors. Expected RED absent workflow; GREEN full
  frontend suite and production build.
- [x] Task 3: One fresh whole-milestone review. Reproduce material findings, fix in
  one RED/GREEN pass. Build Windows exe, fill/save/reopen/undo/recovery on generated
  fixtures. Independent MuPDF/pypdf field and appearance/source checks. Update parity
  record and ledger with concrete evidence and limitations, keeping goal active.

Review focus: bindings lifetime/initialization and reentrant callbacks; native
handles on errors; shared-field AP propagation; choice export versus display values;
imported overlay duplication and recovery; stale source/geometry acceptance.

Final evidence: 395 frontend tests and 263 native tests passed; three explicit
manual printer/large-corpus tests ignored. Production and Windows debug build pass.
One fresh review, all three material findings fixed with failing-then-passing
regressions, no deferred minors. Exact final Windows executable filled every
supported type, undid/redid, duplicated, recovered after forced exit, saved and
reopened. Independent MuPDF/pypdf values and pixels pass; original hash unchanged.
Details, rulings and remaining parity gaps: artifacts/form-filling-progress.md and
docs/product-quality-loop.md. Local worktree retained; overall goal remains active.
