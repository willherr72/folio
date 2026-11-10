# MuPDF JPEG influence implementation plan

Execute inline under standing authorization and the existing phase3 final review
boundary. Spec: docs/superpowers/specs/2026-11-02-jpeg-mupdf-influence-design.md.
Preserve originals, accepted v0.30, predecessor evidence and public JPEG=false.

- [x] Derive bounded native source-window/cache plans. Add failing cases, reuse
  the interval rectangle transform without changing existing affine exports,
  and verify actual native tile frames, sample dimensions and requested extents.
- [x] Compose native post-decode scaling and affine routes with shared resource
  accounting and source-bound JpegSelection ownership. Test every route and the
  rounding/clip/empty/failure boundaries.
- [x] Audit complete saved JPEG PDF rendering in MuPDF, including cache variants,
  and independently verify retained evidence. Run appropriate serial regression,
  preserve predecessor outputs and checkpoint the complete reader connection.

Review focus: 90% full-tile tuning, 1% edge snapping, requested versus actual cache
frame, zero returned extents, reflected quarter-turn scaled-frame orientation.
No per-helper delegation or intermediate release/feature completion claim.

Completed reader-connection evidence is recorded in
`artifacts/jpeg-mupdf-influence-20261102/checkpoint.json`: 321 scoped native passes
(one explicit skip), 185 focused passes, 25056 actual saved-PDF render pairs and
8352 native tile checks. The independent pypdf/byte verifier checks all 50112
PDFs and retained pixels. All 647 predecessor exports remain identical. The
parent JPEG phase3 remains open for canonical viewer binding, compound opaque
paint/publication acceptance and its one final fresh integrated review.
