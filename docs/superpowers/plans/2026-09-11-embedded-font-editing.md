# Embedded Font Editing Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or executing-plans task-by-task, with independent review.

**Goal:** Polish font selection, deliver #12 and complete the actionable investigation/staging requested by #13.
**Architecture:** Existing immutable derived-page edits with validated embedded encodings and an optional explicit replacement FontRegistry asset; shared polished preview picker.
**Tech Stack:** Rust, PDFium/lopdf/ttf-parser, Tauri, React/TypeScript, native and browser fixtures.
**Spec:** ../specs/2026-09-11-embedded-font-editing-design.md

- [x] Picker: FontPicker.tsx/CSS and picker tests; selectable preview, Apply/Cancel, consistent Print layout, keyboard/light/dark/height checks. Preserve resource ownership and adapt existing app/harness expectations.
- [x] Native #12: text_edit.rs and focused embedded-font helper/tests, engine.rs/types.rs contract. Add optional substitution API while retaining old call compatibility. Start with failing real embedded/subset fixtures, prove mapping/no-op/after-edit guarantees, test independent glyph/missing/ambiguous cases.
- [x] Integration: app.rs command, adapter/types, ExistingTextDialog/App. Expose font support details, retain typed input after missing-glyph rejection, choose previewed substitute explicitly, release temporary fonts and retain committed source bytes. Test undo/redo/export/recovery and stale/cancel paths.
- [x] #13: primary-source investigation, reproducible shaping/positioning probe artifacts, design and concrete child issues with testable acceptance criteria.
- [x] Independent review, full regressions, packaged desktop and independent-reader checks; docs/version/package/release; close only implemented issues and link remaining stages.

Verification: 209 frontend tests, 92 native tests (three existing opt-in checks ignored), 12 browser cases, production Windows workflow and two independent PDF readers. Both exports have zero pixel changes outside the permitted edit/note bounds. See ../../verification-v0.9.0.md. #12 is implemented; #13 remains open with #14–#16 tracking production complex-text work.
