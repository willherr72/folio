# PDF link preservation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline, with test-first changes and one final scoped reviewer.

**Goal:** Keep native PDF links functional through Folio save and page operations.

**Architecture:** A child module of bookmarks reuses its bounded Import/PdfValue/remap/write machinery. Reconstruct only Link navigation metadata from original source bytes after page import and form repair, before overlay/crop finalization. Keep source/output link-slot correspondence explicit.

**Tech Stack:** Rust, lopdf, PDFium, existing composition pipeline; pypdf/MuPDF independent oracle.

**Spec:** docs/superpowers/specs/2026-10-24-pdf-link-preservation-design.md

## Global constraints

- No original-file writes, commits, release or prior-checkpoint cleanup.
- No action execution or visible content changes.
- Reuse existing 4 MiB/100,000-value/32-depth bookmark metadata bounds, and bound reconstructed metadata across all output copies before writing.
- Self-links target the current occurrence; other local targets use the first surviving source occurrence; omitted local targets are disabled.
- Replacement source lineage, shared-field version mapping and the replacement UI remain pending.

## Review focus

- Identical named destinations in unrelated sources must never cross-bind; test combined sources.
- Duplicate copies must not share mutable annotation identity; test self-links and repeated outputs.
- Omitted destinations must not become a different live page; test extract and chained actions.
- Link matching must detect missing/reordered native annotations; test malformed geometry and mapping rejection.
- Large/cyclic action graphs must be rejected before output amplification; test limits and shared action references.

### Task 1: Repair navigation metadata in the composition pipeline

**Files:** create src-tauri/src/link_navigation.rs, src-tauri/tests/link_preservation.rs and scripts/verify-pdf-links.py; modify src-tauri/src/bookmarks.rs and engine.rs.

**Interfaces:** `bookmarks::links::prepare(sources: &[acroforms::SourceForm<'_>]) -> EngineResult<Plan>` followed by `Plan::apply(output: &mut lopdf::Document) -> EngineResult<bool>`; Import constructor shared internally by outline reading and links. Preparation bounds source and duplicate output metadata before native copying; application checks imported link identity before reconstruction.

- [x] Write native `save_retains_direct_and_named_internal_links` and watch RED against current composition.
- [x] Implement bounded source navigation import, page mapping, link correspondence and output reconstruction.
- [x] Run GREEN and add the spec's reorder/duplicate/combine/extract/action/error/protection controls.
- [x] Run independent readers, affected native suites and a fresh scoped review; correct important findings once with RED→GREEN.
- [x] Record the verified checkpoint and remaining replacement work. No commit under standing workspace policy.

Completed evidence: `artifacts/pdf-link-preservation-20261024/checkpoint-final/verification.json`.
Review disposition: `artifacts/pdf-link-preservation-20261024/review-disposition.md`.
256 native passes / 3 ignored; seven independent documents / sixteen pages;
two actual Windows save/reopen cycles and four independently checked desktop pages.
Unsupported action types reject explicitly; source-version replacement integration remains pending.
