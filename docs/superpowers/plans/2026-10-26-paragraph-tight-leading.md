# Faithful tight paragraph leading implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final review.

**Goal:** Preserve tight source baselines when actual text and highlights fit.
**Architecture:** Versioned measured vertical extents shared by framing, layout,
publication and independent saved replay; old policies remain exact.
**Tech Stack:** Rust, PDFium, lopdf; existing desktop/CDP acceptance harness.
**Spec:** docs/superpowers/specs/2026-10-26-paragraph-tight-leading-design.md

## Global constraints

User authorized autonomous inline work, no routine approval, no commits/publish.
Keep 4096 bytes/spans/fragments,64 styles,256 lines,1–300physical pt and0.6–3 spacing.
Keep native ownership, frame, collision, resource and raster checks unchanged.
One Cargo/build at a time; preserve failed results and accepted binaries.

## Review focus

- Whitespace with a larger font must affect nominal leading and blank-row fallback.
- Substituted/reopened full fonts need the same native bounds after registry release.
- Tall accents, descenders and highlights must still reserve real painted clearance.
- Old records must retain vertical placement despite normalized UI style IDs.
- Forged version/policy hints must not authorize a different layout or neighbor edit.

## Task 1: Reproduce and implement versioned vertical ink

Files: styled_paragraph_layout.rs and its tests; paragraph_highlight_geometry.rs;
styled_paragraph_area.rs; styled_paragraph_plan.rs; styled_paragraph_memory.rs;
styled_paragraph_native_metrics.rs; text_paragraph.rs; relevant integration tests.

- [x] Capture product baseline/hash manifest and pre-change v4 dense paragraph.
- [x] Add native RED for18pt source baselines (currently18.112), assert row count,
      first baseline, fonts, reading order and unchanged source; save old v4 fixture.
- [x] Add `ink_vertical:bool` to Prepared/Record, true fresh and false old. Emit v5
      when any record opts in. Reject invalid old-version/new-field combinations.
- [x] Add row_extent(atoms,styles,blank_style,gaps,extra,ink_vertical)->Extents.
      Existing horizontal extent remains authoritative; replace only vertical
      extents with visible measured bounds, falling back on blank rows.
- [x] Thread policy through layout_with_text_metrics, first-line ascent and shared
      highlight layout wrapper; framing and replay must call the same policy.
- [x] Extend native_highlight_metrics with include_unmarked:bool; use true for
      v5 full fonts in fresh layout, initial area and replay, false for old records.
- [x] Test actual bounds, blank/space-only styles, accent/descender/mark minima,
     256 rows, minimum spacing, malformed metrics and old/new placement; run RED/GREEN.

## Task 2: Saved and physical fidelity

Files: tests/styled_paragraph_source_alignment.rs, highlight_publication.rs,
paragraph_physical_sizes.rs as needed; tests/fixtures/paragraphs/v4 golden/provenance.

- [x] Verify old v4 dense fixture repeats18.112pt and identical pixels/metadata;
      retain old v2/v3 cases and add forged flag/version rejection.
- [x] Exercise18pt fresh/reopened/repeated paragraphs in all rotations and decimal
      UserUnit, including marked and substituted full fonts after registry release.
- [x] Audit emitted synthetic PDFs independently for order, baselines and pixels.
- [x] Run affected paragraph library/native targets and reconcile every failure.

## Task 3: Review and desktop checkpoint

- [x] One scoped final review; reproduce and correct demonstrated findings inline.
- [x] Fresh desktop build and actual quiz plain/highlighted Preview/Apply/Undo/Redo/
      Save/reopen/re-edit; preserve exact source/binary/PDFium fingerprints.
- [x] Final affected acceptance; update quality record, plan and execution ledger.
      Keep broader Adobe/Foxit quality goal active and remaining gaps explicit.

Final checkpoint: artifacts/paragraph-tight-leading-20261026/verification.json.
Pre-correction broad130passed/1ignored; final51library+32integration+1explicit
editor-normalization=84unique passes. Exact18pt marked/full-font cases passed.
Final EXE7ba8af04/source0bafff53 passes both quiz desktop/independent workflows.
Review P3 reproduced/corrected inline; old/new same-page replay coverage added.
No commit, release, parity claim or overall goal completion.
