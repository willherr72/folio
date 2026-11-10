# Combined selected-text and drawing publication

Continue the structural area-redaction design with actual saved PDF rewriting. Standing autonomous authorization applies; this is still private backend integration, not a complete exposed area-redaction feature.

## Architecture

Use the existing source-bound text-range removal and fresh-graph publisher. Add `redaction_area_content.rs` with `remove_planned_content(doc, text_ranges, marks)`. First validate the original document's strict subset and page marks. If ranges exist, run the existing text-range remover; it already proves retained source glyphs and advances. Reparse that bounded intermediate into a canonical graph, then plan and delete whole selected path operand ranges. Reopen the final fresh PDF and verify all text remaining after the first stage has identical glyph codes, Unicode, page/block ownership and positions.

This two-stage approach avoids reusing old operation indices after text rewrites split Tj/TJ operands. A single combined pass would reduce allocations but require refactoring the established text-removal verifier; defer that optimization until measurements justify it. Intermediate bytes never become an adopted document or external file. Input Document remains immutable. Limit every parse/output to the existing 64 MiB envelope; preserve existing aggregate font/content limits. New path planning uses the existing 100,000-command/64-mark bounds.

`PageAreaMark { page_index, rect: AreaRect }` uses bottom-left PDF points in the currently admitted origin-zero, unrotated, UserUnit1 page subset. Marks must fit the page and be finite/nonempty. `RemovedPath` reports page, complete conservative extent, painted state and collateral-extent flag. At least one text glyph or path must be removed. Metadata omission disclosures from both stages are combined.

## Strict adapter

Map every canonical path and geometry-state operation explicitly, retaining one typed command per operation so planner ranges remain exact. Supported path operators: m/l/c/v/y/re/h, S/s/f/F/f*/B/B*/b/b*/n. Map q/Q/cm/w/J/j/M. Text and color operators, plus validated dash patterns, may be geometry-unchanged commands; they cannot occur inside an unfinished path. W/W*, gs, and any unknown operator refuse. The gs refusal avoids silently ignoring stroke adjustment or future graphics-state geometry. Current text-only removal remains unchanged and may retain its wider admission.

Validate every page, including unmarked pages, through the strict adapter. Unmarked pages use a whole-page validation mark only to validate syntax/state; their decisions are never applied. Preserve all their operations. Existing core admission continues to refuse images, reusable forms, annotations, tags, page transforms and unknown graph semantics. No clipping rectangle or black overlay is added to simulate removal.

## Boundary and limits

Text ranges here are trusted source-bound selections produced by the existing private mechanism. This API does not prove the supplied text ranges exhaust a geometric mark. A future engine/host adapter must derive all intersecting glyphs from the same immutable source and bind the combined candidate to those marks before exposing area redaction. Until images and all admitted content are handled, no new UI/CLI/MCP redaction claim is allowed.

Private methods and existing production workflows remain unchanged. No new dependency, version bump, package replacement or private-user-file use. Existing 0.30 package remains the usable checkpoint.

## Verification

Native regressions must exercise the actual full engine modules: combined range/path removal; a selected path spanning beyond a mark; multiple pages; repeated path removal/reopen; untouched glyph advances including quote-expanded and split TJ sequences; negative rectangles/transforms/strokes; nonpainted operands; metadata/unreachable stream stripping; stale ranges, invalid marks, unsupported clip/gs/image/page objects and no-change refusal. Original serialized source bytes must remain unchanged.

Generate owned original, expected, actual and second-edit PDFs. The independent oracle must compare complete extracted text and glyph geometry, decoded path presence/absence, output graph payloads, full-page rendering against an independently authored expected PDF, and exact untouched-page rendering. Keep masks source-derived; a generated drawing may not enlarge its own allowed region. Corruption controls must reject a painted cover retaining secret text, retained drawing operands and changed outside text. Relevant native redaction suites run serially. One final fresh scoped review, followed by coordinator corrections, closes this cycle. Full native/UI/packaged acceptance remains for the later end-user integration milestone.
