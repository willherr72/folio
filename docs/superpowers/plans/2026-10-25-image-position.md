# Existing image positioning implementation

> Execute inline with executing-plans and test-driven-development. User already
> authorized autonomous work; no routine approval or integration requested.

**Goal:** Preview, apply, undo and save moves/resizes/rotations/removal of existing images.
**Architecture:** Selective raw invocation editing, existing private projection and
full-source publication, existing modal candidate ownership.
**Spec:** ../specs/2026-10-25-image-position-design.md

## Global constraints

Preserve existing dirty checkout, all PDFs and accepted executables; no commits,
pushes or release. Native source/model bounds and policy apply unchanged. Public
translation abs<=1e6 pt, positive scales 0.001..1000, angle abs<=360 degrees.
No direct drag handles; exact numeric editing and preview this milestone.

## Review focus

- Sheared/reflected nested shared images: untouched siblings and clip/paint order.
- Repeated saved edits: bounded operator/object count and exact no-op defaults.
- Intrinsic plus workspace rotation/UserUnit: physical movement and dimensions.
- Pending request then action/draft change: stale candidate disposal without asset import.
- Removal with borrowed annotation resources and duplicate pages: retain other content.

## Task 1: raw transforms/removal

Files: existing_image_model.rs, existing_image_model_tests.rs.
Interface: Model::change(path:&[usize], matrix:Option<[f64;6]>) -> EngineResult<Vec<u8>>;
Some is a desired raw PDF matrix, None removes; Model::rewrite retains replacement API.
- [x] Add model tests asserting one selected matrix changes, shared sibling unchanged,
  exact no-op, removal count, repeated saved object/op bounds and replacement interoperability.
- [x] Run cargo test --lib existing_image_model, observe missing-method failure.
- [x] Implement inverse/relative transform, exact q cm Do Q coalescing/removal, shared
  clone-chain write path; reject invalid target matrices and retain unused aliases.
- [x] Run focused model suite green, retaining logs.

## Task 2: public immutable transaction

Files: existing_image_edit.rs, lib.rs, app.rs, tests/existing_images.rs.
Interfaces: ImageTransform {delta_x,delta_y,scale_x,scale_y,rotation_degrees:f64};
ExistingImageEdit::{Transform{transform},Remove}; PdfEngine::preview_image_edit_occurrence
(source,occurrences,selected,path,selection_id,edit)->EngineResult<PageContentReplacement>.
- [x] Add integration tests for physical movement/rotation/scale, all intrinsic rotations,
  UserUnit1.7, nested clip, duplicate mapping, save/reopen, remove and stale/invalid requests.
- [x] Run focused integration red before facade implementation.
- [x] Convert translation once, reuse projection/publication/identity/appearance protection,
  verify output model, full glyph equality, union old/new raster boundary and candidate cleanup.
  Add Tauri command and registration.
- [x] Run library/image/image-permission/page-occurrence regressions serialized; inspect logs.

## Task 3: editor controls

Files: editor/types.ts, adapter.ts, new existing-image-transform.ts,
components/ImageReplacementDialog.tsx, existing-images.css, App.tsx and focused tests.
Interfaces: imageMeasurements(image,page)->{centerX,centerY,width,height,rotation};
imageTransform(original,draft,page)->ImageTransform; validate finite bounds/empty fields.
- [x] Add geometry tests for workspace rotations, locked aspect and exact reset; dialog
  tests for transform/remove without import, invalid inputs, action/pending invalidation,
  exact reset and ownership; app test one-step Undo/Redo. Observe red.
- [x] Implement helpers, adapter API and dialog action modes; preserve replacement behavior;
  change success/banner copy to image editing. Preview must render before Apply.
- [x] Run focused then full frontend and TypeScript checks, retain logs.

## Task 4: acceptance

- [x] Freeze scoped files/hashes; request one final correctness review with spec and evidence.
  Reproduce/correct actionable issues in one pass; keep coverage gaps distinct.
- [x] Build fresh debug checkpoint using existing build/fingerprint workflow.
- [x] Run owned Windows app normal/narrow: transform, compare, Apply/Undo/Redo,
  save/reopen, removal and replacement interoperability; no original overwrites.
- [x] Independently inspect/render saved PDFs; compare untouched siblings/pages/glyphs,
  physical geometry and repeated no-op; preserve failures and verify original hashes.
- [x] Stop owned app by recorded identity; record final fingerprint/results/limits,
  update capability ledger. Goal remains active, no parity claim.
