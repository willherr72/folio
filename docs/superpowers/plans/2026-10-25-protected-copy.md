# Protected PDF copy implementation plan

> Use superpowers:executing-plans inline. The user authorized autonomous work without
> routine approval. One final scoped reviewer; preserve existing dirty work and artifacts.

**Goal:** Export edited PDF copies with deliberate new or removed password security.
**Architecture:** Explicit transient security option on controlled save; native owner
checks before composition, standard revision-6 encryption before atomic publication.
**Stack:** Rust/lopdf/PDFium/Tauri, React/TypeScript, Vitest, Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-25-protected-copy-design.md

## Constraints and review focus

No secrets in recovery/history/progress/debug/errors. Raw <=1024 and normalized
<=127 UTF-8 bytes; owner nonempty, normalized passwords distinct. AES-256 R6 with
metadata encryption and OS randomness. No source overwrite or ordinary-save policy
change. Every protected source requires owner access for explicit security changes.
Tests must cover normalization-equivalent credentials, annotation/fill permission
dependency, mixed protected sources, cancellation around encryption and denied
export preserving an existing destination. Original build and failed evidence retained.

## Task 1: Native security options and protected composition

Files: new src-tauri/src/protection_output.rs; modify protection.rs, engine.rs,
document_policy.rs, lib.rs, Cargo.toml/lock; new tests/protected_copy.rs.
Interface: public `ProtectionChange` tagged `mode`, variants `Remove` and
`Passwords { user_password, owner_password, permissions: DocumentPermissions }`;
redacted Debug, serde camelCase. `validate()` and `apply(&[u8])->Result<Vec<u8>>`.
`PdfEngine::export_pdf_with_security(request,bookmarks,destination,change,control,progress)`
uses existing controlled save; absent change preserves old behavior. Internal
composition accepts optional security choice and checks all sources before copying.

- [ ] Write/run failing native tests for new protection, replacement/removal,
      owner-only mixed-source changes, invalid credentials/permissions and cancellation.
- [ ] Implement bounded new encryption using existing protection load/save helpers,
      OS random key, normalization validation and explicit native authorization.
- [ ] Run protected_copy, pdf_save, pdf_protection, protected_engine and relevant
      preservation tests; inspect failures and verify output with independent readers.

## Task 2: Transient desktop security dialog and save integration

Files: new src/editor/protected-copy.ts, components/ProtectedCopyDialog.tsx;
modify pdf-save.ts, pdf_save_commands.rs, SaveCopyButton.tsx and App.tsx;
new tests/protected-copy.test.tsx and tests/protected-copy-app.test.tsx.
Interface: `ProtectionChange` TS discriminated union mirrors native. Dialog receives
frozen pages/bookmarks, adapter and onClose/onComplete; owns transient inputs and
controlled-save task/progress, blocks double submit and guards cancel/unmount.

- [ ] Add failing UI/API tests for confirmations, permissions, ownership, pending
      native save/cancel/retry and unchanged active tab/history/dirty state.
- [ ] Wire optional `security` IPC argument, preserve existing callers, add menu/dialog.
- [ ] Verify focused tests then full four-worker frontend suite and production build.

## Task 3: Review, desktop acceptance and checkpoint

- [ ] Freeze baseline/delta/hashes; obtain one scoped review and fix demonstrated
      defects through failing-then-passing tests plus affected regressions.
- [ ] Build isolated executable, run real Windows protection/owner/removal/cancel
      workflows and independent credentials/permissions/feature/pixel checks.
- [ ] Record binary/source/PDFium hashes, original preservation and owned-process
      cleanup; update README/quality ledger. Keep broad goal active, no release/commit.
