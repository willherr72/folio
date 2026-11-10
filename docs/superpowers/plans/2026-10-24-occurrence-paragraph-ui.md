# Complete-source Paragraph Desktop Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing autonomous authorization applies; no routine approval, commit, cleanup or release.

**Goal:** Usable paragraph previews and Apply preserve complete-source ownership.
**Architecture:** Shared explicit transaction results; existing dialogs draft and
render the mapped page; App commits validated occurrence mappings.
**Tech Stack:** React/TypeScript/Vitest, Tauri IPC, actual Windows WebView acceptance.
**Spec:** `docs/superpowers/specs/2026-10-24-occurrence-paragraph-ui-design.md`

## Review focus

Cross-tab/history borrowed responses; malformed mappings; deferred native/render
callbacks after cancel/unmount; synchronous Apply unmount; stale frozen page arrays;
mixed-source/overlay/crop/protected duplicate state; capability consistency and
font-lease lifetime. Legacy fallback is capability-based only, never error-based.

- [x] Add failing transaction, adapter and both-dialog tests; implement optional
  native capabilities and `src/editor/paragraph-transaction.ts` lifecycle.
- [x] Wire Tauri commands in app.rs, mapped rendering and ownership in both dialogs,
  and App context/capability/commit routing. Verify App duplicate and undo regressions.
- [x] Run focused/full frontend and type checks; one scoped review/fix pass.
- [x] Build and run Windows paragraph/form/save/reopen/recovery workflows; inspect
  outputs independently and record accepted hashes with explicit limitations.

Acceptance evidence: `artifacts/occurrence-paragraph-ui-20261024/checkpoint/verification.json`.
No release or parity claim; broader goal remains active.
