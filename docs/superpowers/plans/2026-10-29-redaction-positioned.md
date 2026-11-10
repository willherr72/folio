# Positioned text removal verification plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline; one final review.

**Goal:** Preserve retained native/independent text placement when removing partial TJ operands, and independently validate each reader's verifier.
**Architecture:** Extend existing synthetic corpus and saved-output oracle; correct only demonstrated failures in shared range removal.
**Tech stack:** Existing Rust/lopdf/PDFium and Python MuPDF/pypdf/numpy.
**Spec:** docs/superpowers/specs/2026-10-29-redaction-positioned-design.md

## Global constraints
All bounds and independent0.002pt/exact-outside-pixel criteria from the spec remain. No public integration/package/commit. Preserve failures and earlier immutable artifacts. One Cargo --locked --offline --lib with one job; no implementer agents.

## Review focus
- Removed fragments spanning multiple source strings must keep every original TJ numeric move and following text.
- Prefix/suffix and active Tc/Tw/Tz must not silently use nominal unspaced compensation.
- Native float accumulation may differ from f64 interpreter despite equal nominal displacement.
- Independent reader negatives must execute both actual comparison paths.
- Orphan encoded payloads must still fail graph validation; source-only raster masks must not expand with output paint.

## Task1: saved actual-TJ evidence
Modify src-tauri/src/redaction_ranges_tests.rs to share existing corpus export and add FOLIO_REDACTION_POSITIONED_FIXTURES with six TJ layouts x12faces x2encodings. Add a unit test spanning multiple string/numeric fragments, checking all retained code/origin/endpoints. Copy prior auditor/probe to artifacts/redaction-positioned-20261029; retain old artifacts.
- [x] Export new fixtures and run focused native tests. Expected: valid nominal geometry accepted; any failure retained for diagnosis.
- [x] Run both readers on new saved fixtures; expected: all retained raw text/geometry and outside pixels unchanged. If red, capture exact cases and native reproductions.

## Task2: corrections and reader-isolated negatives
Expose compare_reader(case,before,after,reader_name,diagnostic=None) in the new auditor; compare() still runs both readers plus object/raw-operand checks. Add tests demanding each designated reader rejects each of four real damaged PDFs. Expected first RED for missing interface, then GREEN with independently recorded reader reasons. Fix product defects only after a failing native regression; retain source operation semantics and budgets.
- [x] Add independent reader negative checks, observe RED then implement helper and GREEN.
- [x] Diagnose/fix any native fidelity failure, then rerun new and old saved corpus with unchanged criteria.

## Task3: review and record
- [x] One final frozen scoped review of exact baseline delta and audit evidence; fix Important findings locally with failing/passing tests.
- [x] Appropriate focused/library gate, bind terminal evidence, record checkpoint, roadmap and next substantive redaction integration work. No broad parity/public-redaction claim.
