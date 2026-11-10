# Bounded Composition Parse Reuse Plan

> **For agentic workers:** Use superpowers:executing-plans inline. One final scoped correctness review.

**Goal:** Remove repeated parsing from measured single-source exports without changing fidelity or authorization.

**Architecture:** Private optional OnceCell in SourceForm, immutable Cow access; native comments borrow SourceForm. Composition enables bounded reuse only for one source and releases it before serialization.

**Tech Stack:** Existing Rust, lopdf, PDFium, native integration harness and MuPDF verification.

**Spec:** docs/superpowers/specs/2026-10-25-large-export-design.md

## Constraints and review focus

- Admission: one source, at most 8 MiB bytes, at most 100,000 parsed objects; strict parser and 64 MiB decompression limit unchanged.
- Never clone the cached document or mutate it; preserve every validation and mapping/budget check.
- Preserve duplicate-page ownership, combined-source fallback, form actions/fonts, native annotations and links.
- No global cache, dependencies, original overwrites, commits or releases.
- Failed parsing must not poison retry behavior; retained documents must be dropped before final output serialization/protection.

## Implementation and verification

- [x] Measure baseline export stages and retain the unmodified source snapshot; remove temporary instrumentation.
- [x] Add red regression coverage for immutable bounded reuse and malformed input.
- [x] Implement SourceForm constructor/cache and read-only consumers; update native comment source binding and callers.
- [x] Verify focused regression, preservation/resource/security native suites, and compare repeated synthetic timings and memory.
- [x] Freeze scope and obtain one independent correctness review; correct demonstrated findings.
- [x] Build a fresh checkpoint, independently verify saved output and record remaining desktop delay limitation.

Evidence: artifacts/large-export-20261025/checkpoint/verification.json. 346 native passes,
five ignored diagnostics; fresh TypeScript/build, packaged Windows save and independent
text/dimension/raster verification. The native speedup costs about 41 MB additional
sampled peak private memory at 10,000 pages. Admission bounds are not total retained
heap caps; compressed-object-stream and long-lived memory coverage remains open.
The original >60 s desktop delay was not reproduced or attributed. One follow-up
packaged save took 7.58 s including automation; the IPC timing hook captured no events.
