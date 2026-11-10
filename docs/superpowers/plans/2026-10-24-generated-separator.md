# Generated separator correction plan

> Use superpowers:executing-plans inline, with one final scoped review.

**Goal:** Make valid single-run edits work without treating generated reading spaces as source text.
**Architecture:** Verified source operands supply optional sourceText and mutation text; native snapshots remain selection identity and preservation evidence. Native ownership alone was rejected after probes showed collapsed real spaces.
**Tech stack:** Rust, PDFium, native integration fixtures and Windows desktop acceptance.
**Spec:** docs/superpowers/specs/2026-10-24-generated-separator-design.md.

- [x] Reproduce legacy/complete-source failure and inspect native ownership flags.
- [x] Add exact-text and real-whitespace regressions; implement operand attribution helper.
- [x] Verify no-op, changed edits, conservative mapping fallback, embedded fonts and replay.
- [x] Review and run appropriate native suites and dense performance samples.
- [x] Build, complete Windows workflow and independent saved-PDF verification;
  record acceptance or remaining failures honestly.

Source mappings, real spaces, collisions, Unicode ambiguity, generated internal
gaps and resource growth are the review priorities. Preserve prior work and inputs.

Review P2 corrected: size admission precedes operand copying/decoding, decoded
UTF-8 is bounded, and oversized operands retain their ordinal without blocking a
valid neighboring edit. Two red direct budget tests became green; post-correction
native run passed 212 tests with 4 ignored. Review gaps remain recorded separately.
Final bounded-decoder Windows/independent run passed. Verification:
`artifacts/generated-separator-20261024/checkpoint-final/verification.json`.
