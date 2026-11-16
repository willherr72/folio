# Source accent clusters implementation plan

> **For agentic workers:** Use superpowers:executing-plans. User authorized autonomous implementation; preserve the current dirty worktree, all prior artifacts and selected packages.

**Goal:** Edit already-proved physical accent clusters without normalizing text or introducing shaping.

**Architecture:** Extend source-sequence admission and route predicates around the existing physical glyph codebook. Reuse bound atom encoding and modern styled paragraph measurement unchanged wherever possible.

**Tech Stack:** Rust, existing unicode-normalization/ttf-parser/PDFium/lopdf, public DejaVu fontTools witnesses, existing Windows harness.

**Spec:** docs/superpowers/specs/2026-11-15-source-accent-clusters-design.md

## Global constraints

No global scalar-policy relaxation; exact original source operands remain mandatory. No new dependency, private input, clipboard probe, package/launcher promotion, Git mutation or artifact deletion. Native/frontend/build/runtime serial. One Cargo job with established D: cache; frontend two workers/default timeout.

## Review focus

- Contradictory aliases must not relabel an ordinary glyph as an accent cluster.
- Partial replacement and style boundaries must not split a physical source glyph.
- Explicit substitute-font requests must retain their stricter character validation.
- Shared occurrences and following text must preserve exact pixels and codes.
- Unicode normalization equivalence must never silently rewrite source/replacement text.

### Task 1: Proved source clusters and native editing

**Files:** embedded_font.rs, embedded_cff.rs, text_edit.rs, source_text.rs, text_group.rs, form_text.rs; new src-tauri/tests/source_accent_clusters.rs and public tests/fixtures/source-accent-clusters/v1.

**Interfaces:** `source_sequence(text: &str) -> bool` classifies bounded candidates without granting authority. `VerifiedFont::allows_source_text(&self,text: &str) -> bool` retains unshaped scalar admission and otherwise requires the verified codebook to encode the exact bounded text. Route guards consume it only with original-font proof; explicit substitutes stay strict.

- [x] Generate licensed source witnesses and native integration tests for no-op/growth/removal, grouped/shared edits, styled reflow and save/reopen. Add classifier/refusal and exact-text boundary tests. Expected initial failure: source cluster is unsupported.
- [x] Run RED and retain logs. Implement sequence classifier and proof-aware route guards; keep all downstream code/GID/alias/width/ink/operand checks. Run GREEN.
- [x] Independently audit retained saved PDFs against source-authored text/codes/font widths/programs and whole-page renders; include real negative corruptions.

### Task 2: Qualification and final review

**Interfaces:** Uses Task 1's exact current source and retained outputs, without mutating fixtures during gates.

- [x] Run complete library correctness and complete affected ordinary optimized integrations serially. Retain profile meanings and ignored counts.
- [x] Run fresh full frontend, normal optimized diagnostic build and actual compiled CLI/MCP/owned Windows editing, save/reopen, with independent output checks.
- [x] One fresh scoped final review. Classify findings by user effect; Important/Critical findings get one RED→GREEN correction pass, no re-review. Record limitations/minors and exact completion evidence; broad goal remains active.

Completion: artifacts/source-accent-clusters-20261115/completion.json. Both Important review findings corrected with RED→GREEN evidence in one pass; no re-review. Broad goal and general producer/shaping/release gates remain open.
