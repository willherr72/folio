# Source-preserving single-run font substitution implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. The user opted
> out of per-task delegation and routine confirmations; one final scoped review.

**Goal:** Explicit single-run font changes preserve source positioning, clipping,
tags and neighboring content through desktop and CLI/MCP editing.

**Architecture:** Source-bound private substitution plans replace only the selected
show and required local font/cursor state. Native context and complete-source
publication verify the planned operation sequence and existing fidelity invariants.
Automation reuses bounded FontInputs from the completed paragraph milestone.

**Tech Stack:** Rust, lopdf, locked PDFium, existing TypeScript desktop, Python
independent PDF audits. No new dependencies or renderer patch.

**Spec:** docs/superpowers/specs/2026-10-28-single-run-font-context-design.md

## Global constraints

- Preserve source PDFs, accepted portable package, existing dirty worktree; no commits.
- One Cargo process, locked/offline, jobs1, serial tests; never mutate product/test
  inputs while a gate runs. Capture before/after fingerprints and terminal results.
- Single-line Latin/native restrictions and4096UTF-8bytes; content16MiB/100,000ops;
  nesting64; numeric adjustments finite and at most1,000,000; document512MiB.
- Explicit fonts16paths/16MiBeach/64MiBtotal including duplicates; launch-only MCP
  authority; no silent fallback or arbitrary tool-time paths.
- Exact baseline/size/color and unselected glyph/pixel/graph fidelity; no reflow.

## Review focus

1. Identity-H spaces lose effective single-byte Tw: Task1 mixed spacing tests.
2. Later text depends on the changed cursor: Task1 dependent-show and line-matrix tests.
3. Repeated font changes accumulate stale state/resources: Task1 repeated A/B/samefont tests.
4. Relaxed context proof admits changed clips/tags with equal pixels: Task1 tamper negatives.
5. Explicit font request takes no-op shortcut or registry-only authority: Task2 controls.

### Task1: Native source-bound substitution and context publication

**Files:** create src-tauri/src/text_substitution.rs and focused tests; modify
text_edit.rs, text_context_edit.rs, occurrence_text_edit.rs as needed; extend
tests/word_text_context.rs. Reuse text_show/positioned_text/verified advances.

**Interfaces:** private `text_substitution::Plan::prepare(bytes, selected, runs,
replacement, asset)->EngineResult<Plan>`; `Plan::into_bytes()->Vec<u8>`;
source/candidate operation accessors for private PublicationProof binding. Derive
exact argument types from existing RunSnapshot/FontAsset. Plan fields private.
Context authorization must validate source equality and selected ordinal before
accepting its candidate operation sequence. Outer publication uses the same plan.

- [x] Add native RED tests for explicit font changes in clipped/tagged Tj and
  positioned TJ. Assert actual replacement, baseline/size/color, graph and neighbors.
- [x] Add dependent cursor, quotes, Tc/Tw and repeated-font cases plus clip/tag
  tamper, source mismatch, glyph/overlap/permission refusal controls. Preserve RED.
- [x] Implement bounded source/native mapping, selected-show reencoding, state
  preservation, effective word spacing, cursor correction and font/state reuse.
- [x] Bind private expected candidate operations into context and complete-source
  publication. Preserve all existing source/candidate/native fidelity checks.
- [x] Use source-specific discovery and source-preserving no-op proof for the
  substitution path. Reopen replacement must itself remain editable.
- [x] Run focused tests then library/text_edit/word_text_context/positioned/group/
  occurrence/form/paragraph affected gates serially, with immutable fingerprints.

### Task2: Explicit single-run automation fonts

**Files:** automation/text_preview.rs, automation.rs, cli.rs,
mcp/protocol.rs and worker.rs, tests/text_automation.rs, tests/mcp.rs,
docs/automation.md. Update only related constructor fixtures.

**Interfaces:** TextEdit optional `fontId` from FontInputs catalog; preview entry
receives `&FontInputs`; metadata records the used compact FontChoice. Paragraph
behavior remains unchanged. MCP worker snapshots only referenced authorized fonts.

- [x] RED: CLI/MCP page/nested explicit font workflows and unauthorized-ID/path
  refusal. Existing old request/bundle compatibility and no-font identity controls.
- [x] Reuse bounded authorization/registration/worker lifetime; extend CLI --font
  admission to preview-text and MCP schema. Explicit font disables no-op shortcut.
- [x] Validate actual native eligibility, old metadata defaults, font deletion,
  Apply/Close/reinspect/re-edit/Save in both MCP protocols and protected refusals.
- [x] Run affected library/CLI/MCP/text/paragraph/nested/form gates and fingerprints.

### Task3: Fresh user workflows, independent fidelity, final review

**Files:** artifacts/single-run-font-context-20261028 scripts/results, this plan,
execution ledger, docs/product-quality-loop.md.

- [x] Build isolated fresh desktop/CLI candidate from completed native gate.
- [x] Run tagged/untagged Word growth and accented edits plus overlap refusal;
  independent expected text, baseline/style, outside glyph/pixels, second-page and
  nonempty graph checks. Retain negative controls and source hashes.
- [x] Desktop Preview/Cancel/Apply/Undo/Redo/save/reopen, explicit-font CLI and both
  MCP protocols; saved re-edit without fonts and original no-op/protection checks.
- [x] Obtain one final scoped review of frozen delta and evidence. Resolve concrete
  findings inline through RED/GREEN, retain limitations and all failed evidence.
- [x] Verify and record local checkpoint. Goal remains active, no release/parity claim.

No task is complete from planning alone. Implementation may refine private helper
signatures when source evidence warrants; record deviations with their tests.
