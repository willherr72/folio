# Reversible existing-image crop implementation plan

> Execute inline with executing-plans/TDD; standing autonomous authority. No commits.

**Goal:** Crop existing PDF pictures on-page, restore their hidden pixels after save,
and combine geometry/crop adjustments in one faithful transaction.
**Architecture:** Semantically recognized unit-space image clip wrapper; shared full-
source native transaction; crop-aware visible/full affine geometry and canvas controller.
**Stack:** Rust/lopdf/PDFium, React/TypeScript, Vitest, owned Windows/CDP acceptance.
**Spec:** ../specs/2026-10-25-image-crop-design.md

## Global constraints

Preserve dirty worktree, original inputs and previous artifacts/binaries. No release,
commit, merge or parity claim. Existing graph/CopyBudget/security/projection bounds
remain. Crop retained edges finite0..1, minimum width/height0.001; only serialization
roundoff may normalize. Full normalized crop is0,0,1,1 (left,top,right,bottom).
One builder check, one final review and verified correction pass. Goal remains active.

## Review focus

- Untrusted wrapper semantics/resource bindings and saved restoration (Task1/2).
- Shared nested aliases and combined matrix+resource rewrite ordering (Task1/2).
- Visible crop versus full image centers under reflection/shear/rotation (Task3/4).
- Pending native operations across action/precision changes and Cancel (Task4).
- Narrow-window actions, focus and live pointer mapping during status changes (Task4/5).

## Task1: bounded native crop model

Files: existing_image_model.rs; new existing_image_crop.rs as child module;
existing_image_model_tests.rs (or dedicated child test module).
Interfaces: ImageCrop {left,top,right,bottom}; validated/normalized constructor, FULL,
raw PDF rect conversion and affine crop corners. Occurrence gains crop:Option<ImageCrop>.
Recognize only exact six-op q/re/W/n/Do/Q unit form with one logical child and matching
full matrix, allowed dictionary keys/resources. No promotion of nested ambiguous crop.
Model::adjust(path,target:[f64;6],crop:ImageCrop)->bytes; a Change::Adjust variant carries
relative matrix and whether selected resource is a recognized crop. Leaf resource and
matrix rewrites happen together; draw operation after alias change must be retained.
Replacement preserves selected crop; repeated crop/restore rewrites, never adds layers.
- [x] Tests RED for crop raw/fitted/shared/nested images, explicit crop discovery,
  restore/bounded repeats, combined crop+transform shared aliases, replacement/removal,
  malformed wrapper nonpromotion and named ColorSpace binding preservation.
- [x] Implement crop helpers and model changes within copy/graph limits.
- [x] Run cargo test --lib existing_image (single job/shared target), expected all pass;
  retain red/green logs and exact test counts in execution ledger.

## Task2: native contract/publication

Files: types.rs, lib.rs, existing_image_edit.rs; new integration test
src-tauri/tests/existing_image_crop.rs. Reuse authored image fixtures/generation style.
Interfaces: public ImageCrop; optional crop in ExistingImage; Adjust {transform,crop}
variant, source identity includes derived crop. Only transform translation scales by
UserUnit; crop never scales. Post-read validates selected matrix/crop and siblings.
Always exact image-free context witnesses for crop changes. Pure crop retained-interior
RGBA equality excludes2px boundary; preserve cleanup on every rejected candidate.
- [x] Tests RED for all rotations/UserUnit1.7/CropBox, duplicate occurrence mapping,
  mirrored/skewed/alpha images, retained pixels/full restore, shared resources/notes/
  forms/text/later paint, policy and invalid/stale selection, saved re-edit and cleanup.
- [x] Implement facade validation, metadata mapping/identity and native proof.
- [x] Run focused native image/model/library and integration suites, expected green.
  Write public authored outputs for independent-reader acceptance, never private inputs.

## Task3: visible/full crop geometry

Files: types.ts; new editor/existing-image-crop.ts; existing-images.ts;
new tests/existing-image-crop.test.ts, existing-images.test.ts.
Interfaces: ImageCrop retained edges; fullCrop constant; validateCrop; cropFrame(full,crop);
imagePointToUnit(full,point); cropCorner/cropMove with offset/clamping; fullFrameFromVisible
(originalFull,crop,wantedVisible) via signed affine basis; crop-aware discovery polygons.
ImageEdit TS adds Adjust. ExistingImage crop optional, validated and cloned on discovery.
- [x] Tests RED for invalid edges/finite/minimum/roundoff; all rotations/page crop,
  skew/reflection and visible-to-full matrix round trips, unchanged source full frame,
  opposite corners/window clamp and hidden targets.
- [x] Implement pure helpers and discovery. Run focused Vitest expected green.

## Task4: crop controller/handles/compact workspace

Files: ImageReplacementDialog.tsx (extract controls/controller helpers if needed),
ExistingImageControls.tsx and new ExistingImageCropControls.tsx;
existing-image-canvas.ts, existing-images.css, PageView.tsx;
canvas/controller/app test files plus crop interaction tests.
Interfaces: CanvasImageView includes mode/crop/fullFrame and onCropChange callback;
position controls use a crop-aware logical image with visible corners. Shared controller
tracks full-frame basis, current crop and visible numeric draft; maps changes back to
full geometry for Adjust. Crop updates retain underlying full frame. Geometry/crop
mode switches preserve both; Replace/Remove remain separate actions.
Reset position restores source full frame, Restore full image resets only current crop.
Crop corners/window pointer capture, grab offset, arrow keys, cancel/zoom/session
behavior use the existing controls conventions. Do not trust or fake bitmap cutouts.
Compact action bar reserves status space, collapses precision/help in nonmodal mode,
and keeps Apply/Cancel/essential controls visible at narrow width; modal stays expanded.
- [x] Tests RED for combined crop/transform mode drafts, precision validity, Restore/
  Reset, latest pending debounce/borrowed sources/render errors, Cancel/one Apply/Undo,
  crop pointer/key/cancel interactions and compact focus/keyboard visibility.
- [x] Implement shared state/UI and preserve all prior replacement/image tests.
- [x] Full frontend plus typecheck expected green; failures corrected with evidence.

## Task5: final review and real app acceptance

- [x] Freeze exact scope/hashes and dispatch sole final correctness reviewer.
  Reproduce actionable findings, correct and run suites once; retain declined judgments.
- [x] Fresh source-fingerprinted debug build. Owned Windows/CDP normal/narrow crop,
  crop-window sliding, minimum clamp, uncrop, keyboard, zoom/crop/rotation and reading
  navigation; combined geometry/crop Apply/Undo/Redo; Cancel/Escape, saved reopen,
  Replace/Remove and immutable original hashes. Verify actions remain reachable.
- [x] Independently inspect saved PDF crop geometry/resources/image bytes, retained
  pixels, full restore, notes/text/forms/siblings/other pages. Check loaded PDFium hash,
  stop only owned app and confirm absence. Preserve every failed run and prior binary.
- [x] Update execution ledger, README, product quality ledger and verification.json;
  leave full goal active with honest compatibility/performance/security/UI limits.

## Preflight interface rulings

Task1→2: model crop is dimensionless, full matrix retained; facade only scales translation.
Task1→2: recognized logical occurrence path remains the parent Do; aliases must be updated
before wrapping its draw in q/cm/Q, or Adjust would paint the old resource.
Task2→3: public corners stay full-frame; crop-aware geometry derives visible corners once.
Task3→4: controls measure visible crop, controller converts desired visible affine frame
back to full before native request; crop change preserves full geometry.
Task4→5: one native candidate/lease controller owns all modes; acceptance tests must use
real controls, not only mocked viewport integration. No requirement waived for green tests.

Completion evidence and precise limitations: artifacts/image-crop-20261025/execution.md and checkpoint-corrected/verification.json. Native cases reuse tests/support/existing_image_crop_cases.rs. Final acceptance permits proven scope-equivalent wrapper removal on Restore; see spec ruling. Broad printing/performance/recovery, private-quiz rerun and exhaustive producer/form matrices are not claimed.
