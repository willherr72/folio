# Existing text edits with document ownership

The current text editor isolates one page and publishes that page as a new source.
`acroforms::preserve_page_copy` correctly refuses a shared field because the
frontend replaces only one PagePlan. Removing that check would split field
identity, native links and actions. The previous page-replacement milestone
already provides full-source publication and occurrence mapping.

## Approach

Use an internal isolated page for the existing proven text mutation, then graft
its exact content/resources into an immutable complete source. Retain the
original annotation/form/link graph. Reuse occurrence duplication and mapping so
only the selected workspace copy changes. The isolated edited source is never
returned to the caller and is discarded on success or failure.

Alternatives considered: rewriting all text algorithms to operate on arbitrary
pages of full documents would remove isolation but expand a well-tested change
surface; dropping the shared-field guard would break linked fields. The selected
approach is the implementer's choice under standing autonomous authorization.

## Native milestone

Add `list_occurrence_text_runs(source_id,page_index)` and
`replace_text_occurrence(source_id,occurrences,selected_occurrence,object_index,
expected_text,replacement,font_id) -> PageContentReplacement`.
Keep legacy APIs unchanged. The new inspection may temporarily project shared
fields onto one page; only the new full-source transaction uses this projection
for mutation. Existing expected-text, font, glyph, collision and raster checks
must pass before publication.

Exact publication requires identical raw geometry and UserUnit; it does not fit
or transform text. Preserve bounded page-local paragraph memory strings from the
candidate rather than discarding them. Retain original interactions and current
workspace additions through the same ownership rules as page replacement.
Existing unsupported content/metadata restrictions remain in force. Protected
documents require editing; duplication also requires assembly. Internal candidate
policy must be derived from the destination selected page, never an unrelated
protected source. No intermediate source may survive a failed or abandoned reply.

## Acceptance and limits

Synthetic two-page shared text fields must remain a single fillable field after
editing, saving and reopening. Test unique, first and later repeated occurrences;
re-edit output, verify unchanged pages and source bytes, links/bookmarks, physical
units/rotation and refusal paths. Include native source-lifetime tests. Independent
reader evidence checks final text and interactions. UI integration, paragraph and
nested-form occurrence transactions are subsequent work; this milestone alone
does not remove the desktop limitation or establish commercial parity.
