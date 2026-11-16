# Page-wide replacement planning

This is the next dependency toward document Find/Replace, following the verified
atomic explicit batch editor. The broad goal still includes automatic document
planning, desktop preview/Apply with Undo, deletion, paragraph layout, and useful
producer/language coverage. Completing this dependency does not complete that
goal or make an inaccessible native API a finished user workflow.

## Intent and choices

Use the same page-wide Unicode17 matching as document Find. A match spanning
objects must remain visible, even when one native single-object edit cannot
apply it. Whole-word decisions must see neighbors in other objects. Never search
each text object independently and imply that those results equal document Find.

Executor choices under standing authorization: (1) reuse the native SearchMatches
iterator and original character ranges; (2) separate read-only planning from
mutation; (3) require exact character-to-source ownership before constructing an
edit. The alternatives are a separate per-run search (incorrect boundary and
cross-object behavior), or immediately redesigning paragraph publication and the
desktop workspace composition route (too many unproved interfaces at once).
The cost of this sequencing is that user-facing document Replace remains pending
while its mapping is built. No approval or design-review round is requested.

## Core interface and behavior

New `src-tauri/src/replacement_plan.rs`, under engine, owns the pure planner.
`PlanningRun` carries an object path, exact logical source text, and native
eligibility. `OwnedCharacter` carries one reading entry's text and an optional
`SourceRange {run_index,start,end}` in UTF-8 bytes of that exact source string.
These are internal trusted worker data, never client-supplied edit authority.
`plan_page(characters,runs,query,replacement,options,selected)` returns all Find
matches, selection status, precise mapping blockers and grouped whole-run edits.
`selected=None` selects all matches; `Some(indices)` selects those original
zero-based match ordinals, with duplicate/out-of-range indices rejected.

Map query ranges to original character entries, then require contiguous,
nonoverlapping source byte ranges belonging to one run, valid UTF-8 boundaries,
and exact equality between each reading entry and its source slice. Group all
selected changes to a run and build its final text once in source order, retaining
every byte outside selected ranges. Replacement is literal, without regex or
replacement expansion. Source and input data remain unchanged. Reject overlapping
selected source ranges instead of applying a text change twice.

Cross-object, unmapped/generated separator, ineligible source, whole-object
removal and paragraph-layout requirements remain explicit match blockers. A
source-empty final string is represented as requiring object removal, not silently
discarded or substituted with a space. New line breaks require paragraph layout.
These are pending mutation capabilities, not completion of deletion/reflow work.
Blocked selected matches or native edit budget violations produce no executable
edit list; an explicitly selected eligible subset may produce one. Nonselected
blocked matches stay in the report. An unchanged replacement produces no edit.

Bounds: query1..1024 UTF-8 bytes and non-whitespace after existing normalization;
replacement0..4096 UTF-8 bytes, no NUL; at most250000 reading entries,100000 runs,
1000 matches/page,16KiB/run logical text,13 path entries,16MiB total source text.
At most64 emitted changed runs and512KiB expected/replacement bytes; public page
plan JSON is bounded at2MiB before and after physical-point scaling. Overflow is
an error or explicit batch blocker, never a truncated successful plan.

## Native mapping and eventual consumers

The next task obtains reading order and glyph ownership from the native PDFium
objects. Match object handles to root/nested paths; shared invocations also need
their actual geometry to distinguish ownership. Duplicate/overlapping bindings
remain unmapped. Geometry alone cannot authorize a source string edit. Prove the
complete reading-to-logical-source sequence, including surrogate pairs, ligatures,
generated whitespace and rotated reading copies, before exposing source ranges.
Native inspection requires copy AND edit permission and one immutable source.
Existing conditional font/layout/graph checks still run at mutation time.

The public native page-plan method returns source fingerprint, query/options,
all match rows and candidate edits; no candidate is created. Later document
aggregation uses the original immutable source and existing batch32page limit,
with progress/cancellation and no silent result omission. Desktop integration
must bind the exact current workspace, including reordered/duplicated/imported
pages, crops, overlays, filled fields and bookmarks; it must not edit an old
backing source or use the metadata-removing redaction preparation route.

## Verification and boundaries

TDD model witnesses cover page-wide whole words, cross-object phrases, literal
replacement, UTF-8/casefold expansion boundaries, whitespace, subset selection,
multiple changes in one run, blocked all-or-nothing plans, no-op and byte limits.
Native fixtures cover roots, shared/nested occurrences, rotations/UserUnit,
generated separators, inaccessible ownership, permissions and unchanged source
bytes. Full native correctness and affected optimized tests precede a usable
consumer/binary claim. One final fresh component review follows both tasks.

Keep managed dirty worktree, previous checkpoints/packages/launcher, originals,
failed attempts and all evidence. No commits, releases, destructive cleanup,
clipboard/private-input actions, new dependencies or version changes. Native
commands are serialized with one Cargo job and the existing D: cache/temp paths.
