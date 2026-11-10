# Styled paragraph editor implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement inline,
> task by task. The user authorized autonomous project work without routine
> design/permission gates. Preserve the current dirty worktree and existing apps.

**Goal:** Let a user edit a paragraph's words and selected formatting while
preserving all other styles, then preview, apply, undo, save and re-edit it.

**Architecture:** A separate attributed editor uses the existing immutable
`styled-paragraph.ts` range model and verified native mixed paragraph pipeline.
Keep the current uniform dialog available until mixed desktop acceptance passes.
The PDF preview remains the appearance authority; the editable draft is an input
surface, because arbitrary embedded PDF fonts are not browser font registrations.

**Tech stack:** Existing React/TypeScript, Tauri commands, Rust worker and PDFium.
No new editor dependency is assumed.

**Spec:** `2026-10-23-mixed-paragraphs.md`, milestones 6 and 7. Native faithful
memory evidence is recorded separately in
`artifacts/mixed-paragraph-memory-20261024/verification.json` when finalized.

## Design decisions and constraints

Use explicit UTF-16 range transactions rather than guessing edits from whole
strings. The existing model preserves untouched runs and rejects grapheme splits.
A separate text input for each style would break words/paragraph reflow. A plain
textarea with a whole-string diff cannot reliably attribute repeated text or
composition edits. Use a bounded contenteditable surface with controlled text
spans, explicit beforeinput ranges, plain-text paste, and application-owned undo.
Browser mutations are provisional during composition and never PDF authority.

Keep 4096 UTF-8 bytes, 64 styles, sizes 1–300 pt, spacing 0.6–3, native page/work
limits, expected source text, permissions and collision gates. Do not synthesize
bold/italic or silently substitute fonts. A displayed PDF font name is a label,
not a browser font-family or permission to bind a resource. Source previews
include existing overlays through `renderParagraphPage` in unrotated source axes.

Ordinary manual source selection stays limited to 64 pieces. A native verified
saved-paragraph suggestion may return up to 4096 pieces as one selection; never
expand that limit merely because the frontend receives a larger array. Measure
release-build latency and retain progress/cancel/stale-result handling before
making large saved selections user-visible.

## Review focus

- Composition, dead keys, paste and deletion across a grapheme/style boundary
  must retain the preceding valid draft on failure (tasks 2–3).
- Caret formatting and mixed selection formatting must derive from immutable
  original native styles, including multiple overrides and undo (task 2).
- An async preview/font acquisition may finish after dialog close, source change,
  tab close or a newer draft; reclaim it without changing the current page (task 4).
- A saved paragraph can contain blank-only styles and more than 64 pieces;
  suggestion must independently verify it before exposing the full draft (task 1).
- Applying to the wrong/changed page or tab must fail without losing the draft,
  accepted preview, font leases or workspace history (tasks 4–5).

## Task 1: Native bridge and verified saved-paragraph selection

Files: `src-tauri/src/{app,engine,styled_paragraph_memory,text_paragraph}.rs`,
`src/editor/{types,adapter}.ts`; new native suggestion and adapter tests.

Interfaces: optional `FolioAdapter.inspectStyledParagraph(sourceId,pageIndex,
objectIndices): Promise<StyledParagraphSelection>`,
`suggestStyledParagraph(sourceId,pageIndex,objectIndex,expectedText):
Promise<StyledParagraphSelection>`, and `previewStyledParagraph(sourceId,pageIndex,
request:StyledParagraphLayoutRequest): Promise<DocumentInfo>`. TypeScript DTOs
mirror the existing Rust camelCase DTOs exactly; no caller-supplied resource names.

- [x] Test command forwarding/serialization with original indices/expectedText,
  runs, per-style overrides and bounds; omitted optional changes stay omitted.
- [x] Add a saved-record candidate query in `styled_paragraph_memory.rs` using
  actual snapshots and anchor text. Resolve exactly one independently restored
  candidate through the mixed baseline. Reject overlap/order ambiguity; do not
  select the first metadata record that happens to contain the anchor.
- [x] Test an anchor in a >64-fragment saved paragraph, changed anchor text,
  malformed/forged/overlapping records, whitespace fragments, and ordinary mixed
  selection. An ordinary anchor may fall back to that checked single piece;
  manual selection is available. Do not apply uniform geometric inference to
  mixed text without separate proof/tests.
- [x] Expose inspect/suggest/preview Tauri commands using existing worker pattern;
  register handlers. Keep layout internal until a consumer needs it.
- [x] Run focused native suggestion/memory tests, adapter tests and TypeScript
  checking; review before wiring into the user-facing dialog.

## Task 2: Draft transactions, palette derivation and history

Create `src/editor/styled-paragraph-editor.ts` and
`tests/styled-paragraph-editor.test.ts`. Reuse `styled-paragraph.ts` unchanged
unless a reproduced model defect requires a correction.

Interfaces: `createStyledEditor(selection): StyledEditorState`,
`editStyledEditor(state, transaction): StyledEditorState`,
`undoStyledEditor(state): StyledEditorState`, `redoStyledEditor(state):
StyledEditorState`, `styledEditorRequest(state): StyledParagraphLayoutRequest`.
State owns immutable native selection, draft, UTF-16 selection, typing override,
style definitions, bounds/alignment/spacing and bounded undo/redo snapshots.
Transaction union covers replace range, select range, format range, area and
paragraph formatting. A selection-only move does not create an undo entry.

- [x] Write tests for insertion/replacement/deletion, repeated words, CRLF paste,
  empty draft typing style, grapheme joins, mixed selection, caret override,
  hard/soft breaks and atomic rejection at byte/style limits.
- [x] Derive every changed style from its original native style ID. When changing
  a previously derived style, preserve prior overrides but reference the original
  sourceStyleId, never another changed target. Deduplicate identical definitions;
  request IDs must be contiguous and under 64. Do not reassign an ID still used
  by a retained undo snapshot. Compact/remap only when constructing a request.
- [x] Test formatting a selection spanning two source fonts changes only the
  requested property and keeps both source font identities. Reset clears only
  that property. Snapshot history retains per-style font IDs for lease ownership.
- [x] Bound history to 100 snapshots. Group a composition transaction into one
  undo step. Preserve redo after selection movement; clear it after content or
  format edits. Test undo/redo caret and typing style restoration.
- [x] Run new model tests plus existing styled-paragraph tests and `npx tsc --noEmit`.

## Task 3: Accessible attributed text input

Create `src/components/StyledParagraphInput.tsx`, scoped CSS,
`tests/styled-paragraph-input.test.tsx` and a browser acceptance fixture.

Interface: controlled state/transaction callbacks; DOM spans contain only literal
text and style labels/classes. The surface has role textbox, aria-multiline and
an accessible name. Convert DOM selection endpoints to bounded UTF-16 offsets.
Never set innerHTML from clipboard or treat browser-created formatting as style IDs.

- [x] Test beforeinput target ranges for insert/delete/linebreak/cut/replacement,
  plain-text paste over mixed spans, selection direction, keyboard navigation,
  Ctrl+A, Ctrl+Z/Ctrl+Y and grapheme boundaries. Reject unsupported transformations
  by restoring the existing model and keeping focus/selection.
- [ ] During composition preserve the starting immutable draft and range, avoid
  rerendering its active DOM, and commit one normalized replacement at composition
  end. Test canceled composition, empty composition, compositionend followed by
  input, source replacement mid-composition, and native dead keys in Windows.
- [x] Render source size/color cues without claiming embedded browser fonts match
  the PDF. Reuse registered custom FontFaces only with active leases. Keep spans
  selectable across style boundaries and empty/trailing-newline input usable.
- [x] Run browser keyboard/clipboard/composition checks as well as component tests;
  jsdom alone cannot accept this input implementation.

## Task 4: Mixed dialog, preview ownership and font leases

Create `src/components/StyledParagraphDialog.tsx` and focused flow tests. Reuse
`ParagraphPagePreview`, `FontPicker`, paragraph area geometry and overlay renderer.
Extract shared lifecycle code only when both dialogs genuinely use it; avoid a
speculative refactor of the existing working uniform dialog.

- [x] Show checked piece selection, attributed input, range format controls,
  alignment/spacing and area. Mixed values display Mixed; selecting a color/size
  changes only that property across selected runs. Caret controls set typing
  overrides. Font picker checks the actual selected/replacement text.
- [x] Keep the draft on native rejection. Manual Check selection intentionally
  replaces it only on successful inspection; Cancel adjustment restores it.
- [x] Give each preview a generation token and its own set of font leases. Hold
  all IDs referenced by draft/history/active requests; release after their final
  owner is gone. Deduplicate IDs and use existing serialized acquisition helpers.
- [ ] Test out-of-order preview/inspect/font results, render failure, cancel,
  unmount, source changes, reset and undo after substitution. Each orphan native
  document and blob URL is reclaimed once; transferred results stay alive.
- [x] Follow the existing synchronous `onApply(document): boolean` ownership
  transfer. A refusal retains the candidate/draft; an accepted result relinquishes
  dialog ownership. Show working status, disable duplicate submissions, and keep
  Cancel responsive while native work completes in the background.
- [x] Run dialog/model/lease tests and browser focus/narrow-window checks.

## Task 5: Workspace integration and desktop acceptance

Modify `src/App.tsx` to route the paragraph action to the mixed dialog only when
all mixed adapter methods exist; preserve uniform fallback otherwise. Continue
using `commitTextReplacement` to preserve page identity, rotation, overlays and
undo/redo. Do not apply a result to a source/page that changed during editing.

- [ ] App tests: correct originating tab, tab/page close, changed source, stale
  preview, accepted ownership, cancellation, overlay composition and workspace
  undo/redo. Ordinary uniform requests keep their fallback behavior.
- [x] Run full frontend tests, TypeScript/Vite build, relevant native suites and
  scoped review. Resolve demonstrated defects before desktop acceptance.
- [ ] Build a new debug executable only after connected-flow checks pass. Native
  Windows acceptance: type/insert `this`, delete, paste, selected size/color/font,
  caret typing, IME/dead keys, undo/redo, actual PDF preview, save/reopen/re-edit,
  all rotations, comments/overlays, and rejection retaining the draft.
- [x] Profile release-build small/imported-font and 4096-fragment cases. Do not
  infer interactive speed from debug timing or hide the remaining delay.
- [x] Independently inspect actual saved text/style/origins/notes/pixels; preserve
  private quiz and previous runnable/package hashes. Recheck the quiz without
  copying private text into fixtures. Highlight collisions remain a separate task.
- [x] Record source/binary fingerprints, completed evidence, limitations and next
  gaps. No Adobe/Foxit parity claim or overall goal completion from this milestone.

## Connected checkpoint — 2026-10-24

Implemented and verified against manifest
`artifacts/styled-paragraph-editor-20261024/verification.json`. Frontend 809/809,
six focused native suites 41/41, TypeScript/Vite and debug executable build pass.
Nine input and four dialog browser checks pass. Actual Windows edited/saved PDFs
cover all four rotations; independent MuPDF/pypdf checks verify eight exports,
unchanged styles/origins/native notes/neighbors and exact repeated-edit pixels.

Release full-cycle measurements (layout, preview, raster, export, reopen and
inspection): source font 68–75 ms; imported fonts 514–787 ms; 4096 fragments
1425 ms initially and 2398–2721 ms on re-edit. This is not one UI preview latency.

Unfinished boxes retain specific coverage gaps: OS-driven dead keys/IME and
trusted clipboard, delayed real FontPicker acquisition across source changes,
and styled-specific end-to-end stale tab/page cases. Current helper/dialog tests
cover neighboring paths. Review rounds 1–3 found no P0–P2, with one deferred P3:
Reset position does not restore a cleared coordinate whose model is already
original. These limits are not hidden by the passing checkpoint.

The quiz's original subset refused the added h in several objects; other trials
hit neighboring text. An explicit DejaVu Serif at 10pt or smaller original size
previewed an insertion. No private export/text copy occurred; original hash and
47 previous executable/package hashes remain unchanged. Highlighted instructions
are still a separate graphics-fidelity task. The overall goal remains active.
