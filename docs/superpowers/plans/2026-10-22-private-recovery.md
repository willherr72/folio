# Private recovery implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline.

**Goal:** Windows recovery is account-protected at rest and restored directly into
memory while existing editing/recovery behavior remains usable.

**Architecture:** A bounded DPAPI envelope module wraps every recovery payload.
Recovery migrates legacy owned files and verifies decrypted data before sending
bytes to the engine worker; file opening and byte opening share initialization.

**Tech Stack:** Rust, windows-sys0.61.2 DPAPI, SHA256, existing recovery/PDFium.

**Spec:** ../specs/2026-10-22-private-recovery-design.md

## Constraints and review focus

- Current-user scope; no prompts, new external dependencies or plaintext fallback.
- Limits:512MiB payload,64KiB envelope overhead,32MiB workspace,16MiB font.
- No secret/payload logging, external file migration, commits or package replacement.
- Review wrong-purpose/length-header tampering, interrupted migration, source reuse,
  redirected paths and missing originals/hardlink export guards. Tests below pin them.
- Existing unrelated work remains. One fresh reviewer and one material fix pass.

## Task 1: native envelope and recovery integration

Files: new src-tauri/src/private_storage.rs; lib.rs export; Cargo.toml Windows
Cryptography feature; recovery.rs; engine.rs; recovery_store.rs; new private_storage
integration tests; synthetic probe under src-tauri/examples as needed.

Interfaces: `seal(Purpose,&[u8])->Result<Vec<u8>,String>`;
`unseal(Purpose,&[u8],usize)->Result<Vec<u8>,String>`; internal header inspection for
bounded cached size checks. Purpose has Workspace,Source,Font variants.
`open_recovery_document(Vec<u8>,snapshot_path,original_path,register_fonts)` is
internal to native engine/recovery; it does not expose a new frontend command.

- [x] Write failing DPAPI-envelope and recovery-at-rest tests.
- [x] Run focused tests; expect failures for missing API/plaintext files.
- [x] Implement envelope, checked Windows buffers and feature selection.
- [x] Route recovery reads/writes through envelopes and legacy migration; reuse
      unchanged sources. Restore through a byte-based engine request.
- [x] Update existing white-box recovery fixtures to decrypt/reseal intentional
      corruption; preserve their original behavioral assertions.
- [x] Add migration, wrong-purpose/header/ciphertext, font corruption/rollback,
      source reuse and original-file protection regressions; run relevant suites.
      Expected: all pass with no plaintext content in private storage.

## Task 2: review and actual Windows acceptance

- [x] One fresh reviewer checks this milestone's diff/spec/plan and recorded evidence.
- [x] Reproduce material findings with failing tests and fix them; no second round.
- [x] Run native regression/production/desktop builds and independent DPAPI probe.
      Expected: all relevant checks pass; ignored platform workloads remain explicit.
- [x] Actual isolated Windows edit/forced-exit/recovery/save/reopen; inspect recovery
      files and exported PDF values/pixels and original hash. Close owned processes.
- [x] Update quality ledger, README recovery behavior and remaining lifecycle work.

Desktop acceptance build: `cargo build --manifest-path src-tauri/Cargo.toml
--bin folio --locked --features tauri/custom-protocol`. This embeds the production
frontend; plain Cargo builds navigate to devUrl. Use the isolated helper
artifacts/owned-private-recovery-app.ps1 and port9436, never a user app process.
