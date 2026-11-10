# Folio 0.31 development portable integration

## Outcome and authority
Deliver a new optimized desktop/CLI/MCP portable checkpoint containing the
verified combined content-removal workflow. The previous goal turn made concrete
progress: its corrected native, Windows recovery and independent saved-PDF gates
are bound by `artifacts/content-removal-workflow-20261102/checkpoint.json`.
The broad Adobe/Foxit-quality objective remains active. Standing authorization
opts out of routine design questions; execute inline and preserve existing work.

## Approach
Reuse the existing offline Rust/Tauri packaging pipeline and strict ZIP verifier,
with a new version and fresh acceptance directories. A debug-only checkpoint
would leave the new feature unavailable in the portable distribution. Replacing
0.30 would destroy a known runnable fallback. Coordinator choice: create
`0.31.0-dev.1` beside it, keeping all earlier artifacts unchanged.

## Scope and invariants
- Update only the five version-bearing package/Cargo/Tauri files to
  `0.31.0-dev.1`; describe new behavior and limits in the changelog/README.
- No commit, merge, push, remote release, user-app restart or printing.
- Freeze product and test inputs before the full default native suite. Run
  Cargo serially, offline, locked, with failure-continuing reporting. Diagnose
  failures; never turn incomplete targets or existing ignored tests into passes.
- Reuse the 1,450-test frontend result only if its frontend product inputs are
  unchanged apart from version metadata. The optimized package runs a fresh
  TypeScript/Vite build. Changes to frontend behavior require appropriate new
  tests and renewed frontend qualification.
- Build GUI and CLI from the same fingerprint with the pinned PDFium renderer,
  vendored JPEG codec/license record and offline OCR assets. Existing release
  directories/archives must not be overwritten.
- Verify archive paths, exact file set, hashes, renderer/dependency provenance,
  licenses and version before extraction; verify the extracted copy again.
  Run it from outside the checkout with isolated owned profile directories.
- Exercise the extracted CLI/MCP using the existing 37-step original-font,
  paragraph, nested-object, form, protected/no-op and independent-output checks.
- Exercise actual packaged desktop combined preview/all-page review, Cancel,
  Apply, native Save, Undo/Redo, recovery with its original copied input absent,
  reopen, strict retained-pixel refusal and tall-page bounds. Require exact
  edited-source recovery checkpoint identity, not merely a dirty flag.
- Audit packaged saved outputs in PDFium and ordinary RGB MuPDF: retained text,
  vector absence, selected/unselected JPEG coefficients, unchanged outside pixels,
  and rejected corruption controls. Keep source-defined masks and known refusal.
- Check packaged existing-text/paragraph editing and offline OCR with actual
  desktop workflows and independent saved-output checks; preserve source inputs.
- One fresh integrated review follows the complete milestone. Correct demonstrated
  Important/Critical findings with evidence. Record unresolved coverage limits.

## Completion evidence and limits
Record exact executable/archive/source/helper/input/output hashes and completed
gate results in a new checkpoint. Do not relabel older printing, resource,
structured-document or browser/IME evidence as fresh 0.31 acceptance. Raw-image
only removal, masks/transparency/profiles, broader producers, performance,
accessibility, certificates and commercial parity remain separate requirements.
The release milestone is complete only after extracted-package acceptance and
review disposition, not when a ZIP merely exists.
