# Saved Form Appearance Value Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans inline. No implementation delegation; one final scoped review.

**Goal:** Prevent appearance-only edits from replacing a different saved answer.

**Architecture:** Reuse the parsed native appearance context, copy its AP/resources
through the bounded Copier into a private page, and compare native Unicode glyphs
and generated boundaries with the inherited stored value before mutation/publication.

**Tech Stack:** Rust/lopdf/PDFium, existing Tauri/React error handling, Windows acceptance.

**Spec:** docs/superpowers/specs/2026-10-25-form-appearance-value-design.md

## Global Constraints

No commits/publication/destructive cleanup. Preserve existing worktree, source PDFs,
accepted binaries and failed evidence. One Cargo/build at a time. Value and native
character budget128KiB; copied graphs retain existing Copier limits. No permission
prompt; user opted out. Native all-font text extraction must not substitute fonts.

## Review Focus

- Generated spaces/newlines must not falsely reject comb or wrapped ordinary values.
- Real whitespace boundaries must not silently equate different saved answers.
- A long value/expanding AP/resource graph must be bounded before copying or decoding.
- Missing or unreadable AP text must not authorize replacing a nonempty saved answer.
- Protected/repeated-page editing must still verify the selected immutable source.

## Task1 — reproduce and enforce native consistency

Files: new src-tauri/src/form_appearance_value.rs and tests/form_appearance_value.rs;
modify module inclusion,form_text_appearance.rs,form_text_appearance_engine.rs.

Interface: private verify(pdfium:&Pdfium,source:&Document,widget:&Dictionary,value:&str)
returns EngineResult<()>. Existing public IPC shapes unchanged. Change private
appearance inspect to accept the existing Pdfium owner and invoke the check using
its parsed Context. Avoid a second full-source parse for the probe.

- [x] Archive scoped baseline; public RED proves size-only edit changes saved text.
- [x] Write matcher/wrapping/comb/embedded/missing/oversized regressions. Existing embedded
  font preservation and filled ASCII reopen inspection pass; populated non-ASCII
  embedded restyling remains a recorded gap.
- [x] Implement bounded private AP probe and boundary comparison, integrating original
  inspection and generated-output confirmation.
- [x] Run new and existing appearance/filling/positioning/reflow suites; inspect output.

## Task2 — review and desktop checkpoint

- [x] Freeze scoped source/hash delta; request one final reviewer with limits.
- [x] Reproduce and correct actionable findings; no rereview.
- [x] Full frontend, appropriate native gates and fingerprinted desktop build.
- [x] Desktop demonstrates explicit refusal for inconsistent saved text and successful
  ordinary appearance editing/fill/Undo/save/reopen; independent saved-output checks.
- [x] Stop owned app, bind evidence and update README/quality loop. Leave goal active.
