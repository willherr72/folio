# PDFium image placement implementation plan

Execute inline with the executing-plans skill and the existing phase3 final
review boundary. No per-helper delegation, commit or package replacement.

Goal: connect decoded sample influence to the installed AGG bitmap renderer.
Spec: docs/superpowers/specs/2026-11-02-pdfium-image-placement-design.md.
Architecture: bounded native branch/rectangle enumeration and existing outward
arithmetic, signed stretch and affine sampling. Rust product; Python DLL audit.

- [x] Add failing tests for normal/reflected/quarter/general placement, clipped
  intermediate frames, uncertain branch thresholds and budget refusal.
- [x] Implement RendererTransform::pdfium_image_bounds in
  src-tauri/src/redaction_render_pdfium_image.rs with private placement helpers;
  export varied owned cases from redaction_render_pdfium_image_tests.rs.
- [x] Audit installed PDFium raster changes and independently verify saved bytes;
  run scoped native regression and compare all predecessor exports. Record exact
  evidence and limitations, keeping combined publication and public gates open.

Review focus: fractional outer/closest rounding, crop-local offsets, reflected
swap axes, branch thresholds, bounded amplification. Each is covered by the
owned corpus or focused admission tests. Final phase3 review remains required.
