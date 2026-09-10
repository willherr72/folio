# Compatibility corpus implementation plan

Goal: reproducible redistributable PDFs and measured native compatibility/performance evidence for issue 6.

Architecture: Python development-only generation produces fixed small fixtures and optional bulk artifacts with SHA-256 manifests. An ignored Rust integration test exercises the real Pdfium worker and writes per-operation metrics; a PowerShell runner records machine data and process memory. Existing native types/runtime stay untouched.

- [x] Add generator contract tests: deterministic bytes, manifest integrity, image-only semantics, font embedding, rotation/crop and expected text.
- [x] Implement fixed-seed fixtures; bundle unmodified DejaVu Serif and its redistribution license/provenance.
- [x] Add real engine corpus integration tests for open/render/extract/reorder/edit/export/reopen and source-byte identity.
- [x] Generate 300-page text and 40-page 200dpi scan workloads outside git; run timed release checks with machine/memory sampling.
- [x] Record commands, measured results, limitations and baseline guidance. Do not enforce arbitrary performance thresholds.

Scope: scripts/corpus*, docs/corpus*, tests/corpus* and tests/fixtures/corpus*, src-tauri/tests/corpus.rs only. No runtime dependencies, UI changes, native engine changes, commits or physical printing.

Completed: six final native measured runs, packaged bulk cache/scroll/memory run, six actual copy cases, four mixed geometry checks, natural close/idle retention samples. No product changes or physical printing.
