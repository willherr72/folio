# Dictionary Number Fidelity Implementation Plan

> Execute inline with superpowers:executing-plans under the existing phase3
> review boundary. Preserve all inherited changes and accepted packages.

**Goal:** Validate original dictionary number spellings before redaction.

**Architecture:** Optional bounded-loader numeric predicate and shared redaction
loader; bounded final graph numeric validation. No persistent token sidecar.

**Tech stack:** Rust/lopdf, Python owned PDF/native-reader audit.

- [x] Add opt-in loader API and failing native dictionary/lifecycle regressions.
- [x] Implement parser callbacks, redaction load integration and final numeric
  graph validation; verify focused red/green and lifecycle behavior.
- [x] Audit actual page-box interpretation, run serial native regressions and
  compare prior artifacts. Record remaining compatibility and placement gaps.
