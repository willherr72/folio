# Original quiz editing session implementation plan

> Use superpowers:executing-plans inline. Preserve the inherited worktree; no commits or cleanup in this milestone.

**Goal:** Establish and, where necessary, repair faithful insertion, reflow and sustained saved re-editing of the original quiz.
**Architecture:** Existing CLI produces disposable previews and immutable saved copies; an independent Python audit checks source-derived fixed regions and both readers.
**Tech Stack:** Rust/PDFium CLI; Python, MuPDF, pypdf, Pillow; existing Windows process-memory support.
**Spec:** docs/superpowers/specs/2026-11-03-quiz-growth-session-design.md

## Global constraints
- Original attachment, retained copy and accepted 0.31 package remain unchanged.
- Private quiz data stays local; no external font input or silent substitution.
- Serialize native PDF jobs; reuse verified build only while fingerprints match.
- Failed runs and immutable evidence remain retained.

## Task 1: Bound the independent oracle and first saved session
Files: artifacts/quiz-growth-session-20261103/session.py and audit.py; tests in test_audit.py.
Consumes: inputs.json and inspect-original/first-question/instructions/last-question recon responses.
Produces: fixed case definitions, command receipts, saved outputs and independent audit reports.
- [x] Define source-derived regions and expected edits before processing outputs. Record original style/geometry, all baseline hashes and candidate identity.
- [x] Write oracle negative controls; run before completing the oracle and observe failures. Implement complete-page text, selected style, unselected glyph/pixel and region checks. Expect positive controls accepted and each targeted corruption rejected for the intended reason.
- [x] Run small word insertion and saved re-edit on the first question. Expect exact requested normalized text, retained original font/size/color, unchanged outside pixels/glyphs and unchanged source bytes.

## Task 2: Growth, highlights and sustained saved re-editing
Consumes: Task 1 CLI receipts/oracle. Produces: separate saved-session and resource reports.
- [x] Run final-question multi-line growth into preselected free space, another saved edit, and unchanged re-edit. Expect actual wrapping and unchanged-pixel no-op.
- [x] Edit/re-edit highlighted instructions while retaining source run styles. Expect exact text/style sequence and outside-region pixels.
- [x] Run 12 alternating add/remove saved edits on a source-owned paragraph; record duration, sampled memory, file/object/font/resource counts. Investigate monotonic growth rather than relabeling it acceptable.
- [x] If a product defect is demonstrated, add a focused RED regression and correct it; otherwise retain product source. Run appropriate regression evidence and verify fingerprints.

## Task 3: Integrated review and checkpoint
- [x] Freeze helper/evidence hashes and request one fresh integrated review. Exclude private PDF/text content from the review package unless essential to a demonstrated finding.
- [x] Resolve Important/Critical findings once with negative-control or regression evidence. Record declined judgments, costs and deferred minors.
- [x] Verify recorded saved-output hashes, original identities and reused-test provenance before checkpointing. Update the quality roadmap and leave the overall goal active.

## Review focus
- Output-selected masks can hide damage: fixed source-derived regions plus outside-content negative controls (Task 1).
- Whitespace normalization can conceal reordered/missing content: compare full normalized sequence in two readers (Task 1), record exact-whitespace limitation.
- Style/line grouping can confuse selected and neighboring labels: source ownership and fixed geometry, style corruption controls (Task 1/2).
- Sustained edit evidence can omit descendants or outputs: bind each command/candidate/saved hash and record the memory sampling scope (Task 2).
- Skipped/old tests can be misrepresented as fresh: check source/test fingerprints and state reused versus fresh evidence explicitly (Task 3).
