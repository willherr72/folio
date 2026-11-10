# Bookmarks as document content

Support reading existing nested PDF bookmarks, navigating internal page targets,
and adding, renaming, nesting, moving and deleting bookmarks. Bookmark changes are
ordinary document history and dirty state. Titles, hierarchy, initial expansion,
bold/italic/color and destination view parameters survive export/reopen. External
and other action bookmarks remain visible and are preserved without executing
their actions inside Folio. New bookmarks target the current page with Fit view.

Use a bounded flat preorder array (level 0 roots) with stable bookmark IDs.
Destination/action values use a portable PDF-object representation: indirect
objects are resolved, page references become stable editor page IDs, streams retain
their encoded contents and dictionary. Named local destinations resolve to explicit
page destinations during import, including GoTo actions. UI navigation follows the
primary internal destination; unusual actions remain preserved but not executed.

Source-native bookmarks use page-index string keys until the frontend maps them to
its stable page IDs. Reordering and source-text replacement retain those IDs.
Duplicating a page leaves existing bookmarks attached to the original occurrence.
Appending PDFs appends their root bookmark trees. Deleting/extracting pages removes
destinations to omitted pages; retain a parent as a heading when its children still
have useful targets. Never export dangling page references. Undo restores both
pages and their bookmarks. Save/flatten/extract all carry the chosen bookmark tree.
Recovery stores the tree alongside pages, requiring no original PDF to restore it.

Native export preserves source bookmark trees by default for older engine callers.
An explicit supplied tree replaces the inherited tree, including an empty tree.
Bookmarks are written into the private export file before atomic publication.
Use explicit standard PDF outline objects, links and counts, with Unicode titles.

Bound import, export and recovery: 4,000 bookmarks, 32 outline levels, 4,096 UTF-8
bytes per title, 4 MiB portable values, 32 value-reference/container levels and
100,000 portable values. Reject cycles and malformed/dangling local destinations.
If existing outlines cannot be imported safely, allow reading the pages and show
the bookmark error; refuse silent bookmark loss on saving. The user can explicitly
discard unreadable bookmarks as an undoable document change and then save.

The sidebar switches between Pages and Bookmarks, preserves useful keyboard focus,
has named controls, and supports collapse/expand without marking content dirty.
Add/edit controls commit one history action; a keyboard-accessible page jump also
helps navigate long documents. Jumping to a bookmark closes Find to avoid a delayed
search response overriding the requested destination.

Acceptance: nested/style/Unicode/named destinations, external actions, move/delete/
duplicate/append/extract and source replacement; undo/redo, dirty status, native
save/reopen and crash recovery. Independent reader confirms destination page IDs,
hierarchy and display settings after reordering. Malformed/cyclic/oversized outlines
leave the source untouched and cannot silently disappear from an exported copy.
