# Form border precision implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final scoped review.

**Goal:** Preserve fractional PDF widget border widths during native form filling.
**Architecture:** Fifth patch to pinned PDFium; direct behavioral probes and native
integration acceptance before replacing the runtime.
**Tech stack:** C++, Python ctypes/pypdf/MuPDF, Rust, Windows Tauri.
**Spec:** `docs/superpowers/specs/2026-10-26-form-border-precision-design.md`.

## Constraints and review focus

Preserve originals, prior artifacts and old runtime. No commits or publication.
Check missing width defaults, legacy arrays, malformed width fallback, interactive
layout vs saved appearance consistency, and scaled physical widths. Keep unsupported
custom-appearance reflow guarded. One Cargo/build at a time.

## Task 1: Reproduction and separate candidate

- [x] Reproduce 0.5 disappearing and 1.5 becoming 1 against accepted DLL.
- [x] Add `scripts/probe-pdfium-form-borders.py`: generated and filled appearances
  must retain requested numeric border geometry; compare independent raster rings.
- [x] Run the probe against accepted DLL and retain expected failures.
- [x] Add `patches/pdfium/fractional-form-borders.patch` for BAAnnot and PWL widths.
- [x] Update `scripts/pdfium-source-lock.json`, source build recipe and provenance;
  build a separate candidate and pass new plus all existing direct probes.

## Task 2: Native integration and runtime acceptance

- [x] Add focused Rust regression for fill/save/reopen at fractional widths and
  UserUnit 1.7, retaining values and unchanged source.
- [x] Run focused regression against old DLL (fail), then candidate (pass), with
  choice and inheritance compatibility. Other form suites remain part of full run.
- [x] Complete one scoped review with frozen evidence; correct actionable findings.
- [x] Back up installed runtime; install verified candidate. Update fetch checks,
  build fingerprints and notices/docs for five patches.
- [ ] Run full native suite and fresh debug build; verify desktop fill/save/reopen.
- [ ] Record hashes, terminal test evidence and remaining limits in milestone
  verification and execution ledger. Resume border/background controls.
