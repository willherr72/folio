# Accented equivalent encoding implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans inline; one fresh final review.

**Goal:** admit literal redundant accented WinAnsi/MacRoman glyph assignments without weakening font or appearance proofs.
**Architecture:** share existing pinned encoding names at text_edit scope; use exact literal table lookup for each Differences slot. Existing callers retain original dictionaries and full font validation.
**Tech Stack:** existing Rust/lopdf/PDFium and authored Python/pypdf/MuPDF verification.
**Spec:** docs/superpowers/specs/2026-11-15-accented-equivalent-encoding-design.md

## Global constraints
No Git mutations, new dependencies, version/package/launcher changes, private inputs or clipboard operations. Preserve previous artifacts. Single Cargo job and serial native/build/UI; existing D: caches. Frontend min/max two workers with default5000ms. User-authorized inline execution; no implementer agents.

## Review focus
- High-byte names differ by base: never accept eacute at WinAnsi byte142 merely because MacRoman maps it there.
- Literal aliases uni00E9 and alternative names must not become nominal glyph authority.
- Resource equivalence must not bypass source byte roundtrip, malformed ToUnicode or font width checks.
- Shared CFF tables must retain exact original data/visibility and behavior.
- Repeated saved edits must retain source encoding and unselected text in nested/shared occurrences.

### Task1: Exact-name recognizer and native regression
- [x] Add failing unit tests for high-byte direct/indirect names, wrong base and aliases.
- [x] Add authored native accented source fixtures and route/save/re-edit assertions.
- [x] Run focused RED without changing production behavior.
- [x] Move table module ownership to text_edit; reuse in CFF and equivalence. Retain per-run ambiguous-byte guards separately from literal resource equivalence.
- [x] Run focused GREEN and complete library correctness plus affected optimized integrations.

### Task2: Saved-output and compiled acceptance
- [x] Independently audit authored saved outputs against nominal-encoding references; reject actual metric/text/paint corruptions.
- [x] Run normal compiled CLI original-font edit/save/reopen and preservation checks, fresh frontend and appropriate normal build.
- [x] Build the normal optimized desktop candidate after final native/CLI acceptance; run owned Windows edit/Undo/Redo/save/reopen/restore and paragraph workflows, then independently audit saved outputs.
- [x] Request one fresh scoped final review of the complete change and desktop acceptance, resolve Important findings with observed RED/GREEN and appropriate reruns.
- [x] Record completion, source/evidence hashes and explicit limits. Broad goal remains active.
