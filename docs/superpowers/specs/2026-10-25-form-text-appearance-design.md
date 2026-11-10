# Existing text-field appearance

User authorization: continue the PDF editor autonomously toward everyday Adobe/Foxit usefulness. These are coordinator implementation choices, not a newly requested user scope. Implement after the form-color checkpoint finishes its native gate. Existing isolated worktree; no commits, publishing, or replacement of older executables.

## Outcome

In Fill forms, select a text widget and choose **Text appearance…**. Show its original font name, physical font size (or Auto fit), text color, and Left/Center/Right alignment. Prepare a native preview, then Apply as one Undo step. Later filling and save/reopen retain the chosen appearance. The selected widget changes; shared values remain linked and sibling widgets retain their own appearances.

Support ordinary single-line, multiline, and comb text fields, including empty fields with valid default fonts. Keep the original font program. Source objects, field names/labels/defaults/flags/actions, form relationships, annotation geometry, page content, and sibling appearances are preserved. Support source page rotations, UserUnit, cropped display pages, repeated page occurrences, and existing document protection policies.

## Approach

Use local widget DA/Q overrides plus isolated appearance regeneration and a bounded source-font proof, following the existing reflow/publication path. A whole-form refill risks changing sibling artwork. Editing AP text alone fails to persist settings for later filling. Neither alternative satisfies the selected-widget contract.

Extract reusable inspection/regeneration helpers from form_reflow only where required; retain its saved-artwork font policy and existing tests. The new settings operation resolves the native effective font from widget/field DR, AcroForm DR, or valid page resources, using the renderer's precedence. If saved artwork demonstrably binds a conflicting font, decline rather than silently switching it. Empty widgets without AP may use a valid effective default font; nonempty custom artwork without a verifiable matching font needs an explicit unsupported message. Generated font graphs must equal the bound source program. No silent substitution.

## Settings and validation

Inspection returns immutable-source style identity, original font label, physical font size (0 means Auto fit), RGB display color, original device color-space kind, and alignment. Resolve indirect/inherited objects with cycle/depth/retained-byte limits, strict DA parsing (at most64 KiB), exactly one valid Tf, and supported g/rg/k color operators. Invalid/ambiguous settings must not be converted to plausible defaults. Existing size outside the control range can be retained when changing another setting.

Requests contain annotation index, expected value, expected source-display bounds, expected style key, and optional changes. Missing size/color/alignment means preserve the corresponding original setting. Explicit size0 requests Auto fit; fixed sizes must be finite physical1–512 pt. Color changes are explicit #RRGGBB device RGB. The dialog defaults to Original color (with gray/RGB/CMYK label); an explicit Custom RGB choice enables the color input. Its RGB swatch is only a display approximation for an original CMYK color, never an automatic conversion. Alignment is Left/Center/Right. Size-only and alignment-only changes retain original gray/CMYK color instructions. Physical size converts through UserUnit once, with endpoint-tolerant validation. Reject an empty change or stale style/value/bounds.

Apply settings to the selected widget's own dictionary; never mutate a shared DA string, shared parent setting, resource dictionary, or AP dictionary in place. Preserve unedited settings semantically. Generate the unchanged value in isolation, import only the normal AP with existing byte/object budgets, preserve other AP states, and prune obsolete unreachable graphs. Existing source-font and border refusal rules apply; expose the native reason in the dialog. Password, rich-text, file-selection, hidden/locked/fixed-view widgets and ambiguous aliases remain explicitly unsupported for this operation. This is not a change to ordinary filling admission.

## Ownership and UI

Use immutable PageContentReplacement with the existing occurrence mapping. Require edit-content and annotation rights; preserve protection policy, bookmarks, overlays, links, comments, recovery and page indices. Inspection is read-only and requires the same authorized source. Candidate documents have one owner; foreign/borrowed source IDs are rejected, stale/closed candidates released, and Apply transfers ownership once.

Disable appearance entry while the value draft is dirty. Show **Original font** as read-only text; font substitution is separate later work. Numeric/color/alignment changes immediately invalidate preview. Reset restores all draft controls even if the underlying numeric value was already original. Apply requires the current raster to be ready. Errors remain visible, keyboard focus stays usable, and narrow640px controls/preview remain reachable. No automatic preview loop for this first dialog.

## Acceptance

Native tests cover visible changed artwork and persisted effective settings, exact unselected APs and page pixels, complete values, shared identity, metadata and actions, physical size at UserUnit1/1.7/2 and all rotations, repeated page isolation, empty fields, embedded/default font identity, same-alias font conflicts, preserved gray/CMYK on size-only change, Auto fit, alignment, protected inputs, no-op/stale/invalid requests, and bounded repeated edits. Compare saved output with independent readers.

Frontend tests cover request units and changed-only payloads, invalid inputs, dirty-draft entry, stale preview/raster results, borrowed source rejection, cleanup/Apply ownership, reset, error visibility and source/tab changes. Desktop acceptance exercises preview/Apply/Undo/Redo, subsequent fill, save/reopen, narrow controls and unchanged sibling appearance. One final scoped review; reproduce/correct findings without a rereview. Keep broad corpus/performance/accessibility and commercial parity limits explicit.
