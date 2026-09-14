# Isolated form occurrences implementation plan

> Execute with superpowers:subagent-driven-development and independent review.

**Goal:** Edit one selected nested text occurrence without changing shared copies.
**Architecture:** Bounded occurrence discovery, private resource-chain cloning, verified encoded replacement; existing frontend text dialog and immutable source commit.
**Tech:** Rust/PDFium/lopdf, Tauri, React/TypeScript.
**Spec:** ../specs/2026-09-14-form-occurrences-design.md

## Native deliverable

Owner: native implementer; new form_text.rs/resource helper modules and tests/form_text.rs, plus engine/app/types/lib wiring. Do not modify frontend.
- [x] Write failing shared-form identity/isolation tests using list_form_text_runs and replace_form_text.
- [x] Implement bounded graph and unique invocation mapping; prove standard Type1 ASCII font context; clone selected resources and preserve original state.
- [x] Implement native listing and whole-page no-op/replacement validation, immutable publication and cleanup.
- [x] Add nested/inherited/transformed/refusal/resource tests and authored references; run focused Cargo tests.

## Frontend deliverable

Owner: root; editor/types.ts and adapter.ts, ExistingTextLayer/Dialog, App.tsx and focused tests.
- [x] Write failing path-key, explicit occurrence routing and late-response/source cleanup tests.
- [x] Combine optional form candidates, retain complete path and existing modal/history logic; show occurrence hint and disable group/substitution for forms.
- [x] Run focused tests/build and preserve old top-level behavior.

## Verification/release

Owner: root and independent reviewer/probe.
- [x] Review native resource/encoding/geometry protections; run independent reader references.
- [x] Create and verify original practice PDF, full regression suites and optimized build.
- [x] Test packaged Windows native editing, undo/search/save/reopen/recovery; audit package.
- [x] Prepare accurate behavior/issue status and verified release evidence. Publish after the final archive audit under standing release authorization.

One Cargo process at a time, -j1, primary cached debug target. Root owns release build slot only after native handoff. Use existing positioned-text worktree on form-occurrences branch. Do not delete it or build caches. No arbitrary source traversal without explicit depth/byte/visit bounds. All desktop testing uses isolated owned PID/path and synthetic files.

Ruling: scope graph validation to new form APIs before native page traversal. Applying strict content parsing to every ordinary Open introduced unrelated compatibility and parser risks. General renderer hardening and annotation-appearance traversal are outside this release.
