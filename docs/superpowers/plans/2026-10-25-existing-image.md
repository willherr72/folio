# Existing image replacement implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline, task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Replace one existing PDF image, preview it and undo it without changing shared uses or document context.

**Architecture:** A bounded raw content model isolates one image invocation; the existing private projection and full-source publisher preserve document ownership and fidelity. The desktop adds image selection and a candidate-owning replacement dialog.

**Tech Stack:** Rust/lopdf/PDFium, Tauri, React/TypeScript, Vitest, Windows CDP acceptance.

**Spec:** docs/superpowers/specs/2026-10-25-existing-image-design.md

## Global constraints

- No commits, integration or release. Preserve dirty worktree, prior binaries, source PDFs and failure evidence.
- Source512MiB; total model decoded content16MiB; expanded operations100000; depth12; image occurrences5000; q stack256; direct copies16MiB/100000 values; reference depth64.
- Images32768 per axis/32MP each/80MP unique referenced source images including masks; incoming64MiB/32MP; prepared cache128MiB.
- Affine coordinate magnitude1e8; reject degenerate transforms; physical units converted once.
- Explicit form Resources, no Type3/Pattern/ExtGState soft-mask consumers in admitted scopes. No marker-based cleanup authority.
- Full glyph equality includes generated separators. Pixel-change region selected quad plus at most2 raster pixels.
- All tasks inherit the full spec. Rulings and command evidence go in artifacts/existing-image-20261025/execution.md.

## Review focus

- Annotation appearances inheriting page XObjects: preserve before alias rebinding (Task2).
- Repeated replacements after save/reopen: bounded wrappers/resources and stable frame (Tasks1/2).
- Sheared/reflected quads partially or wholly outside crop: visible polygon keyboard targeting (Task3).
- Late preview response uses another tab's source ID: never close borrowed source (Task3).
- EXIF alpha/physical-unit combinations: actual saved pixels and dimensions (Tasks2/4).

### Task 1: bounded raw model

Files: create src-tauri/src/existing_image_model.rs and existing_image_model_tests.rs; register private module in lib.rs.
Interfaces: `Model::read(bytes:&[u8],page_index:usize)->EngineResult<Model>`;
`Model::images()->&[Occurrence]`; Occurrence has path Vec<usize>, matrix [f64;6], corners [[f64;2];4], width/height u32.
`Model::rewrite(path:&[usize],replacement:Option<(&PreparedPdfImage,ImageFitMode,f64)>)->EngineResult<Vec<u8>>`;
None means resource-chain clone with unchanged content. f64 argument is oriented asset aspect ratio.

- [x] Write tests for shared leaf and parent uses, nested affine composition, missing/inherited scopes, malformed matrices/paths, cycles and budgets, fit matrices, exact resource isolation, repeated frame replacement and rejected forged wrappers.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib existing_image_model -- --nocapture` to record red.
- [x] Implement strict traversal, safe alias isolation, prepared installation and exact structural frame recognition; preserve unmodified stream dictionary and operator values.
- [x] Run the same focused suite green; record tests and rulings.

### Task 2: native full-source API and proof

Files: create src-tauri/src/existing_image_edit.rs, src-tauri/tests/existing_images.rs; modify engine.rs, types.rs, lib.rs, app.rs; narrow text_edit.rs helper visibility only as needed.
Interfaces: `ExistingImage {object_path,selection_id,corners,bounds,pixel_width,pixel_height}`;
`ExistingImages {images}`; `ImageFitMode {Fit,Fill,Stretch}`.
`PdfEngine::list_existing_images(&self,source:&str,page:usize)->EngineResult<ExistingImages>`;
`PdfEngine::preview_image_replacement_occurrence(&self,source:&str,occurrences:&[usize],selected:usize,path:&[usize],selection_id:&str,asset:&str,mode:ImageFitMode)->EngineResult<PageContentReplacement>`.

- [x] Write facade tests for shared/nested replacement, duplicate mapping, retained text/glyphs/annotations/pages, source immutability, protected permissions/origins, save/reopen/re-replace, UserUnit1.7/crop/rotation and source identity mismatch.
- [x] Run focused integration suite red; implement worker request dispatch, bounded input checks and source/asset authority.
- [x] Reuse with_text_projection/publish_text_occurrence; validate clone exactness, glyph/geometry and selected-quad pixel proof; clean every temporary/error/abandoned registration.
- [x] Add Tauri calls, public types and physical geometry conversion. Run focused model/facade plus related image/publishing regressions green.

### Task 3: desktop selection and transaction

Files: create src/editor/existing-images.ts, src/components/ExistingImageLayer.tsx, ImageReplacementDialog.tsx and CSS; modify adapter.ts, types.ts, App.tsx, PageView.tsx, DocumentViewport.tsx. Tests: tests/existing-images.test.ts, existing-image-layer.test.tsx, image-replacement-dialog.test.tsx, existing-images-app.test.tsx.
Interfaces: adapter methods mirror camelCase native facade. Layer calls onEditImage(pageId,image). Dialog receives document/page/image and onApply(candidate)/onCancel; uses existing lease/replacement validation helpers.

- [x] Write red tests for crop-clipped polygons, keyboard selection, explicit preview/apply, invalidation on edits, stale/unmount/borrowed-source cleanup and late import release.
- [x] Implement Edit images tool and layer; block annotation gestures while selected; retain source and document identity guards.
- [x] Implement Fit/Fill/Stretch, Original/Edited comparison, zoom, error/busy state and candidate ownership.
- [x] Run focused tests; add Apply/Undo/Redo and cross-tab app regressions. Run full frontend and typecheck green.

### Task 4: review and executable acceptance

Files: artifacts/existing-image-20261025/*, docs/product-quality-loop.md and capability docs.

- [x] Freeze scoped hashes and request one fresh correctness review; regrade findings, one test-verified correction pass, no automatic rereview.
- [x] Build fresh debug executable using artifacts/build-page-image-export.ps1 into a new checkpoint; verify fingerprint.
- [x] Generate public fixtures and run isolated Windows workflow including shared image, alpha/EXIF, geometry, comparison, apply, Undo/Redo, save/reopen/re-replace and narrow keyboard interaction.
- [x] Verify saved results with independent MuPDF/pypdf: selected pixels, unchanged siblings/pages/text, resource growth and original hashes. Stop owned app by recorded identity.
- [x] Record exact evidence/limits, update quality ledger and preserve accepted prior binary. No parity/release claim.
