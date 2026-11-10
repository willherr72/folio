# Paragraph suggestions implementation plan

> Execute inline with superpowers:executing-plans. One focused fresh review and one fix pass at the milestone boundary.

**Goal:** Open paragraph editing with a validated selection around the clicked text, including words split across PDF objects, and let the user adjust it before editing.

**Architecture:** A read-only native suggestion uses bounded page snapshots in raw PDF coordinates, then the existing paragraph validation. The React dialog loads a suggestion alongside its text-piece list and presents the original paragraph immediately. Manual correction remains available. No preview source or workspace history is created by suggesting text.

**Tech stack:** Existing Rust/PDFium/lopdf worker; Tauri command; React/TypeScript.

## Design and decisions

The previous turn completed portable0.19.0-dev.2 and is progress. The existing linked worktree is reused; preserve all previous packages and uncommitted work. The user authorized autonomous implementation and uniform-format paragraphs. No routine approval or new worktree is needed.

The actual quiz has a question split into three objects, including `data`/`base`. Today each fragment must be selected manually. A rectangle-selection tool would reduce clicks but leave paragraph interpretation to the user. A frontend font-name heuristic lacks native font identity, color and transformation information. Choose a native suggestion with the same final validation as an explicit selection, while displaying it as a suggestion that the user can correct.

Build bounded content-order text rows from baseline, horizontal gaps and direction; keep list markers as boundaries. Grow from the anchor through neighboring aligned rows with ordinary leading, stopping at larger spacing, indentation/column changes and new list items. Geometry is measured before intrinsic page rotation. Reject unsupported orientation, excessive members and mixed formatting through the existing native checks; do not silently discard pieces of a mixed row. Ambiguous layouts can require manual selection. This does not promise exact paragraph semantics for PDFs without structural information.

Return the existing TextParagraphSelection after baseline validation, checking the clicked text is current. Keep all permissions/source admission, maximum64 pieces/4096UTF-8 bytes, font identity/color, ordering, page-copy appearance and subsequent candidate pixel proofs. Preview and Apply retain their ownership semantics. Suggestion does not mutate original bytes or publish a new source.

Dialog behavior: native adapters attempt suggestion on entry; successful results initialize the selected pieces, text and area. Show a suggestion notice and an explicit Adjust selection action. Individual checkboxes and page selection remain the fallback. Failed suggestions explain that manual selection is available. Late results after cancellation, unmount or source changes cannot replace state. Adapters without the new optional method retain the existing manual workflow.

## Tasks

- [x] Add failing native tests for split-word/wrapped-line selection from every member; separate labels/questions/columns/large gaps; mixed format refusal; all rotations; immutable source; bounded/stale selection.
- [x] Implement a bounded native selector, worker/API/Tauri command, then existing paragraph validation; run affected native tests.
- [x] Add failing UI tests for automatic initialization, manual adjustment, fallback and delayed results; implement optional adapter and dialog behavior.
- [x] Run frontend suite/build and relevant native text/comment suites; one focused fresh review, one fix pass.
- [x] Build a copied Windows debug executable. Exercise actual quiz without manual fragment selection, adjust selection, preview/apply/undo/save/reopen; independently verify extracted text, font and unchanged pixels. Verify rotated selection and unchanged mixed-format content.
- [x] Record hashes/evidence and gaps; preserve portable0.19.0-dev.2. Do not repackage merely to distribute this source checkpoint.

Completed evidence: `artifacts/paragraph-suggestion-verification.json`. Six final native
suggestion tests pass, including review-driven inline-baseline refusal and centered
uniform/mixed paragraphs. The centered case was reproduced both synthetically and
in the actual quiz before correction. 675 frontend and 68 related native tests pass;
the related native run preceded the selector-only fix pass. One review completed;
no second round. Original documents, earlier packages/debug copies remain unchanged,
and all owned jobs exited. Next observed UX gap: the expanded piece list and Check
selection action compete with the ready paragraph editor after suggestion.

Review focus: partial selection inside mixed rows; adjacent list items/columns with the same font; intrinsic rotations/crop offsets; asynchronous UI results after manual actions or closing; permission and original-source immutability. Synthetic tests cover these classes; actual quiz remains private in artifacts.
