# Native comment preservation during text rewrites

Portable 0.19 integration exposed an untouched native note changing Rect from 24pt to 20pt and gaining PDFium-generated AP while editing a saved, rotated paragraph. Independent MuPDF found changed pixels outside the paragraph. The failing release package remains immutable and is not an accepted checkpoint.

Use the existing immutable-source annotation geometry/appearance restoration at each final text-rewrite byte boundary before candidate validation/publication. Cover paragraph, single-run and grouped text editing; all can save copies that PDFium has rendered. Form-occurrence editing already rejects annotated sources; retain and test that guard without adding a new form feature. Retain annotation order/identity checks and popup owner validation. Restore original AP absence and discard only newly introduced PDFium generated-appearance markers. Do not reconstruct custom appearance streams or change overlay ownership. Keep permission, form, source immutability and candidate pixel proofs.

- [x] Reproduce native metadata loss in paragraph/single/group paths and test intrinsic quarter turns; confirm the existing form annotation refusal.
- [x] Apply shared restoration to final candidate bytes, with original source-page/exclusion mapping.
- [x] Run affected text/comment/form suites and broader native regression checks: 22 focused and 434 full native tests pass; four existing ignored tests.
- [x] One focused fresh review of this new source correction; one fix pass (candidate popup ownership and custom appearance coverage added; final regression gate passed).
- [x] Verify the failing saved-rotation Windows paragraph workflow and independent output with a new copied build.
- [x] Build a new 0.19.0-dev.2 package, preserve dev.1 failure evidence, and complete package acceptance including quiz, images/recovery and OCR. Evidence: artifacts/portable-0192-verification.json.

No change to user documents, no publication/commit, and no general parity claim. Existing package evidence: artifacts/portable-019-acceptance. The image acceptance failure was a floating-navigation overlay covering the bottom-right handle; the harness now centers the target in the document viewport and verifies the pointer actually hits it before dragging. Retest in a fresh profile; do not mask pixels or weaken PDF assertions.
