# PDFium affine sampling plan

Execute inline under standing authorization. Keep source selection, existing
documents, accepted v0.30 and public JPEG admission unchanged.

- [x] Add failing sampling/support tests and API stub.
- [x] Implement quantized matrix, float roundoff and complete-pixel influence.
- [x] Compile the source-derived native probe, audit and independently verify
  saved pixels, run serial native regression and compare all predecessor exports.

Spec: docs/superpowers/specs/2026-11-02-pdfium-affine-design.md. The standing final
phase3 review boundary remains; no per-helper delegate or new release package.

Evidence: `artifacts/jpeg-pdfium-affine-20261102/checkpoint.json`. The final
implementation passes 158 focused and 294 ordinary native tests, with one explicit
native skip. The pinned-source probe passes 642 comparisons across 386 owned input
intervals; all 13254 changed pixels are contained and all saved bytes independently
verified. The 585 predecessor exports remain identical. Full DLL/page rendering,
prior stretching and the enclosing publication plan are not completed by this gate.
