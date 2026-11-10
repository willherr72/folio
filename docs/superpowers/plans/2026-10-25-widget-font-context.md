# Widget Font Context Implementation Plan

> Execute inline with superpowers:executing-plans, TDD and independent review.

Goal: edit page text beside local-font form widgets without losing appearances.
Architecture: retain independent source font contexts in the existing immutable
page-content graft path. Rust/lopdf/PDFium; no new dependencies or frontend API.
Spec: docs/superpowers/specs/2026-10-25-widget-font-context-design.md.

- [x] Add native regression cases to page_content_replace.rs and occurrence
  tests proving local AP and inherited field resources, conflicting aliases,
  pixels, later fills and save/reopen. Run and retain failures.
- [x] Update default_appearance_context in page_content_replace.rs to check
  independent resources without requiring global DR; move button/signature
  early return before text-font requirements. Bound DA before cloning/parsing.
  Retain rejection of malformed and truly page-dependent text fonts.
- [x] Run related library, replacement, occurrence and form suites; independently
  review the scoped delta, fix demonstrated defects and build a fingerprinted exe.
- [x] Run the original PyMuPDF desktop fixture through text edit, fill and
  save/reopen; compare independent field pixels and source hashes, stop only
  owned processes and record evidence/remaining gaps in the quality ledger.

Review focus: namespace conflicts, shared parent fields, indirect resources,
malformed cycles/large DA, and annotation state dictionaries. Existing limits,
source rights and immutable ownership remain required. No commits/releases.
Evidence: artifacts/widget-font-context-20261025.

Completed: artifacts/widget-font-context-20261025/checkpoint/verification.json.
All added native regressions live in page_content_replace.rs and exercise occurrence APIs; the existing occurrence suite was rerun unchanged.
