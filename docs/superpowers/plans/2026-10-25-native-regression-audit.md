# Native Regression Audit Plan

> **For agentic workers:** use superpowers:executing-plans inline. Preserve existing work; fix demonstrated failures before expanding scope.

**Goal:** Verify that recent form, image, text and export milestones still work together across the ordinary native suite and representative desktop workflows.
**Architecture:** Run all default-feature library and integration targets against the pinned renderer, diagnose failures from their actual inputs, and bind evidence to product hashes. Reuse the current executable if sources remain unchanged; rebuild after product changes.
**Tech Stack:** Rust/Cargo, PDFium, React/Tauri, Windows, independent PyMuPDF/pypdf inspection.
**Spec:** docs/product-quality-loop.md (original quality objective and capability checklist).

## Constraints and decisions

Previous turn: progress; field properties implemented and final source fingerprint revalidated.
No commits, publishing, source-PDF overwrite, cleanup, concurrent Cargo/builds or routine permission questions. Preserve failed logs and prior executables. Do not run ignored physical-printer/dialog/benchmark tests as ordinary tests. No parity claim from these gates.
Ruling: full ordinary native regression before another feature — shared machinery has changed across several milestones — cost: recompilation and a broad integration run.
Ruling: reuse the verified executable only when its complete source/renderer fingerprint matches — avoids unsupported binary claims — cost: every product fix requires another build and desktop run.

## Review focus

Source text edits retaining fields; form edits retaining original action graphs;
paragraph replay after save/reopen; image/page transformations retaining surrounding
content; owned process/source cleanup on failure. Existing tests exercise these
boundaries. Add a regression only for a newly demonstrated uncovered failure.

## Tasks

- [x] Capture product/renderer fingerprint and native test-file hashes under artifacts/native-regression-audit-20261025.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib --tests --no-fail-fast`, one build job, retaining all output and exit code.
- [x] Diagnose any compile/test failure before changing expectations; add failing coverage where needed, fix the root cause, run targeted and then appropriate full gates. Request one final scoped review if product code changes.
- [x] Run representative current-build Windows text/form/save/reopen workflows with independent output verification. Preserve source PDFs and close only owned test processes.
- [x] Refresh the capability checklist, record exact counts/ignored scopes and hashes, and leave the broad goal active.

Completed corrected gate:1,021passed/0failed/11ignored across98targets, exit0;
source474efc6e755ea973fdba41924266a8d547d43a1a94cc5a9fa43582e16b80e588.
Recovery cleanup ownership was independently reproduced and corrected. The original
unprinted word-spacing error's cause remains unproven; both failed baseline tests
pass in the corrected complete run. Preserve that distinction and failed baseline.
Fresh build and three independently verified Windows workflows match this source.
See `artifacts/native-regression-audit-20261025/verification-corrected.json` for
per-suite results, ignored tests, hashes and limits. Overall quality goal is active.
