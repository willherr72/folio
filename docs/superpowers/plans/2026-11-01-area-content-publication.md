# Combined text and drawing publication implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans task-by-task.

**Goal:** Publish real fresh PDFs with selected text and whole touched paths removed, retaining editable neighboring content.

**Architecture:** Existing source-bound text rewrite, bounded intermediate reparse, strict one-command-per-operator drawing adapter, path deletion, fresh publication and retained-glyph verification.

**Tech Stack:** Existing Rust/lopdf/PDFium. Independent Python/PyMuPDF audit tools are acceptance-only.

**Spec:** `docs/superpowers/specs/2026-11-01-area-content-publication-design.md`

## Constraints and review focus

Private integration only; no UI/automation exposure. 64 marks, 1000 pages, 64 MiB input/intermediate/output, existing content/font budgets. Existing text-only behavior and 0.30 package preserved. No commits or cleanup. Review operation-index stability, q/Q/path ownership, no hidden selected operands, unmarked-page preservation and numeric/budget refusal. Tests below cover each.

## Task 1 — Native combined publication

Files: new `src-tauri/src/redaction_area_content.rs`, `redaction_area_content_tests.rs`; private registration in `lib.rs`.

Interface: `remove_planned_content(&lopdf::Document, &[TextRange], &[PageAreaMark]) -> EngineResult<RemovedContentPdf>`. Result contains bytes, removed character count, removed paths and combined omission disclosures. No exposed command or candidate registration.

- [x] Add native fixtures and failing regressions for the spec's positive and refusal cases; compile/run actual `cargo test --lib redaction_area_content -- --test-threads=1` on the stub and preserve failure output.
- [x] Implement strict adapter, immutable two-stage rewrite and final retained-glyph verification; rerun focused suite, then all `redaction_` library tests serially.

## Task 2 — Saved-output acceptance and review

- [x] Emit owned original/expected/actual/second-edit PDFs from the native tests with an explicit artifact output directory. Independently audit complete text, glyph placement, drawings, graph payloads and rendered pages, including corruption controls.
- [x] Freeze delta, source/input and evidence hashes. Request one final scoped review; reproduce and fix demonstrated findings with regression evidence.
- [x] Reconcile terminal gates, preserve original evidence and 0.30 archive, update roadmap and ledger. Keep full area workflow and product goal open.

Self-review: strict text/graph admission remains upstream; the new API cannot claim geometric glyph completeness. No requirement silently drops images, tagged trees, annotations, forms or other unsupported content. Full host/UI validation is explicitly still pending.
