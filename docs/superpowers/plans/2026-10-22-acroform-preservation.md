# AcroForm preservation implementation plan

Use superpowers:executing-plans inline, with one fresh review after implementation.

Goal: preserve interactive form definitions through the existing PDF export path,
as the prerequisite for the full field-filling workflow.
Spec: ../specs/2026-10-22-acroform-preservation-design.md
Stack: existing Rust/lopdf/PDFium; no new dependency.

Keep the existing isolated worktree and all prior work. User's autonomous-work
authorization supersedes skill approval/commit gates; retain local ledger.

- [x] Task 1: Add native regressions proving current export loses AcroForm fields.
  Create src-tauri/tests/acroform_preservation.rs with generated fixtures. Cover
  shared widgets, merged fields, hierarchy, reorder, extraction and duplication.
  Expected RED: output catalog lacks AcroForm. Implement src-tauri/src/acroforms.rs
  bounded source graph transplant and call it before final export publication.
  Interface: preserve(output: &mut lopdf::Document, sources: &[SourceForm]) where
  SourceForm holds original bytes and (source page index, output page index) pairs.
  Expected GREEN: complete field graph, valid links and unchanged appearances.
- [x] Task 2: Add regression coverage for combined sources/default resources,
  malformed graphs, XFA/signed fields, and original/destination immutability.
  Complete font alias/name conflict handling and bounds. Integrate isolated native
  text-copy preservation; explicitly refuse cross-page shared-field text edits.
  Run native export/bookmark/recovery/text-edit suites and frontend production build.
- [x] Task 3: One fresh read-only review; reproduce and fix material findings in
  one pass. Real Windows save/reopen plus MuPDF/pypdf values/appearance checks.
  Update parity record and leave form-filling UI as the next unfinished step.

Review focus: mixed widget/field dictionaries, cross-page linked radio/text fields,
duplicate page ownership, font/name collisions with scripts, copied references
to omitted pages, cycles and large streams. Add these to owning task regressions.

Final acceptance: 134 native tests passed; two physical-printer tests ignored.
Production frontend and Windows desktop builds passed. The exact final executable
saved, extracted, duplicated and reopened generated forms in an isolated profile.
MuPDF/pypdf confirm field values, hierarchy, distinct widget ownership and unchanged
page pixels/source hash. An external fill updates both duplicated shared widgets.
The completed native render was inspected; the owned test process was closed.

One fresh review completed; all material findings fixed with regression coverage.
Deferred minor: separate field and widget AA event keys when splitting merged
dictionaries. Full form entry/appearance regeneration remains the next milestone.
Rulings and limitations are recorded in docs/product-quality-loop.md and the local
artifacts/acroform-preservation-progress.md ledger. No parity or release claim.
