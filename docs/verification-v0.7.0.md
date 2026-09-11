# Folio v0.7.0 verification

Verified on Windows on 2026-09-11. This release implements the bounded existing-text editing scope in [issue #8](https://github.com/willherr72/folio/issues/8). See [supported documents and preservation checks](existing-text-editing.md) before testing arbitrary PDFs.

## Automated checks

- `npm test -- --run`: 172 tests passed in 36 files. Includes dialog focus, unsupported/error states, immutable source ownership, duplicate isolation, undo/redo, export/search integration and late asynchronous completion cleanup.
- `cargo test --tests --lib`: 54 tests passed; three existing opt-in printer/large-corpus checks remain ignored. The nine text-edit integration tests include twelve standard font faces across four cropped page rotations, shorter/equal/narrower replacements, colored text, unchanged surrounding pixels, unsupported encodings and PDF layers, chaining, source-file protection, annotation preservation, multipage isolation and a 400-run listing fixture. Recovery also succeeds after the original file is removed, preserving edited bytes, rotation and annotations through checkpoint and export/reopen.
- `node scripts/smoke-existing-text-browser.mjs`: 32 source/editor rotation and zoom combinations passed using actual pointer clicks and keyboard submission. Changed text-layer content and copy events were checked, with no browser errors.
- `cargo fmt --check`, TypeScript compilation and the Vite/Tauri production build passed. No dependency versions changed.

## Packaged desktop and independent PDF checks

The isolated production executable passed the complete desktop harness without resuming midway. Windows file dialogs were submitted separately in the owned test process; earlier attempts exposed automation timeouts and an overly narrow error-message assertion, which were corrected before the final run.

The desktop run opened a standard-font PDF, added an annotation, rejected an overlong replacement while keeping its input, changed “Original sentence” to “New sentence”, then verified undo/redo, search invalidation and native mouse selection with Ctrl+C. Saving preserved the original file's SHA-256. The saved PDF reopened after moving the original out of its former path, retained the annotation, and exposed the replacement as an editable run. An embedded-font fixture displayed an unsupported explanation and no Apply action. There were no WebView runtime errors.

Independent pypdf extraction returned the replacement and unchanged neighboring text, with no original sentence. PyMuPDF rendering with annotations excluded found 1,040 changed pixels, all inside the union of the old/new text bounds plus a four-point inspection allowance. The saved PDF contained one annotation; visual inspection confirmed its appearance and the edited text.

[Machine-readable browser, desktop and independent-reader results](verification-v0.7.0.json) retain the fixture/output hashes. Tested and shipped executable SHA-256:

`46c79da5b68347a52f3db14cf1c384b439653f34b624ce45f694dc213ecf67c5`

Independent runtime review found a PDF-layer visibility problem during page import. The final implementation rejects optional-content documents before copying and requires the unedited copy to render identically to the original; the regression passed and review found no remaining important issues.

## Reproduction and limits

Generate owned fixtures with `python scripts/generate-text-edit-fixtures.py` (requires PyMuPDF). Start a production executable with `scripts/corpus-start-desktop.ps1` using a unique CDP port and isolated profile. Set `FOLIO_APP_PID`, `FOLIO_CDP_URL` and, when submitting dialogs separately, `FOLIO_DIALOG_MANUAL=1`; run `node scripts/smoke-existing-text-desktop.mjs`. The harness intentionally moves its generated original and refuses to overwrite an existing output; retain those artifacts or use a fresh test directory/worktree for another run.

This is a conservative first version, not general paragraph editing. Embedded/subset fonts, OCR, reflow and secure redaction remain outside its scope. Physical low-memory hardware validation remains tracked separately in [issue #10](https://github.com/willherr72/folio/issues/10). This release does not make a new performance claim or replace the existing opt-in real-printer tests.
