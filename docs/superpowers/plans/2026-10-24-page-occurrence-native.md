# Page occurrence replacement implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline with TDD and one final fresh scoped reviewer.

**Goal:** Replace one duplicated workspace page while keeping one native source and shared interactions.

**Architecture:** Append one native page copy to the original document only when the selected native index repeats. Repair the copied page and shared interaction graph with existing bounded helpers, then reuse immutable page-content replacement. Return explicit occurrence mapping with the new document.

**Tech Stack:** Rust, lopdf, PDFium, existing native policy/recovery and independent pypdf/MuPDF checks.

**Spec:** docs/superpowers/specs/2026-10-24-page-occurrence-replacement-design.md

## Global constraints

- No source-file writes, commits, external release, or old-artifact cleanup.
- At most 100,000 submitted occurrences and 100,000 native source pages after append.
- Append at most one native page; retain existing indices, original catalog and page frames.
- Native edit-content and incoming copy policy apply; duplicate preparation also requires assemble permission.
- Retain original protection identity/credentials; duplicate provenance requires assemble rights.
- No intermediate registered source; abandon/cancel paths must release unpublished assets.
- Preview/apply UI remains the next milestone, not a completed capability.

## Review focus

- First versus later selected duplicates must preserve first-target links and shared field identity.
- Renumbered copied objects must not capture original catalog/page/resource references.
- Non-unit/cropped/rotated pages must retain native frames and independent workspace crop reset semantics.
- Protected recovery must not admit repeated provenance without native assemble permission.
- Candidate failures/cancelled replies must leave no registered source/assets or altered input files.

### Task 1: Native occurrence-aware source replacement

**Files:** create `src-tauri/src/page_occurrence_replace.rs` and `src-tauri/tests/page_occurrence_replace.rs`; modify `engine.rs`, `page_content_replace.rs`, `document_policy.rs`, `types.rs`, `lib.rs`; add independent verification under `scripts` and task artifacts.

**Interfaces:** public `PageContentReplacement { document: DocumentInfo, page_indices: Vec<usize> }`; `PdfEngine::replace_page_content_occurrence(&self, destination_id: &str, occurrences: &[usize], selected_occurrence: usize, incoming_id: &str, incoming_page: usize) -> EngineResult<PageContentReplacement>`. Worker request mirrors those fields. `SourcePolicy::duplicated_page(index)` retains original origin/protection and appends one page/baseline after native permission checks.

- [x] Write `replaces_one_duplicate_retaining_shared_fields_and_links` and watch failure against the absent native API.
- [x] Implement bounded mapping, original-catalog page append, resource/comment/form/link repair and existing replacement reuse; register only the final candidate.
- [x] Run GREEN and add first/middle/last, invalid, geometry, catalog, editability, repeated-save and source-preservation controls.
- [x] Add protected allowed/denied save/recovery and abandoned-reply tests; verify policy invariants and lifetime cleanup.
- [x] Run independent readers and affected library/page/link/form/comment/crop/protection/recovery suites; obtain one scoped review and correct important findings with RED→GREEN.
- [x] Record exact evidence/fingerprints and update the quality ledger. Keep UI and broad parity goal pending.
