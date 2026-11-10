# Combined content removal implementation plan

Execute inline with superpowers:executing-plans and test-driven-development.
Spec: docs/superpowers/specs/2026-11-02-content-removal-workflow-design.md.
Goal: a usable combined-content preview/apply workflow with verified native
ownership, actual affected-area disclosure, history and desktop acceptance.
No commits, cleanup, source replacement or accepted-package replacement.

##1. Native owned preview
Files: redaction_content_preview.rs (new), redaction_content_preview_tests.rs
(new), redaction_preview.rs, redaction_area_selection.rs, redaction_image_cache.rs,
redaction_area_images.rs, engine.rs. Add failing combined-preview/ownership tests,
then implement typed summaries and budgeted pending geometry through the existing
session lifecycle. Retain text-only response/behavior. Verify image-only/mixed/
blank-page and close/expiry/release/adoption cases through actual WorkerRuntime.

##2. Paired raster and host bridge
Files: redaction_content_raster.rs and tests (new), redaction_host.rs,
redaction_commands.rs, engine.rs and command registration. Add failing actual
paired-device and all-page-Apply tests. Implement bounded fixed-size raster,
source-derived influence, outside-change refusal and encoded PNG accounting.
Expose new combined methods through real window sessions. Verify wrong owner,
stale pair, resource endpoints, missing page review and dropped receivers.

##3. Frontend workflow and dialog
Files: editor/text-removal.ts, text-removal-workflow.ts, adapter.ts,
components/TextRemovalDialog.tsx, a paired-raster hook, text-removal.css, App.tsx
and their existing tests. Observe failures for new explicit combined mode,
malformed/stale raster replies and reviewed-page gating. Implement validated
native paired previews and blob cleanup; preserve text-only behavior. Keep
single history commit and original source retention. Verify keyboard/navigation,
Cancel, async disposal, Undo/Redo, adapter and tab ownership.

##4. Integrated acceptance and review
Use owned mixed fixtures and current build. Complete serialized native suite,
frontend tests/build, actual Windows before/after/Apply/Save/reopen/recovery/Undo
and independent saved-PDF checks with controls. Freeze one integrated review,
correct demonstrated findings, checkpoint exact evidence and limitations. Public
feature/parity claims require their corresponding gates; the broad goal remains
active beyond this feature.

Review focus: image-only pages and empty marked pages; old JPEG grammar replay;
large retained descriptors and tall pages; cross-tab or delayed response cleanup;
all affected pages reviewed and metadata/whole-path expansion acknowledged.

## Completion record

Steps1–4 are complete for this scoped desktop milestone. Evidence and exact limits
are in `artifacts/content-removal-workflow-20261102/checkpoint.json`. The single
integrated review's ownership finding was observed failing, corrected, and verified
through native-2, build-2, desktop-2, desktop-restore-3 and independent audit-3.
The earlier failed recovery/audit attempts and review snapshot remain preserved.
This closes the combined desktop integration, not the broader JPEG compatibility
plan, portable-release qualification or the overall commercial-quality goal.
