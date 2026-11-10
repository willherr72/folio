# Empty form discovery implementation plan

> Use superpowers:executing-plans inline with one final scoped review.

**Goal:** Avoid projecting pages with no reusable form text while preserving authorization.
**Architecture:** Check immediate object kinds after raw source admission and page authorization; retain existing form path.
**Tech stack:** Rust/PDFium, existing React/Tauri desktop and Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-24-form-discovery-design.md.

- [x] Measure current discovery on 100/400/1600 top-level and form pieces.
- [x] Add regression coverage for absent form content, unused/other-page form
  resources, annotation appearance forms, authorization and invalid page handling.
- [x] Implement the early exit; verify the real form path and repeat scale probe.
- [x] Review, build and validate Windows text selection; retain evidence and limits.

Source ownership, permissions, graph limits, annotation-only forms, actual nested
forms and invalid-page behavior are the review priorities. No cache, IPC or dialog
change. No source PDF mutation or release/commit implied.

Accepted together with the generated-separator correction, after the first desktop
run exposed that pre-existing mutation defect. Final evidence:
`artifacts/generated-separator-20261024/checkpoint-final/verification.json`.
