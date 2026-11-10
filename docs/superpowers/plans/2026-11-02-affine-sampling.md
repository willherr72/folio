# Affine sampling implementation

Execute inline under standing authorization using the existing final phase3
review boundary. Keep accepted v0.30 and source documents unchanged.

- [x] Add failing support tests and a renderer-only API stub.
- [x] Implement scan-origin, fixed-step and complete-pixel influence bounds.
- [x] Audit owned native drawing, persist and independently verify all pixels,
  run serial native regression and compare predecessor exports.

Spec: docs/superpowers/specs/2026-11-02-affine-sampling-design.md.
No commit, new portable package, public JPEG enablement or per-helper delegate.

Evidence: `artifacts/jpeg-affine-sampling-20261102/checkpoint.json`. Native
regression passes 289 tests, with one explicit skip; the fast harness passed 153
before the final zero-step refusal assertion (included in native verification).
1248 owned cases and 1824 matrices produce 3336 actual drawing pairs. All 61470
changed pixels are contained. Independent byte verification covers every source
and saved output; 584 earlier exported artifacts remain identical. This completes
the conditional MuPDF affine-stage prerequisite, not whole-publication acceptance.
