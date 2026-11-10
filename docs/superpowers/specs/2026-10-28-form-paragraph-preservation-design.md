# Form paragraph preservation through consistent extraction geometry

The user authorized autonomous work toward faithful existing-PDF editing. A
fresh native regression isolates a refusal affecting an otherwise editable
horizontal nested paragraph: promotion preserves page geometry, pixels and
physical glyphs, but PDFium changes a generated space into CRLF. Source text is
`Neighbor\r\nCafe old Cafe old`; promoted text inserts CRLF between the two
spatially separate occurrences. Complete-glyph preservation correctly refuses it.

## Cause and approach

The locked PDFium `CPDF_TextPage::ProcessInsertObject` compares raw text-object
rectangles from different Form coordinate systems in its early horizontal line
test and later overlap exception. Other position calculations already compose
the Form matrices. A source Form and its page-promoted equivalent can therefore
produce different generated separators despite identical actual glyph geometry.

Three approaches were considered. Ignoring generated glyphs in publication would
weaken the existing proof and is rejected. Retaining/reconstructing Form wrappers
would require a new paragraph publication and replay model while preserving the
renderer inconsistency. Correcting the coordinate comparison at its source is
the selected approach: no Rust preservation condition is relaxed.

The experimental candidate maps the incoming rectangle and position into the
previous Form's coordinate space when both Form matrices are positive,
axis-aligned and the existing writing-mode classifier is horizontal. Equal Form
matrices retain the exact old path. Vertical, reflected, rotated and sheared Form
heuristics are outside this correction; page rotation is separate and is tested.
The later horizontal overlap exception must use the same normalized coordinates.
Before integration, verify finite/invertible transformation handling and ensure
the candidate does not add an unbounded allocation or recursive traversal.

## Requirements

- Keep exact geometry, raster and complete-glyph checks in paragraph preparation.
- Preserve legitimate same-line spaces and separated-line breaks, including mixed
  root/Form text, repeated Forms, unequal positive scales and inherited fonts.
- Preserve actual glyphs, their positions, rendering and source bytes. Generated
  separators may become more accurate on original documents under the corrected
  renderer; extraction results are not promised byte-identical to the buggy DLL.
- Keep existing non-axis-aligned behavior unchanged unless a separate reproduced
  defect warrants a new decision. Do not claim all native paragraph refusals fixed.
- Maintain source locks, patch hashes, build provenance and distribution notices.
  A diagnostic DLL cannot become the installed renderer solely because one case
  passes. Existing renderer probes, native regression and fresh desktop/CLI/MCP
  saved-output acceptance remain required.
- Preserve the accepted portable0.22 and original private quiz. No commits, push,
  release, destructive cleanup or parity claim. Keep the overarching goal active.

## Current evidence and limits

`artifacts/form-paragraph-preservation-20261028` retains baseline RED, projection
PDFs/pixels/full glyphs, a separate renderer candidate and its patch. The cached
renderer source was restored after building; installed DLL/lock/provenance remain
unchanged. The disposable candidate fixes the source/promotion comparison and
allows CLI paragraph discovery, unchanged save and added-word save with independent
MuPDF text/raster checks. Twenty-four synthetic layout comparisons pass; three
baseline text mismatches disappear. Wider input classes and integration are still
pending. These are experimental results, not accepted release evidence.

## No-op identity correction discovered during acceptance

The prior nested automation design requires unchanged operations to retain stable
content identity. The acceptance probe now reproduces eight failures across CLI
and MCP, page and nested selections, and single text and paragraph edits. Each
unchanged request passes native validation but unnecessarily rewrites the PDF;
paragraph publication also promotes nested text. Source-bound selection IDs then
change. Exact original-file preservation is the intended no-op behavior.

Use the existing native validation, then publish the exact immutable input for a
proven request with no changes. A text no-op compares replacement against genuine
source text (not a generated extraction separator). A paragraph no-op requires
identical ordered characters and source style IDs, equivalent layout and fit:false.
Splitting an unchanged style run or adding empty runs does not create an edit.
Compare flattened character/style iterators without constructing another string.
Bounds may be omitted or exactly equal to inspected bounds. Alignment and spacing
may be omitted, or exactly equal to a known inspected value; an explicit value
when the source value is unknown is an actual layout request. Equal text with
different source style IDs or changed layout remains a normal edit. Source lookup,
permissions, paragraph membership, style validation and native publication proofs
are not bypassed. No approximate float comparison authorizes unchanged output.

Copying engine.source_bytes directly is unsuitable: protected documents retain
decrypted editing bytes there. Reopening the user's original path at publication
is also unsuitable because that file may have changed since inspection/opening.
Retaining another potentially 512 MiB memory buffer in every engine document is
unnecessary for this automation-only correction.

Instead, capture a bounded private on-disk snapshot for text/paragraph preview
requests before opening the engine source. Open that snapshot and retain its
file proof through publication. For an unchanged request, copy the same snapshot
to the candidate with hash verification; for a changed request, keep normal
native export. The snapshot holds ciphertext for encrypted inputs, so unchanged
copies retain their exact protection bytes. It is temporary, is never returned
to the client, and is removed on success/error when the request ends. On Windows,
use FILE_FLAG_DELETE_ON_CLOSE as existing MCP input snapshots do: worker termination
must remove the file even when Rust destructors do not run. A child-process exit
regression must prove this. Unix files use mode0600 and normal RAII cleanup; forced
Unix termination cleanup is not established by this Windows milestone. This adds
one bounded disk copy per text/paragraph preview, avoiding another full memory
buffer. Other commands and form-value previews retain their existing routes.

This design choice is made under the user's autonomous-work authorization.
The original full native gate remains frozen until terminal evidence is read.
Then add native regression coverage for private snapshot lifetime, source-path
mutation, bounded copies, exact run/style/layout detection, and protected CLI
no-ops/permission refusal. Re-run the eight transport cases against a fresh CLI,
including same-selector subsequent edits and MCP ownership after original close.
