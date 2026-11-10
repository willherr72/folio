# Bounded initial PDF loading

Spec: docs/superpowers/specs/2026-10-29-redaction-loader-budget-design.md
Implement inline in the existing isolated worktree. Preserve unrelated work and evidence; no commits, cleanup or public exposure.

## Task 1: Scoped aggregate loader budgets
Create vendored load_limits.rs and export ResourceLimits plus opt-in Document loader. Add Folio library tests in redaction_loader_tests.rs using modern and classic PDFs. First run aggregate decoded/node/payload/input/xref refusal tests against an unbounded API stub; expect failures. Implement scoped monotonic counters, sticky exhaustion, nested guard/indirect depth protection, parser preallocation charging, sequential bounded object loading and object-stream parsing, stage-by-stage decoded accounting and checked predictor scratch allocation. Ordinary paths keep defaults.
Interfaces: reader/parser/decoder use internal scoped functions; new API is synchronous and rejects encryption before raw encrypted loading. Redaction will consume the public vendored loader API only. Tests use small budgets and saved valid compressed documents.
Expected: exact boundary successes, refusal above budget, no swallowed failures, scope/thread isolation and ordinary recovery behavior preserved.

## Task 2: Redaction integration and provenance
Switch source-owned redaction parsing to the bounded loader; add vendored lopdf Rust/TOML files to build fingerprints. Test compressed and incremental redaction inputs, source immutability on budget refusal, prior source mappings, saved mark outputs and multi-page preview byte equivalence.
Expected: accepted ordinary/modern PDFs continue through the same selection/removal kernel; refusal leaves source and preview registrations unchanged.

## Task 3: Regression and independent review
Run focused loader/redaction checks, relevant existing loader/recovery tests, full Folio library gate and applicable vendored checks. Freeze precise source/evidence and obtain one final fresh review. Correct consequential findings locally with RED/GREEN; no re-review. Record bounds as logical allocation/work limits with remaining process-memory/native-parser/UI limitations.
Expected: complete passing logs, fingerprints including dependency changes, persistent checkpoint and next host/composition integration steps. Overall goal stays active.

## Review focus
Preallocation versus after-the-fact checks; swallowed nom/decode errors; per-filter and unfiltered accounting; literal nested copies; dictionary/header/stream roots; duplicate/overlapping ObjStm offsets; compressed Length cycles; PNG attacker dimensions; xref chain/recovery; Rayon/TLS escapes and RAII restoration; ordinary reader compatibility and provenance.
