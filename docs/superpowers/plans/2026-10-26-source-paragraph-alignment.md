# Source Paragraph Alignment Implementation Plan

> Use superpowers:executing-plans inline. One final scoped review; corrections
> inline without another review round. No commits, publishing or original edits.

**Goal:** Preserve inferred alignment and its horizontal anchor during ordinary
existing-paragraph editing and area fitting.
**Architecture:** Shared checked row measurements, conservative fresh inference,
alignment-aware frame construction and bounded width/height fitting.
**Tech Stack:** Rust/PDFium/lopdf, existing TypeScript editor, Windows harness.
**Spec:** docs/superpowers/specs/2026-10-26-source-paragraph-alignment-design.md.

## Constraints and review focus

- Do not mutate product sources while the source-order gate remains active.
- Saved replay and explicit user settings remain authoritative.
- Physical measurements must agree across crop, rotation and UserUnit.
- Trailing whitespace, mixed style metrics and ink/highlight overhang must not
  shift inferred anchors or fabricate alignment on ambiguous source lines.
- Center/Right fitting must check every expanded side and retain safe endpoints
  when width changes the first-line ascent. Keep bounded allocation/work.
- Publication keeps original glyph/font/neighbor/pixel and source ownership gates.

## Task 1: Fresh alignment and initial frame

Files: src-tauri/src/text_paragraph.rs; new focused source-alignment helper if
needed; src-tauri/src/styled_paragraph_area.rs;
new src-tauri/tests/styled_paragraph_source_alignment.rs.

- [x] Capture the current source baseline and source-order terminal evidence.
- [x] Write/run native failing cases for fresh styled Center/Right and mixed
      pieces, with direct source coordinates and ambiguous/Left controls.
- [x] Share row extent construction using each run's verified metrics; return
      a conservative alignment proposal and checked source anchor.
- [x] Apply frame padding around that anchor and retain initial source baseline.
- [x] Derive mixed-row spacing from adjacent row maximum ems; reproduce and fix
      unchanged three-row text widening into two rows under a first-font ratio.
- [x] Cover suggestions, highlights, saved replay, trailing whitespace and
      transformed physical pages. Verify independent saved fragment positions.

## Task 2: Alignment-aware fitting

Files: src-tauri/src/styled_paragraph_area.rs; its focused unit tests;
src-tauri/tests/styled_paragraph_source_alignment.rs.

- [x] Reproduce centered/right-aligned anchor drift when widening is necessary.
- [x] Express bounded fitting trials in width/height with explicit anchors so
      search parameters stay monotone even when a rectangle's right edge is fixed.
- [x] Preserve current Left/Justify behavior, fresh baseline anchoring and the
      safe narrower width when a combined height extension is required.
- [x] Test obstacles on both sides, page limits, mixed first-line heights,
      impossible fits and no-change cases; verify fit leaves inputs untouched.

## Task 3: Review and packaged acceptance

- [x] Run affected uniform/styled alignment, area/fit/baseline, highlight/memory,
      source-order, physical-page and occurrence tests; preserve exact evidence.
- [x] One final scoped review with frozen before/after hashes; reproduce/correct
      demonstrated findings inline and rerun only affected checks.
- [x] Fresh desktop build and quiz highlighted-instructions workflow; independent
      center/font/color/highlight/order/baseline/pixel audit and source hashes.
- [x] Update plan, quality record and execution ledger. Keep the broad goal active.


## Corrective scope and evidence

The one final review found edge-mark horizontal drift; its native RED moved a
centered row from300 to302pt. Corrected inline with version4 text alignment and
explicit horizontal insets, preserving old versions1/2/3. Insets survive the
removal of the final mark. Native ink/mark ownership, containment, exact replay,
and raster gates remain unchanged. Added widest-row and fitted-width cases.

The affected run also exposed an initial single-line paragraph becoming two lines;
source-width descent extension now requires retaining the source row count.
A boundary RED reproduced256 valid rows being rejected during duplicate padded
wrapping; palette validation is separate from row creation. Signed spare width
keeps new Center/Right anchors exact at the existing wrapping tolerance.

Focused corrected run:15 native passes. Independent PDFs:8 edge-highlight and8
physical-page cases pass. Final EXE and both actual quiz workflows pass; see
artifacts/source-paragraph-alignment-corrected-20261026/verification-progress.json.
The active broad gate has two outdated test expectations requiring highlight-driven
horizontal drift. Assertions now require stable anchors plus retained reservation;
four-test rerun and terminal gate/fingerprint reconciliation remain pending.

Final acceptance reconciled: 46 library and77 unique integration passes,1 ignored.
Original broad batch retains its two obsolete assertions as failed evidence; corrected
four-test target passes with unchanged product fingerprint. Both final-build quiz
workflows and16 independent PDF cases passed. Final record: corrected artifact
verification.json. Tight source leading remains the next bounded fidelity task.
