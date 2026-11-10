# Source-bound paragraph reflow automation

Build paragraph replacement through CLI/MCP, preserving original attributed
formatting and reusing native complete-source layout, fit and publication proofs.
This follows verified paragraph inspection; overall commercial quality remains
the active objective, not a claim made by this milestone.

Coordinator choice under continuing autonomous authorization: reuse the desktop's
styled request and immutable preview bundles. Flattening text would lose styles;
inventing layout in the CLI would bypass established preservation checks. Both
are rejected. Keep edits explicit and source-bound across disposable workers.

CLI `preview-paragraph INPUT --request EDIT.json --out-dir EXISTING` supports the
existing renderer and password-stdin options. MCP `folio_preview_paragraph` takes
documentId plus the edit fields, never a caller path. Edit schema: page (1..10000),
seedSelectionId (text-v1 SHA256), paragraphId (paragraph-v1 SHA256), runs (1..1024
strict {text,styleId}, at most4096 aggregate UTF-8 text bytes), optional bounds
{x,y,width,height}, alignment (left/center/right/justify), lineSpacing (0.6..3), and
fit boolean (default false). Maximum serialized edit32KiB. No unknown fields or
explicit nulls. LF line breaks only; native Unicode/glyph/grapheme proofs remain.
Style IDs must belong to the re-inspected original palette. New font/resource IDs
and style substitutions are not part of this request.

Resolve the seed against loaded source bytes with copy/edit rights. Re-inspect via
the complete-source native suggestion, then require exact paragraphId equality.
Build native StyledParagraphLayoutRequest with original object membership and
expected text. Omitted controls use source bounds, inferred alignment/spacing or
the desktop defaults left/1.2 when inference is absent. fit:true invokes native
fitting before preview; report the actual final bounds/controls. fit:false keeps
the requested/source rectangle and reflows inside it or refuses safely. Neither
mode moves unrelated text or shrinks font sizes implicitly.

Use preview_styled_paragraph_occurrence with original full page order. Export the
new immutable full-source candidate, reopen with original credentials, and render
before/after at bounded1200px via render_page_plan. Original/page graph/protection
preservation remains native policy. Reuse exclusive exact-byte Save and private
Apply/Close/Discard. `save-text-preview` remains the backwards-compatible CLI
publisher for both bundle kinds (document the historical name).

Keep text manifest v1 unchanged. Paragraph manifest v2 adds optional paragraph
metadata (seed ID, attributed runs, actual bounds/alignment/spacing); top-level
selectionId becomes paragraphId, plain replacement equals concatenated runs.
Validate version/metadata combinations and all bounded fields before Save/Apply.
Existing fixed filenames,64KiB manifest,512MiB bundle/session preview budgets,
hash validation, no-clobber output and cleanup remain. Supervisor reads bounded
metadata only; expensive native/copy/hash work stays in a disposable worker.

Acceptance: CLI true multi-line wrapping and repeat edits; mixed fonts/colors and
verified highlights; stale paragraph/source/style and invalid-input refusal without
staging residue; exact source hashes/other-page pixels/bookmark/note retention;
protected owner CLI flow; v1 compatibility and v2 tamper rejection. MCP preview,
apply, re-inspect, edit, save, discard/close in one session, strict schema, existing
capacity and cancellation ownership. Portable official SDK legacy/modern on public
Word PDFs plus generated mixed/highlighted fixtures, with independent text/pixel
verification. No changes to native admission or desktop UI. Nested reusable-object
automation and MCP password entry remain later workflows.
