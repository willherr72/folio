# Keep text replacements when entering paragraph editing

## Intent and current evidence

Continue Folio toward everyday commercial-editor usefulness by making the existing text-to-paragraph transition retain the user's edit. Accepted 0.35, its archive, launcher and all original PDFs remain available. User standing autonomy applies; implementation choices below are the implementer's decisions, with no routine permission gate.

The original quiz contains adjacent source text objects inside a single visual heading. Single-object growth correctly refuses overlap; paragraph editing is the existing route for moving following words. The packaged quiz history already validates paragraph insertion, Preview/Apply, 100 Undo/100 Redo and saving. The current ExistingTextDialog keeps replacement/font locally, but onEditParagraph takes no arguments and App moves only the original request into textParagraph. That transition drops the replacement.

## Chosen approach

Carry a source-bound draft into the existing paragraph editors, and use a new read-only native mapping operation to locate the selected original object inside the checked paragraph. The native inspector already has exact source ranges for fresh paragraphs. Verified saved styled/uniform replay must retain equivalent range provenance through hard breaks, wrapping and trimming.

String search is insufficient: repeated text can occur in different objects, and edge spaces may belong to a source piece, a line break or a generated separator. Reconstructing this ownership from screen bounds would duplicate native layout rules and mis-handle mixed sizes. Keeping a separate copy-only buffer would retain bytes but would still make the user repeat the edit. The native mapping operation is additive desktop functionality; existing CLI/MCP paragraph responses, selection IDs and serialized layouts remain unchanged.

## Required behavior

- Clicking Edit paragraph after typing transfers those changes into the correct source piece of the suggested paragraph. No PDF mutation occurs until the existing Preview/Apply flow.
- Preserve surrounding text and attributed font, size, color and highlight styles. An explicitly selected substitute font follows the selected piece, never the entire mixed paragraph accidentally.
- Keep the original dialog and its exact input/font if preparation, mapping, validation or capacity checks fail. Report the failure there. No silent fallback that drops or misplaces input.
- An unchanged original-font edit takes the existing path without unnecessary mapping work.
- Preparation is owned by its original tab, immutable page/source, adapter and anchor. A stale reply cannot install into another document or page. Cancel/unmount releases temporary ownership.
- The transferred changes form one local paragraph Undo item. Reset paragraph restores the checked original; selection adjustment or unrelated rerenders must not reapply the transfer.
- Explicit Cancel keeps its existing meaning: discard the uncommitted paragraph session. Failure to enter that session must leave the original input recoverable.

## Native source mapping

Add a bounded read-only mapping capability for normal occurrence paragraphs and reusable-form paragraphs. Proposed adapter interface:

```ts
mapParagraphDraft?(sourceId:string, pageIndex:number, objectIndices:number[],
  expectedParagraphText:string, anchorIndex:number, expectedSourceText:string,
  kind:"uniform"|"styled"):Promise<ParagraphDraftMapping>;
mapFormParagraphDraft?(sourceId:string, pageIndex:number, objectPath:number[],
  objectIndices:number[], expectedParagraphText:string, anchorIndex:number,
  expectedSourceText:string):Promise<ParagraphDraftMapping>;
```

The result binds sourceId/pageIndex, ordered objectIndices, anchorIndex, exact original paragraph/source text and their retained character ranges. External offsets are UTF-16 positions with verified character boundaries; native assembly may keep UTF-8 byte offsets internally. The retained source substring must exactly equal its paragraph substring. Trimmed original prefix/suffix metadata must not be mistaken for neighboring or generated text.

Reuse occurrence/form projection, edit/copy permission, source identity, clipping/tag ownership and paragraph admission. Form mapping uses the already checked original object path and promoted anchor; private projection IDs never enter the workspace or response ownership. No render/font/admission check may be bypassed by the new metadata. Mapping creates no published source or output file.

Fresh mapping comes from paragraph_spans::Builder source ranges. Saved styled mapping follows the complete ordered logical stream through independently verified replay fragments: consume every fragment exactly and skip only logical LF hard breaks; reject unconsumed visible characters or unequal substrings. Existing measured layout preserves all non-LF characters, so no public layout-field change is required. If that invariant cannot be proved, carry private measured-atom provenance instead. Saved uniform mapping follows the verified row text/newline assembly. Keep this provenance internal alongside layout; do not add fields to serialized CLI layout/inspection payloads or trust unverified replay hints. Existing inspection/publication behavior must remain identical when mapping is not requested.

Use current paragraph limits: 4,096 UTF-8 bytes, 64 styles, at most 4,096 selected pieces for verified saved paragraphs (fresh admission retains its existing smaller limit), and existing form-path bounds. Validate all endpoints, membership, text equality and response ownership again in the frontend. A piece without a proved retained logical range remains a recoverable preparation failure, not an excuse to search for an equal string elsewhere.

## Draft and formatting transfer

Define a small shared immutable draft payload with original source text, replacement, input selection and optional FontInfo. Parent preparation holds the chosen font synchronously before the original dialog can unmount, and owns that lease until the paragraph session closes. Child history/preview requests retain their existing independent leases.

Project the replacement through the checked source range. Preserve typed changes while accounting for unchanged source padding trimmed by paragraph assembly. Preserve new/changed edge whitespace rather than silently trimming the replacement. Apply bounded grapheme-aware text edits to the existing attributed model so unchanged spans keep their styles. Repeated text within the selected piece uses deterministic edit alignment; source-object identity determines which piece is changed. Explicit font selection formats only that transferred piece and preserves its other style attributes.

Prepare the local paragraph state before closing the original dialog. Supply the checked initial selection and prepared draft to the paragraph component; avoid a late suggestion overwriting it. Keep the original checked text as native expectedText. Preserve baseline, area, alignment and spacing unless the user changes them or chooses Fit area to text.

Changed-draft transfer requires the styled editor capability, which the official native adapter uses for both uniform and mixed PDF formatting. Older uniform-only adapters retain their existing unchanged paragraph-entry behavior; a changed-draft handoff must fail before closing the original text dialog, with its input/font still available. This avoids offering a transfer that cannot retain local Undo/history or survive a subsequent piece-list failure. For styled adapters without range mapping, a single checked piece may use an exact whole-piece fallback when its source padding/text correspondence is proved; changed ambiguous multi-piece input remains recoverable rather than being guessed. The native read-only mapping API still supports uniform replay for library/automation consumers; that does not imply an unsupported UI handoff.

## Implementation boundaries

- Native: new paragraph draft DTO/helper, existing paragraph assembly/replay/layout provenance, occurrence/form request facades and two desktop commands. No renderer, dependency, security-policy or PDF writer changes.
- Frontend: shared paragraph-draft mapping/model helper, adapter capability, form adapter binding, ExistingTextDialog callback, App session/font ownership, initial state in StyledParagraphDialog. Leave the legacy uniform editor unchanged by removing the incomplete initial-draft installation found in review.
- Tests: pure source/mapping and formatting tests, App handoff integration, asynchronous ownership/font cleanup, focused native mapping, and actual Windows quiz handoff. Preserve failed runs and accepted builds.

## Acceptance

Prove a failing integration reproduction first. Cover repeated identical pieces, multibyte characters, source padding/generated spaces, hard/blank lines and saved replay, mixed styles/highlights, explicit font ownership, stale/malformed responses, form scope, source changes, failure/Cancel and one-item local Undo/Redo. Existing broad frontend tests, TypeScript/Vite and fresh appropriate native gates must pass; native code changes prohibit reusing the 0.35 native pass receipt as a fresh result. Keep original native test profiles and source/shared-fixture fingerprints explicit.

Build a separate optimized GUI candidate. On the original quiz, type into the leading heading fragment, observe the valid overlap refusal, enter paragraph editing without retyping, Preview/Apply and save/reopen. Verify original fonts, exact workspace Undo/Redo and unaffected glyphs/pixels independently. Also verify explicit font transfer and reusable-form handling with public generated fixtures. Use owned desktop processes, no overlapping compiler/resource observer, no injected CSS and no relaxed preservation assertions.

One builder check precedes implementation and one final scoped code review follows concrete verification. This does not promote a new combined package automatically or certify broad producer/script, printing, accessibility, low-memory, signature or commercial parity. Larger-document native latency remains the subsequent performance investigation.
