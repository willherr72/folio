# Clipped paragraph fitting implementation plan

> Use superpowers:executing-plans inline, test-driven development, and one final
> scoped reviewer. Standing user authorization replaces routine approval stops.

**Goal:** automatically fit eligible existing paragraphs inside their source clips.
**Architecture:** reuse exact prepared-context containment in anchored search;
retain independent layout/publication checks and conservative obstacle boundaries.
**Tech stack:** current Rust/PDFium/lopdf, existing CLI and Windows harnesses.
**Spec:** docs/superpowers/specs/2026-10-28-clipped-paragraph-fit-design.md

## Constraints and review focus

One locked/offline Cargo process, jobs 1, serial tests; no source/test/build-input
edits during gates. Retain baseline, failed results, packages and original PDFs.
Review horizontal/vertical clips, center/right anchors, physical page transforms,
initial source-baseline padding and impossible/invalid areas. Test each below.

## Task 1: native fitting and publication

Files: src-tauri/src/styled_paragraph_area.rs;
new src-tauri/tests/styled_paragraph_clip_fit.rs.

- [x] Freeze source baseline. Add synthetic clipped fixtures with explicit valid
  layout control, automatic growth, strict returned bounds, saved re-edit and
  untouched-neighbor checks; exercise width, height, center/right and transforms.
- [x] Run `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
  --test styled_paragraph_clip_fit -- --test-threads=1`; verify semantic failures
  before product edits. Add opt-in original Word save/re-edit test.
- [x] Admit anchored bounds only when `prepared.check_context_area(bounds)` passes
  in current fit. Verify existing initial suggestions remain contained; change
  that separate path only if a violation is reproduced.
  Keep `MeasuredPlan::layout` and publication containment unchanged.
- [x] Rerun focused tests including original inputs; independently audit saved
  PDFs. Run area/library and affected styled paragraph/context integration tests.

### Required integration correction discovered by the independent audit

`green-2` has five native passes, but four transformed outputs lose visible text
in MuPDF because clip replay restores CTM before applying W/n. See
artifacts/clipped-paragraph-fit-20261028/clip-replay-diagnosis.md.

- [x] Add a failing transformed clip-publication invariant; retain the complete
  independent 16-output red audit as the cross-reader reproduction.
- [x] Publish clips with compatible transform/consumption order, proving exact
  context bounds and bounded numeric error. Preserve recognition of valid saved
  legacy envelopes; do not replace supported transforms with blanket refusal.
- [x] Verify nested/intersecting clips, repeated envelopes, inverse precision and
  save/reopen fidelity in PDFium, MuPDF and pypdf before accepting the fitting fix.

## Task 2: candidate integration and review

- [x] Build fresh GUI/CLI with source binding. Reuse the 36-workflow original
  producer matrix; require both Word table cases plus all 34 prior successes.
- [x] Freeze scoped source/tests/evidence and request one final review. Resolve
  important findings with one red/green correction pass and affected checks.
- [x] Run owned Windows Word table paragraph fit, Preview/Apply, Cancel, Undo/Redo,
  save/reopen/re-edit; independently check text order, neighboring pixels, links,
  fonts and nonempty tag graphs. Stop only exact owned process identities.
- [x] Reconcile source/binary/output hashes into checkpoint; update roadmap and
  ledger with actual evidence/limits. Keep the full goal active.


Completed source milestone: see artifacts/clipped-paragraph-fit-20261028/checkpoint-final.json. Review gaps and coordinator dispositions are retained in review-response.md. All runtime gates are terminal; optimized packaging and commercial parity are not claimed.
