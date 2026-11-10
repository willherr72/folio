# Desktop reusable form editing implementation plan

> Use superpowers:executing-plans inline, with one final scoped review.

**Goal:** Make verified native complete-source form edits available in the desktop.
**Architecture:** Complete adapter capability pair, existing dialog and frozen
document ownership, mapped complete-source commit with legacy adapter fallback.
**Tech stack:** Existing React/TypeScript/Tauri/Rust and Windows WebView acceptance.
**Spec:** docs/superpowers/specs/2026-10-24-occurrence-form-ui-design.md.

- [x] Add RED App/adapter/layer regressions for native form transactions.
- [x] Wire optional adapter methods, capability helpers, layer listing, App Apply
  and Tauri commands. Make legacy test adapters explicit. Run focused/full tests.
- [x] One scoped review and correction pass, TypeScript/build with fingerprints.
- [x] Windows edit/font/undo/save/reopen/fields/recovery acceptance and independent
  saved-file checks; record executable, evidence, limitations and next work.
