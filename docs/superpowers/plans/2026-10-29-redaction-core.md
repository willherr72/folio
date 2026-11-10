# Structural Text Redaction Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build and independently verify the private input-to-output structural text-removal kernel described as milestone A.

**Architecture:** Canonicalize admitted content with pre-allocation lexical admission, bind whole BT blocks to the canonical document, erase selected shows while retaining state, and serialize a fresh minimal object graph. Do not expose a redaction command until later geometry/security/UI milestones.

**Tech Stack:** Existing Rust, lopdf, SHA-256; existing MuPDF/pypdf for independent artifact checks.

**Spec:** docs/superpowers/specs/2026-10-29-redaction-design.md

## Global constraints
No new dependencies, public command, source-file overwrite, package overwrite or commits. Work inline in the existing isolated worktree. Preserve inherited changes. Cargo --locked --offline, one job and one Cargo process. Limits: 1000 pages; 100,000 objects and operations; 500,000 lexical tokens; depth 64; 16 top-level operands; 4096 array elements/targets; 8 MiB decoded per page; 64 MiB cumulative decoded/output; 1 MiB text; finite magnitudes at most 1,000,000. Page dimensions 1..20000 pt, zero-origin boxes, Rotate 0/UserUnit 1. Twelve standard Latin Type1 fonts, default/WinAnsi, printable ASCII operand strings only. Implicit default color spaces refuse.

## Review focus
- Double-quote text state survives deletion and still controls later blocks: task 2.
- Implicit/default resources inherited from a parent cannot change unaffected paint when dropped: task 1.
- Source aliases, comments and unreachable graphs cannot retain removed strings in output: task 2.
- Malformed tails, inline images, huge operands and nested syntax refuse before generic decoder allocation: task 1.
- Shared page/content aliases, stale targets and metadata-only changes cannot cause cross-page removal or reattach old content: task 2.

## Task 1: bounded canonical inspection
Files: create src-tauri/src/redaction_content.rs, src-tauri/src/redaction_core.rs, src-tauri/src/redaction_core_tests.rs; modify src-tauri/src/lib.rs for private modules only.

Interfaces: `inspect_standard_text(&lopdf::Document) -> EngineResult<Vec<TextBlock>>`; TextBlock has page_index:usize, block_index:usize, text:String, selection_id:String. Internal `ContentBudget` tracks lexical/operation/text totals. `canonicalize(data, resolve_font, budget) -> EngineResult<Program>` normalizes quote shows into state/movement plus Tj, records each BT block's show positions, and retains typed safe operations. Page preparation validates the complete page tree and effective resources before returning any selection.

- [x] Add failing tests for basic inspected selection, inherited standard resources, malformed/unsupported operators, default color spaces, parser limits and finite/state/operand shapes. Wire only minimal missing-behavior stubs for the red run.
- [x] Run cargo test --lib redaction_ -- --nocapture, preserve failing evidence.
- [x] Implement lexical admission and strict decoding, font canonicalization, tree/resource validation, canonical document digest and inspected block IDs.
- [x] Run focused tests; source fingerprint/logs bind each terminal result.

## Task 2: real removal and fresh output
Interface: `remove_standard_text(&Document, selection_ids:&[String]) -> EngineResult<RedactedTextPdf>`; result contains bytes:Vec<u8>, removed_blocks:usize and omitted_metadata:Vec<&'static str>. Resolve targets against a newly prepared canonical document; do not accept client-supplied output content. Unknown/duplicate/empty selections refuse. Source immutability and output maximum are enforced.

- [x] Add failing end-to-end tests for real Tj/TJ/single/double-quote removal, state-preserving following text, multiple selections and untouched repeated text on other pages.
- [x] Add secrets in Info/XMP/comments/unreachable streams/unused resource aliases; require all copies absent from serialized and decoded output while remaining visible text and required numeric operators persist.
- [x] Implement selected-show filtering and minimal fresh Catalog/Pages/Page/Contents/Font graph construction with generated aliases. Preserve text state, page order and remaining numeric paint; never clone source objects.
- [x] Verify selected output contains no removed strings in any emitted object/stream and input serialization is unchanged. Test stale/duplicate bindings, shared source streams and reference/page cycles.

## Task 3: independent acceptance and review
Files: artifacts/redaction-core-20261029 helpers and evidence; execution ledger and quality roadmap.

- [x] Export deterministic synthetic before/after PDFs from a controlled native test path, including quote-state, shared-content occurrence and hidden-copy cases.
- [x] Use MuPDF and pypdf to verify exact remaining text, every unaffected glyph position/style, decoded-object absence, no original stream bytes and raster invariance outside removed text ink; include a black-overlay-only negative control.
- [x] Run the focused native module suite and appropriate affected library checks; no frontend change needs a new frontend suite.
- [x] Request one scoped final implementation review, fix concrete findings locally, and reconcile exact evidence/limits. Record the kernel as private and incomplete toward the full redaction workflow; leave the broader goal active.

## Final review fix pass
Two reproduced findings are corrected locally: complete numeric-token grammar before decode, and opt-in strict bounded Flate decoding. Additional file: src-tauri/vendor/lopdf-0.45.0/src/object.rs. Baseline copy/hash retained in artifacts/redaction-core-20261029/baseline. Review-red:14 pass/3 fail; review-green and final library verification pending. No second reviewer, public redaction command, package rebuild or unrelated refactor.

Final verification: review-green-2 18/18; library-2 603 passed,4 existing ignored,0 failures,fingerprint0. Four saved outputs and five negative controls verified; all frozen review hashes retained. See checkpoint.json and review/disposition.md. Milestone A only; broader goal active.
