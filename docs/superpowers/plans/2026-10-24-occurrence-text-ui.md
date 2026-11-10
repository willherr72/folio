# Complete-source Desktop Text Editing Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing user authorization applies; no routine approval, commits, cleanup or release.

**Goal:** Make shared-field single-run text editing usable in the actual desktop app.

**Architecture:** Optional adapter capability pair, existing selection/dialog,
complete-source native transaction and existing validated occurrence commit.

**Tech Stack:** React/TypeScript, Vitest, Tauri/Rust, actual Windows WebView workflow.

**Spec:** `docs/superpowers/specs/2026-10-24-occurrence-text-ui-design.md`

## Constraints and review focus

Preserve source ownership across tabs/history, immutable inputs, pending request
lifetime, current page/overlay/bookmark state and protected rights. No isolated
fallback after full-source refusal. Legacy paragraph/group/form behavior remains.

- [x] Add failing protocol, canvas and App tests for full-source edits/undo and malformed/borrowed/late replies.
- [x] Wire native IPC, adapter pair, canvas inspection and App commit; retain explicit legacy fallback coverage.
- [x] Run focused and full frontend/type checks; one scoped review and important correction pass.
- [x] Build a staged executable and verify actual Windows shared-field editing, undo/save/reopen/filling and recovery.
- [x] Independently inspect saved outputs and persist accepted hashes/evidence/remaining limitations.

Acceptance exposed a short-page navigation defect, corrected with a failing-then-
passing viewport regression. Final full frontend: 886 passes; corrected build and
Windows editing/filling/recovery passed. Evidence and remaining limits are in
`artifacts/occurrence-text-ui-20261024/checkpoint-corrected/verification.json`.
