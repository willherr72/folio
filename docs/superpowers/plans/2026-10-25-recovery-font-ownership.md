# Recovery Font Ownership Implementation Plan

> **For agentic workers:** use superpowers:executing-plans inline; one final scoped review, parent fixes, no rereview.

**Goal:** Recovery cleanup cannot remove another operation's loaded fonts, while its own unused imports are released.
**Architecture:** An Arc-owned import scope carries a unique registry marker across recovery and native source opening. Cleanup matches explicit ownership rather than registry snapshots.
**Tech Stack:** Rust/PDFium, existing recovery storage and font registry.
**Spec:** docs/superpowers/specs/2026-10-25-recovery-font-ownership-design.md.

## Constraints

Finish the original native audit without source mutation first. Preserve failures,
exact baselines, source PDFs and all prior binaries. One Cargo/build at a time;
no commits or publication. Existing font limits remain16MiB/file,64 fonts,128MiB
total. Never hold a registry mutex while waiting for the native worker.

## Review focus

Unrelated registration during restore; same-font external adoption before rollback;
multiple source imports within one scope; failed generations and migration save
errors; native reply loss and existing image/protection cleanup.

## Task1 — deterministic failure evidence

- [x] Add a test-only thread-local observer in recovery.rs after font/source import
  and before final workspace validation. It consumes one callback on the calling
  test thread; no release-build hook or global test scheduling change.
- [x] New recovery ownership unit tests exercise normal load success/failure,
  same-font adoption on failure and legacy migration. Their callbacks register a
  unique valid font and assert it remains available after cleanup. Preserve the
  failing old behavior before implementing the scope.
- [x] Improve the existing word-spacing assertion to print its actual error without
  changing its expected semantic refusal. Preserve the poisoned-mutex failure and
  isolated passing evidence; do not serialize independent tests as the correction.

## Task2 — scoped registry and recovery integration

- [x] Add FontImportScope in a focused native module, with Arc<FontRegistry>, a
  unique Arc marker and a mutex-protected created-ID set. register/register_assets
  enforce existing batch limits; adopt(retained IDs) releases only owned unused
  assets, adopts referenced assets, and clears scope ownership. Drop rolls back.
- [x] Share existing registry admission through a scoped helper. Lock registry
  before scope. Same-scope reuse keeps the marker; external claims invalidate its
  cleanup authority permanently. Never acquire ownership of a preexisting asset.
- [x] Pass the scope through private OpenBytes/open_recovery_document. Keep normal
  open_bytes callers unchanged through a wrapper around a scoped implementation.
  Scoped imports do not also belong to a source's unpublished-font rollback list.
- [x] Replace restore and for_application registry-ID sweeps. A private load helper
  returns pending workspace/scope for migration; normal load adopts references only
  after validation, inside the error-cleanup boundary. Failed source/image cleanup
  stays intact; legacy migration owns fonts until save and temporary-source closure.
- [x] Unit scope tests cover repeated own import cleanup, external adoption,
  borrowed existing fonts and partial adoption. Run recovery fonts/images/store,
  physical/protected recovery and the full parallel text-state suite.

## Task3 — verification and review

- [x] Freeze exact sources and invoke one final scoped review. Parent reproduces and
  fixes demonstrated findings; no second review round.
- [x] Run corrected broad native gates, compile the desktop app, and bind current
  source/renderer hashes. Retain initial full-run results as failed baseline evidence.
- [x] Actual Windows recovery restores a custom-font draft and preserves subsequent
  editing/save/reopen. Recheck the quiz/text-form workflows after product changes.
- [x] Record evidence/limits and update the audit checklist and ledger. Leave the
  overarching commercial-quality goal active.

Final evidence: `artifacts/native-regression-audit-20261025/verification-corrected.json`.
The complete corrected run passed1,021tests/11ignored across98targets, exit0,
with unchanged source/test inputs. Focused326/3ignored, fresh Windows build and
three independent desktop workflows also passed. Review gaps and the unproven
cause of the original unprinted spacing error remain recorded. No parity claim.
