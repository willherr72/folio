# Structural area redaction

## Intent and authority

Continue the autonomous Folio quality goal with real area content removal, preserving editable content elsewhere. These are coordinator decisions under the user's standing instruction to proceed without routine permission questions. They are not claims that the user individually selected each technical choice.

The accepted 0.30 package remains intact. Its live product fingerprint was rechecked as `4bb96e1b1e2b023dcf5040885a52f1baa6243b0fb6f9b24edeb6de596abf284c` before this work. Existing Remove text remains text-only. No area command is exposed by the first foundation milestone.

## Approaches

1. Cover marks with rectangles: fails removal because underlying content remains recoverable.
2. Rasterize the whole document: loses editable outside content and is not the intended product behavior.
3. Rewrite admitted content into a fresh bounded graph: selected approach. It builds on the current canonical text-removal kernel, supports independent structural verification, and can refuse an unsupported document without changing it. Compatibility must expand through verified cases rather than silently dropping unknown content.

No new third-party runtime or dependency is introduced. Rust/lopdf/PDFium remain the native stack; React remains the UI stack. MuPDF may be used by existing independent audit tooling, not introduced into the distributed application.

## Current engine and integration boundary

`redaction_core.rs` prepares only a strict page/content/resource subset and publishes a fresh graph. `redaction_ranges.rs` removes source-bound glyph operands while retaining advances. `redaction_selection.rs` binds native glyph measurements to source glyphs. `redaction_graphics.rs` admits bounded opaque graphics states. `redaction_preview.rs`, `redaction_composition.rs`, and `redaction_host.rs` own composed current-document snapshots, previews and adopted results.

Current restrictions include unrotated origin-zero media boxes, equal crop/media boxes, UserUnit 1, no images or reusable Form XObjects, no annotations/forms/tag tree, and bounded fonts and graphics. Existing text removal must not inherit more restrictive area-planner admission. Existing previews and package outputs must not change in this foundation cycle.

## Intended complete workflow

- A separate Redact areas action prepares the current edited document, including ordered page occurrences and pending edits, through the trusted host. Marks use physical page points and a well-defined projection into source coordinates; unsupported projections refuse before publication.
- Marks are reviewable, clearable and revisable. Preview shows removed glyphs, affected images and complete affected drawings, plus any effects beyond the requested rectangles. There is no silent clipping-only fallback.
- Apply adopts exactly the previewed immutable candidate. Cancellation, invalidation and window destruction release unadopted candidates. Reuse the existing owner, byte, count, TTL, source-hash and one-shot safeguards.
- Undo retains the original in the local editing session. Saving the sanitized copy creates a fresh file without previous revisions; original source files are preserved. Recovery and Undo are not advertised as sanitized stores.
- Saved output is explicitly a sanitized copy. Metadata, attachments, hidden alternate text, bookmarks, forms, annotation appearances and optional content must each be preserved safely, sanitized by an explicit policy, or cause refusal. No generic raw graph import is permitted merely to keep the page looking right.

## Content policy

Text: remove each intersecting glyph's source bytes, preserving the advances and positions of retained glyphs. Native measurements must be bound to the same source. Hidden or clipped text cannot escape removal because it is not currently visible.

Drawings: initial policy removes an entire path object when its conservative painted bounds touch a mark. This includes all its subpaths; preview must disclose the resulting full extent. This can remove a border or background beyond the mark. Curve control hulls, transformed stroke widths, caps and miter joins require conservative bounds. Later exact splitting may reduce collateral removal, but is not required for the initial policy. Discarded nonpainted paths are also removed if selected, because their operands can retain shape information. An input clip is not a sanitization technique. The first planner refuses clipping, hairline strokes and graphics-state changes inside an unfinished path rather than claiming complete semantics.

Images: remove affected pixels in decoded image samples and re-encode a fresh image without original compressed bytes, metadata or unused alternate/mask resources. Isolate occurrences when resources are shared. Map page marks into pixel space conservatively, accounting for reflection, rotation, interpolation and masks. Remove a whole image if that is the supported explicit preview policy; never retain covered pixels behind a mask. This is a separate implementation milestone, not present in the planner milestone.

Reusable forms, patterns, shading, transparency, optional content and tagged semantics: require dedicated bounded traversal and sanitization. Until implemented, refuse the document without adoption. An untouched page's unknown content is also not silently discarded. Protected documents remain refused until equivalent permission-preserving sanitization has its own proof.

## Foundation milestone: drawing planner

Implement a private pure Rust planner in `redaction_area_paths.rs`. It consumes typed commands emitted by a future strict content adapter and rectangles in PDF bottom-left coordinates. It has no PDF parsing, filesystem, renderer, publishing or UI authority. `PathCommand::Unchanged` is reserved for adapter-verified commands that cannot affect path geometry or graphics state; unknown commands must map to `Unsupported`.

`plan_paths(&[PathCommand], &[AreaRect]) -> Result<Vec<PathDecision>, PathError>` returns inclusive command ranges, conservative bounds, painted/nonpainted status, removal status, and whether a removed bound is not contained in any individual mark. The last flag is conservative: separate marks jointly covering a drawing may still produce a disclosure. Geometry is not a proof of absence of hidden content elsewhere in the graph.

Limits: 1..64 finite positive marks; at most 100,000 commands; at most 64 saved graphics states; all input numbers and transformed coordinates finite and absolute value <= 1,000,000. Rectangles require representably increasing endpoints. Matrices require bounded coefficients, maximum absolute linear coefficient >= 1e-9 and normalized determinant magnitude >= 1e-12. Directed interval arithmetic propagates rounding uncertainty through concatenation and transformed points; subnormal products and nonzero subnormal sums refuse. Nonzero strokes are bounded with the transformed square envelope using half-width times max(2, 2*miter-limit) for miter joins and an outward-rounded sqrt(2) for other joins, with L1 matrix row norms; this intentionally overestimates. Unsupported/invalid commands, dangling paths and unmatched save/restore reject the entire result. No partial plan is returned.

Path construction commands must be contiguous through their terminator; save/restore, matrix/style changes, text boundaries or other commands inside a path reject. This deliberately avoids pretending that the current path participates in q/Q state restoration. Boundary contact counts as selected. Construction bounds include move points and all curve control points. This may over-select sparse compound paths; the preview policy must report it.

## Verification and acceptance

Foundation tests run the actual Rust module directly, without mocks. Hand-computed fixtures cover retained and selected paths, disjoint drawings with overlapping envelopes, curve hulls, thick/miter/sheared strokes, reflected rectangles, matrix concatenation, q/Q restoration, malformed state, zero-width strokes, singular/near-singular transforms, overflow, boundary contact, marks and operation budgets. A separate sampled geometry oracle checks conservative containment using independently evaluated curves and stroke sample points. Failure cases return errors, never incomplete plans.

Integration acceptance later must combine operand-level absence, fresh graph reachability and unused-resource checks, independent text extraction and image decoding, fixed source-derived raster masks outside the disclosed change area, and saved-output reopen/re-edit tests. Black appearance alone never proves removal. Negative controls must reject an opaque cover, retained selected image bytes, copied old content streams and enlarged exclusion masks. Windows Cancel/Preview/Apply/Undo/Redo/Save/Reopen and tab ownership are required before UI acceptance.

## Milestones and limitations

1. This cycle: complete design and bounded drawing planner, reviewed and tested; no new end-user capability or package.
2. Strict content adapter and vector/text removal candidate with saved-output independent audits.
3. Image occurrence sanitization and complete graph admission, with decoded-pixel audits.
4. Combined host/frontend area workflow and Windows acceptance; package only after relevant regression gates.
5. Broader producers, clipping, forms, patterns, tags, protection and performance supported one proven class at a time.

The feature must never be labeled generally secure redaction while an accepted content type can survive marks. Commercial parity, broader access controls, physical printing and general resource-quality gates remain open.

## Standards reference

Adobe's published [ISO 32000-1:2008](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf), 12.5.6.23, requires removing the specified content and destroying affected image data rather than hiding it with clips or masks. This informs the architecture; it is not certification. The broad document-sanitization policy and bounded phased implementation above are Folio design decisions.
