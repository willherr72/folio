# Folio v0.10.0 verification

Verified on Windows x64 on 2026-09-13 with bundled PDFium chromium/8044.
[Machine-readable results](verification-v0.10.0.json) record the tested executable
hash and workflow outcomes. The same executable is included in the portable ZIP.

## Release checks

- 241 frontend tests passed across 48 files.
- 137 native tests passed; three existing opt-in tests remain ignored.
- TypeScript/Vite/Tauri production build and Rust formatting passed.
- HarfRust provenance verified 85 upstream files and two documented patches.
- License collection includes 551 dependency records, full notices, and HarfRust's
  local patch, provenance and change notice. The collector resolves worktree
  junctions and verifies matching production dependency declarations.

The retained [editor rendering matrix](shaped-editor-integration.md) separately
covers 64 editor/native raster comparisons and 32 native crop/rotation cases.

## Packaged desktop acceptance

An isolated production WebView used native Open, Import font and Save dialogs.
No test IPC or browser mock replaced PDF editing or font preparation.

- New text enters editing immediately. Importing DejaVu Serif renders native
  shaped outlines; toggling ligatures changes the actual glyph count.
- Text undo/redo passes. CDP composition followed by insertion commits as one
  undo step in the real WebView.
- Editable and flattened saves preserve the original PDF. Reopening the editable
  copy works after moving both the original PDF and imported font. Shaping
  controls, font data and exact logical text survive.
- Shaped annotation search finds the full phrase. The flattened PDF exposes exact
  Unicode through the native reader; dragging and copying returns `office café Ω`.
- Tab return retains native preparation. Print setup opens and cancels correctly;
  this check submits no physical print job.
- After a forced stop of the owned test process, recovery restores an unsaved
  shaped draft with both source PDFs and the imported font unavailable. Native
  outlines regenerate, controls restore and the draft remains searchable.
- Separate native font imports, previews, flattened exports and reader reopen
  pass for Arabic `سلام` and Indic `नमस्ते`. MuPDF and pypdf independently extract
  those exact phrases and the Latin/Greek phrase from the packaged app's exports.

The hidden Windows Shell dialogs expose their filename edits as accessibility
Panes. The bounded test helper uses exact process/title/control matches and
native edit replacement, then reads back the filename before accepting. It refuses
existing Save targets. This replaces the failed automation path documented for
v0.9.1 without modifying application dialog behavior.

## Limits

WebView composition automation is not physical Windows IME acceptance. Foxit
opened a synthetic export, but its hidden document pane exposed no text pattern;
no Foxit or Acrobat copy/selection pass is claimed. Independent text extraction
is distinct from interactive selection compatibility.

Shaping remains opt-in per text box with an exact custom static TrueType-outline
font. Mixed direction, RTL word separators/joiners, font fallback, paragraph
wrapping, variable/color/CFF fonts, and more than 255 distinct semantic character
definitions remain refused. This does not expand general existing-PDF complex
text replacement or introduce glyph-level canvas caret editing.

## Reproduction

Run `scripts/start-release-test.ps1 -Binary <portable/Folio.exe> -RunRoot <fresh-directory>`.
Set `FOLIO_RELEASE_RUN` to that directory, then run these scripts in order:

1. `node scripts/smoke-shaped-editor-desktop.mjs`
2. `node scripts/smoke-shaped-recovery-desktop.mjs`
3. `node scripts/smoke-shaped-scripts-desktop.mjs`

Each uses the launch record's owned process and debugging port. Test copies and
recovery data remain isolated in that directory. Stop the recorded test process
when finished. Run the frontend suite with `npx vitest run --maxWorkers=2 --minWorkers=1`
and native suite with `cargo test --locked --manifest-path src-tauri/Cargo.toml -j 1`.
