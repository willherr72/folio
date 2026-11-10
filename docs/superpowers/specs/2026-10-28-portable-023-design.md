# Portable 0.23 development integration

The user wants autonomous progress toward an everyday PDF editor and values local
CLI/MCP automation. Several reviewed checkpoints now postdate portable0.22.0-dev.1.
Ship those improvements together in a separately named local development package
so desktop and automation users can use the same tested engine.

Coordinator choice: reuse the existing optimized combined Windows portable
pipeline at version0.23.0-dev.1. Copying the debug candidate would omit a fresh
release-configuration check; maintaining separate distributions would duplicate
resources and obscure which engine is in use. No new runtime dependency or product
feature is required. This is local packaging, not publication or parity approval.
The user has opted out of routine questions and chose inline work with one final
scoped review; no design/per-task delegates or permission checkpoint is needed.

Preserve every older package, archive, input PDF, failed run, review and user's
running editor. Do not commit, push, publish, delete or overwrite an existing
package. Keep the current linked worktree. One Cargo process, locked/offline,
jobs1, serial native tests, target
C:/Users/WilliamHerr/Desktop/Code/Folio/src-tauri/target. Product/test/build inputs
remain immutable during gates and builds.

Bump package.json, its root package-lock entries, Cargo.toml, Folio's Cargo.lock
entry and tauri.conf.json together. Preserve dependency versions and LF Cargo
manifest formatting. Add a development changelog entry. Make README point to one
current package and its included guides instead of stale per-feature executables;
retain historical evidence in the quality loop and an exact pre-edit snapshot.

Reuse scripts/build-portable.ps1, verify-portable.py and their existing tests.
The package must include both release executables, the pinned renderer/patches,
offline OCR assets, examples, licenses and guides. Source/build metadata and ZIP
contents must agree. Verify the archive before extracting it into a fresh owned
temporary directory outside the checkout, then verify that extracted directory.
The verifier establishes integrity, not publisher authenticity.

Acceptance:
- Fresh frontend and Python packaging/launcher suites; a complete default native
  suite on the new version plus the two explicitly supplied original Word tests.
  Keep ignored workloads and actual counts explicit; never infer a pass.
- Optimized desktop and CLI build from the gated source; immutable source/test and
  renderer fingerprints plus both executable hashes, ZIP hash and asset manifest.
- Extracted CLI help/version, info, text and rendering without repository-relative
  renderer lookup. Current text/paragraph/form and nested automation workflows,
  explicit fonts, saved original-font edits and no-op/protected identity controls.
  Reuse established public fixtures/harnesses; bind all reports to extracted binaries.
- Native desktop existing-text and paragraph workflows with save/reopen, explicit
  font use where necessary, Undo/Redo and independent output fidelity. Exercise
  packaged OCR assets using the existing offline smoke workflow. Use owned data
  directories and process identity guards; do not touch user/shared browser tabs.
- Independent glyph/pixel/graph checks on representative saved outputs, with
  unmodified input and old-package hashes. Repeated assembly must refuse the
  existing destination. One final frozen review, fix demonstrated important defects
  with reproductions, then record the package checkpoint.

General producer/script support, secure redaction, MCP password entry, certificates,
physical printing, 8GB hardware and commercial parity remain separate open goals.
The package does not change those limits or imply a fresh acceptance of every
possible desktop workflow.
