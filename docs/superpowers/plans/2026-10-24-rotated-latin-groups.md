# Rotated Latin groups implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Edit verified accented and single-character groups on rotated pages.
**Architecture:** Selection-bound native direction proof replaces only the group
call to the page-wide ASCII workaround; existing publication proofs remain.
**Tech Stack:** Rust/PDFium/lopdf, Python independent readers, Tauri/WebView.
**Spec:** `docs/superpowers/specs/2026-10-24-rotated-latin-groups-design.md`

## Constraints and review focus

Keep the global reader helper unchanged. Preserve current page bounds, existing
font/encoding restrictions, no-op bytes, permissions/comments and neighboring
content. Check rotated glyph ordering, single characters/space boundaries,
unselected directions, malformed transforms, and borrowed object lifetime.

## Task 1: Selected direction and Latin compatibility

Files: `src-tauri/src/text_group.rs`, `src-tauri/tests/text_group.rs`.
Public APIs unchanged; private native proof validates only selected group members.

- [x] Add rotated/cropped seven-split embedded-font test; observe current refusal.
- [x] Replace group use of `simple_forward_text` with selected transform and
  native-origin checks; retain 64-object/4096-character bounds for rotated pages.
- [x] Verify unrelated orientation, single letters, exact no-op/export, original
  bytes/font, and selected backwards/sheared/vertical/baseline refusals.

## Task 2: Verification and checkpoint

- [x] Generate 28 public-fixture cases; verify independent no-op and edited output.
- [x] Run group/library/comment/protection/paragraph regressions as appropriate.
- [x] One fresh scoped review, one important-fix TDD pass if necessary.
- [x] Build and verify actual Windows rotated-group edit/undo/save/reopen; record
  executable, loaded renderer and source fingerprints, limits and quality ledger.

No commits or external publication within this broad ongoing dirty worktree.
