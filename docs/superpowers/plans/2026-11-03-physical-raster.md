# Physical-pixel raster implementation plan

> Use superpowers:executing-plans inline for production implementation. The first
> task is a bounded architecture spike, with independent evidence rather than a
> production implementation claim.

Spec: docs/superpowers/specs/2026-11-03-physical-raster-design.md.

1. Verify prior checkpoint and current PageView/native render contract. Build a
   physical-grid HTML canvas spike under artifacts/physical-raster-20261103;
   preserve fixtures and test both focused and blurred display at DPR1/1.25.
   Expected: a decisive result for exact derived-buffer presentation and physical
   geometry; failures remain failures. No product edits in this step.
2. If supported, add tested projection/lifecycle implementation and integrate the
   real page surface, preserving interaction layers, fallback and source ownership.
   RED→GREEN geometry, update ownership, allocation bounds and disposal tests;
   browser controls must retain source identity, crop, rotation and focus.
3. Verify actual Windows editing, saved PDF fidelity and unchanged resource budgets
   on a fresh candidate. One final fresh scoped review for implemented code; no
   release or parity claim without broader evidence. Unsupported spike results
   instead become a documented blocker to this integration, with a concrete next
   discriminator and unchanged product checkpoint.

Review focus: native versus derived pixels; fractional physical origin and DPR;
white paper versus transparent image data; source/edited ownership during async
replacement; bounded surface reuse and actual Windows lifecycle/resource behavior.

Outcome: the browser component experiment passed, but actual Windows presentation
failed and sustained memory improvement was not established. The implementation
and review were archived; active product source was restored byte-for-byte to the
accepted history/caret baseline, with1483frontend tests and a successful rebuild.
This plan closes as a rejected optimization, not an accepted rendering feature.
See artifacts/physical-raster-20261103/milestone-result.json and next-investigation.md.
