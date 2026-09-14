# Explicit adjacent text groups — issue15 B2

Stage B2 of issue #15 extends v0.11.0’s positioned single-run editing with explicit adjacent-group selection. Implementation uses the isolated adjacent-text branch.

## Product behavior

The existing single-run dialog gains **Edit together…** when native group APIs are available. A separate modal shows nearby source pieces with checkboxes; the initially selected piece stays selected. Users explicitly select 2–8 pieces and choose **Check selection**. Only a verified selection exposes its combined original text and replacement field. **Apply changes** commits one immutable source and one undo step. Cancel leaves the PDF untouched. Keep single-run substitution and editing unchanged. Groups use the original PDF font; no group substitution or shaped existing-text editing in this stage.

## Native contract

New inspect_text_group(source_id,page_index,object_indices) -> TextGroupPreview {object_indices,text,font_name,font_size}. New replace_text_group(source_id,page_index,object_indices,expected_text,replacement) -> DocumentInfo. Maximum8 selected pieces, minimum2; require sorted distinct consecutive top-level page-object indices, bounded single-line Latin/ASCII, same actual PDF font token/resource, size, color, rendering mode and affine linear matrix. Reject explicit Tc/Tw/TJ spacing pages in this initial group path; B1 remains available for individual runs. Keep catalog/encoding/graphics preflight exclusions. Reject marked content and original tagged-document structure (StructTreeRoot/StructParents); a raster proof cannot establish preservation of tag semantics.

Use characters assigned to each selected text object, never concatenated FPDFTextObj_GetText strings: that API can duplicate a boundary space. Generated or ambiguous Unicode cannot be silently invented or normalized. Build a private candidate by setting the first object's text to the logical concatenation and removing subsequent selected objects in descending index order. Preserve the first object's transform. Before admitting a group require unchanged whole-page raster and every character's Unicode/origin/box, first font/style and unchanged unselected runs after deterministic index adjustment. This lossless proof establishes compatibility, not proximity or a familiar font name. Non-contiguous, interleaved, differently styled, offset or ambiguous groups are rejected with actionable errors.

Inspect drops all private candidate bytes/resources and publishes no source. Applying repeats the proof and expected-text check; use a private temporary source to reuse the existing single-run replacement validator (fit/collision/neighbor/raster/encoding/protection). Always remove this temporary source after success/error. Publish only the final immutable source; original path protection, duplicate pages, undo/recovery and cancellation/orphan cleanup remain intact. Group no-op must preserve original visible and logical content.

## UI lifecycle

Modal selection changes invalidate previous verification. Draft survives apply errors. Inspection has a per-request token and unmount guard; late responses cannot restore a cancelled dialog. Apply uses the existing operation lock and captured source/page/tab identity, and closes orphan results exactly as single-run editing does. All tab switches, file operations and workspace shortcuts respect modalOpen. Selection is keyboard accessible, bounded and explicit; no automatic grouping from visual proximity.

## Verification and release

Test split words with spaces on either side of boundaries, split-within-word, genuine adjacent versus different rows/fonts/resources/transforms, generated/doubled spaces, interleaved graphics, stale text/indices, shorter/equal/longer/no-op, cropped page rotations, source isolation and closed temporary resources. Use native glyph/raster/reference export checks plus independent MuPDF/pypdf extraction/rendering. UI tests cover explicit selection, stale/cancelled inspect, one apply/undo, error draft retention and legacy behavior. Package a practice PDF and exercise native Windows dialogs, group edit/search/save/reopen/recovery. Keep #15 open for B3 nested-form occurrence isolation and any explicit B2 refusals; do not expand #14 script claims.
