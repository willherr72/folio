# Image PDF appearance implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with a fresh focused review after native and independent-reader verification.

**Goal:** Build the image resource used by portable/flattened PDF appearances, retaining alpha, sample depth, orientation and supported source color descriptions.

**Architecture:** Prepare a deterministic unit-square Form XObject from an immutable
ImageAsset. PNG color samples and alpha become separate losslessly compressed image
streams, retaining8/16-bit precision. JPEG retains its original DCT bytes and applies
EXIF orientation through the form's matrix. ICC resources describe original source
components, including CMYK; they never describe the asset's converted RGB preview.
Prepare fully before installing any objects, and keep the same resource graph available
for the subsequent portable appearance proof. This is phase2a of the full feature;
engine overlay/ownership, saved-copy import and recovery remain phase2b, UI phase3.

**Tech stack:** Existing Rust image/jpeg-decoder/lopdf and moxcms0.8.1 (already an
image dependency, now direct for bounded ICC parsing/writing); original-image fixtures,
PDFium, MuPDF, pypdf and Pillow/LittleCMS for output verification. No new dependency version.

**Spec:** `../specs/2026-10-23-image-placement-design.md`.

## Constraints and decisions

- Preserve original encoded assets and current portable builds; do not expose image
  controls until save/recovery/lifetime integration is complete.
- Use bounded original assets (64MiB,32Mpixels,32768dimensions). These are admission
  bounds, not a whole-process memory cap. ICC profiles capped at16MiB/4096tags.
- PNG supports ICC, explicit sRGB and gAMA/cHRM calibrated ICC descriptions (sRGB/D65 defaults
  for missing chromaticities). Source gAMA is inverted to the profile's decoding gamma; alpha
  is always linear. PNG HDR/non-sRGB cICP is explicitly refused pending tone mapping.
- Reject unsupported/malformed or component-mismatched ICC instead of silently
  assigning CMYK profiles to RGB preview samples. ICC v2/v4 input/display/output/
  color-space profiles, XYZ/Lab PCS, RGB/GRAY/CMYK channel signatures supported.
- CalRGB failed the actual PDFium test (128 instead of188 for gamma1), while MuPDF
  produced188. PDFium's image fast path ignores that calibration. Generate an equivalent
  ICC profile instead, retaining exact sample values. Normalize its generated creation
  date to2000-01-01 because the dependency writes wall-clock time. Original ICC bytes
  remain unchanged. Bounded unaligned input tag ranges are tolerated, matching readers;
  generated gray profiles use parametric curves, avoiding8.8 gamma quantization.
- JPEG color-transform hints and CMYK decode polarity accompany original samples;
  test all EXIF orientations and baseline/progressive/gray/CMYK reference inputs.
- Prepare returns a complete nested stream graph without mutating a PDF. Install
  makes every stream indirect recursively. Overlay geometry remains outside this unit.

## Review focus

1. Transparent samples/16-bit byte order: exact sample and alpha assertions plus render.
2. Mirrored JPEG orientation: all8 EXIF transforms, non-square asymmetric colors.
3. ICC component/profile mismatch: negative tests, original profile/sample identity.
4. PNG color precedence/gamma: gamma1 vs default, sRGB/ICC priority, HDR refusal.
5. Native text/background preservation: overlay output tested with independent readers.

## Task1: Deterministic resource and sample tests

Files: new `src-tauri/src/image_pdf.rs`, `src-tauri/src/image_pdf_color.rs`,
`src-tauri/tests/image_pdf.rs`; module export in `lib.rs`.
Public interface: `prepare_pdf_image(&ImageAsset)->EngineResult<PreparedPdfImage>`;
`PreparedPdfImage::form()->&lopdf::Stream`,
`PreparedPdfImage::install(self,&mut lopdf::Document)->lopdf::ObjectId`.
The public form exposes an immutable uninstalled resource graph for proof/testing.

- [x] Add tests for PNG exact color/alpha8/16 samples, unit-square form and independent
  streams; JPEG original DCT bytes, EXIF geometry; profiles/gamma/invalid metadata.
- [x] Observe missing-interface RED, then implement pure preparation and recursive
  stream installation. Verify native tests GREEN.
- [x] Generate PDFs containing unchanged native text/background and image forms;
  independently inspect resources and render alpha/orientation/color results.

## Task2: Review and integration handoff

- [x] Run asset/resource suites, app compilation and original/profile fidelity checks.
- [x] Fresh focused review; reproduce/fix material findings and rerun affected checks.
- [x] Record evidence and limitations; next implement Overlay::Image, portable proof/
  reimport leases, engine resource ownership and private recovery before UI work.

Reference decisions: [PDF Reference1.6](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf)
(DCT marker precedence/ColorTransform and image soft masks) and
[PNG third edition](https://www.w3.org/TR/png-3/) (color precedence; alpha is linear),
inspected2026-10-23. No feature/parity completion claim follows from this subphase.
