# Inherited Form Appearance Context Implementation Plan

> Use superpowers:executing-plans inline. One final independent scoped review.

**Goal:** fill ordinary fields whose retained appearance uses page resources,
preserving their artwork, font meaning and linked-widget ownership.
**Architecture:** resolve bounded original placement contexts, localize missing
categories without merging category names, and pass that context through the
existing answer transplant and publication proof.
**Tech stack:** Rust/lopdf/PDFium, Python independent readers, packaged CLI and owned desktop.
**Spec:** docs/superpowers/specs/2026-10-27-form-appearance-context-design.md

## Global constraints

Wait for native80037 to terminate; preserve its results/fingerprints first. One
Cargo, jobs1, locked/offline, serial native tests. Existing 64 MiB aggregate
publication budget remains. No source overwrite, package replacement, commits,
release, dependency/API changes or parity claim. Preserve all failed evidence.

## Review focus

- Local category precedence must survive same-name page resources.
- Shared appearances must use each widget's original page context.
- A repeated widget cannot choose a differing page binding arbitrarily.
- Implicit default color spaces and nested resource users must keep their meaning.
- Direct context data must be charged before cloning, with cumulative bounds.

## Task 1: reproduce and bind original appearance context

Files: create `src-tauri/src/form_fill_context.rs` and focused unit coverage;
modify `src-tauri/src/form_fill_publish.rs`, `src-tauri/src/form_answer_transplant.rs`;
extend `src-tauri/tests/form_fill_fidelity.rs` and `src-tauri/tests/page_content_replace.rs`.

Consumes: original Document/normal Stream, NodeBinding.placements, original page
map, and the existing FillAppearanceBudget. Produces: a bounded effective resource
dictionary or context view for the original appearance, retaining source refs.
Keep existing transplant callers/tests compatible through a wrapper if a context
parameter is added. Resolve page inheritance once per relevant page; avoid an
all-pages traversal for every widget.

- [x] Read terminal baseline failure list and preserve exact original files/hashes.
- [x] Add public regressions for the three packaged RED cases, local controls and
  explicit unmarked-program preconditions. Run them RED before implementation.
- [x] Add context tests for local category precedence, parent-page inheritance,
  malformed/cyclic resources, direct-data budget exhaustion and shared placement
  contexts. Include same-name font/XObject collisions and implicit defaults.
- [x] Implement bounded context resolution and integrate it with transplant's
  resource proof and outer Resources construction. Retain original field DR/DA,
  graph identity, answer-region guards and native value witnesses.
- [x] Run focused tests GREEN. Verify original bytes and candidate refusal on
  ambiguous/invalid inputs; do not weaken existing pixel or font assertions.

## Task 2: verify visible preservation and repeated use

Files: extend `src-tauri/tests/form_fill_fidelity.rs` and
`src-tauri/tests/page_content_replace.rs`; add acceptance harness/evidence under
`artifacts/form-appearance-context-20261027/`. Reuse the prepared synthetic CLI
cases with a configurable fresh candidate executable; preserve their baseline.

- [x] Exercise linked siblings with distinct page contexts and same-widget aliases.
  Check hierarchy/defaults and all affected/unaffected placements.
- [x] Save/reopen/refill inherited-font and artwork cases; verify repeated growth
  remains bounded and every retained resource still resolves.
- [x] Independently compare nonblank artwork and outside-answer/other-page pixels.
  Assert exact values in two readers and use a missing-artwork negative control.
- [x] Run all affected native form-fill, automation, OCR and page replacement
  suites; reconcile any separate baseline failures without conflating their causes.
- [x] Build a fresh local candidate and run CLI plus owned desktop fill/save/reopen;
  preserve accepted portable0.22 and original PDFs, stop only the owned process.

## Task 3: review and checkpoint

Files: `artifacts/form-appearance-context-review-20261027/review.md`, response and
checkpoint; update `docs/product-quality-loop.md` and the active progress ledger.

- [x] Request the sole final scoped review after corrected runtime verification.
- [x] Resolve demonstrated findings with appropriate regressions and affected reruns.
- [x] Reconcile the full ordinary native result/fingerprints against all changes,
  and record exact acceptance plus remaining alias/producer/performance limits.
- [x] Leave the overall parity goal active while material gaps remain.

Plan self-check: the five review risks are covered in Tasks1/2; all spec acceptance
items map to native tests, independent output checks, fresh runtime or final review.
Execution completed; corrected evidence and retained limitations are recorded in artifacts/form-appearance-context-20261027/checkpoint-corrected.json and the review response. Full ordinary regression on the corrected source is a separate next gate, not part of the affected-suite pass count.
