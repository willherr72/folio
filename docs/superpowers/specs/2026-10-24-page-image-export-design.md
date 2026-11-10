# Export edited pages as images

## Intent and authority

Close the image-export gap in the everyday PDF workflow: save selected finished
pages as PNG or JPEG for sharing, documents, or image applications. The user has
authorized autonomous feature/test/review cycles and opted out of routine design
questions. Choices below are assistant decisions, not claims of user approval.
Prior checkpoint: artifacts/page-cropping-20261024/verification.json.

## Approach

Use native composition and PDFium rendering, preserving current overlays, native
annotations/forms, workspace rotation and per-occurrence crop. Do not capture the
screen or reuse capped UI previews: export resolution must be real. Compose/render
one page at a time; do not collect raster buffers for a batch. A new owned folder
under a directory chosen with the native folder picker avoids filename collisions
and overwriting source documents. PNG is lossless; JPEG uses an opaque white
background and explicit quality. No new libraries are required.

Alternatives considered: browser canvas export would duplicate native typography
and annotation rendering; individual save dialogs would interrupt batch work;
a PDF-to-image external executable would add deployment and fidelity dependencies.
The native folder batch is the selected implementation.

## User experience

Add Export images beside existing save/export actions. Dialog selects all/current/
custom ranges (existing selection semantics), PNG/JPEG, DPI (72,150 default,300,
600), and JPEG quality (1..100,default90). Show selected-page count, first selected
page's expected pixel dimensions and an explanation that images contain the
visible page including edits; text/form fields are no longer interactive.
Show validation before opening the destination picker. Exact output dimensions
are nearest-integer physical points*DPI/72, minimum1pixel, after crop/rotation.
Never silently lower DPI. Reject any page beyond24million pixels or12000pixels
per dimension with a lower-DPI message. The user can choose a smaller range.

Choose folder, create a new `Folio images-<random>` subfolder, write
`page-0001.png`/`.jpg` using original workspace page numbers (not selection index).
Success reports count and output folder; it does not mark PDF edits saved.
Show native per-page progress and allow cancellation between stages/pages.
Cancel/failure removes only the owned incomplete folder; cancellation never edits
or closes source documents. Crashes may leave an incomplete output folder; an
in-progress marker makes that state explicit. Successful output removes marker.
A native picker cancel returns without changing the workspace.

## Native contract and permissions

Add public PageImageOptions(format,dpi,jpegQuality), validation, and an engine
export method receiving current PagePlans, original page numbers, destination
parent, cancellation token and progress callback. Add CompositionIntent::ImageExport
requiring source-owned copyContent permission, distinct from printing and UI
preview; retain annotation/crop rights checks. Prepared-print registrations must
never be accepted. Caller-supplied protection or dimensions cannot authorize output.
Validate source geometry/unit and every selected plan/options before creating
output. Export is a content-copy operation, so assembly permission is not required
merely to export an allowed page image; print permission does not grant copying.
Mixed sources with different encryption may export only if each permits copying.
No decrypted intermediate PDF is written to disk.

Use the existing physical-to-raw boundary exactly once; final physical pixel sizes
must derive from source-owned UserUnit and composed PDF dimensions. PNG and JPEG
must render exactly the same visible geometry, annotation state and white page.
Bound encoded output per image to128MiB and aggregate batch output to1GiB; check
cancellation before/after composition/render/encoding/write and before success.
Reject empty lists, mismatched/duplicate/out-of-range labels, invalid DPI/quality,
invalid destinations and oversize pages. No source or existing output overwrite.
The worker is serial and owns PDFium; cancellation is an atomic token set outside
the worker queue, so it cannot be stuck behind the export request. Progress IPC
contains counts/stages only; do not expose source contents or passwords.

## State and lifetime

The App captures selected current page plans while its modal prevents edits/tab
changes. Native jobs get unique IDs with an application-owned cancellation map.
Register before publishing started progress; remove tokens on all terminal paths.
Progress receivers ignore other/completed jobs. UI cannot close during a running
job except through cancellation; ordinary unmount requests cancellation. Do not
modify history, saved digest, source IDs or recovery. Source registration ownership
continues to belong to the open document sessions.

## Acceptance

Native generated fixtures: physical units0.5/1/1.7/2, intrinsic/workspace rotations,
crops, overlays and native forms/comments, PNG exact pixels versus independently
rendered saved PDF; JPEG dimensions and bounded error; both formats decode through
independent readers. Check default/300/600dpi, hard resource limits, source hashes,
existing destination preservation, cancellation and injected failure cleanup.
Protection tests must reject copy-disabled but print-enabled PDFs even with forged
UI rights; permitted copy with print disabled works; prepared-print sources fail.
Frontend tests cover ranges/options, exact dimensions, errors/progress/cancel,
no double export, and unchanged history/dirty state. Build a fresh executable;
Windows acceptance exports edited/cropped/rotated pages to a chosen folder,
independent readers inspect saved images, and keyboard/narrow layout is checked.
Use fresh scoped review and record source/build/evidence hashes. No parity claim.

## Non-goals and retained gaps

No Office conversion, image extraction from PDF resources, transparency option,
TIFF/WebP, ICC output profile controls, or redaction. Image exports can still show
sensitive visible content and are not a redaction workflow. Real large-file/
low-memory hardware performance and full source compatibility remain audit gates.


## Builder-review dispositions (2026-10-24)

All seven contracts in artifacts/page-image-export-design-review/review.md are
accepted as implementation choices. ImageExport always composes flatten:false,
empty bookmarks and screen/view annotation visibility; does not impose assembly,
print or encryption-combination gates. Source rights and provenance still apply.
Preflight clamps the same far crop edges as publication; inspect composed units
and dimensions before allocating, render with set_target_size on both axes, and
verify bitmap dimensions. UI dimensions are labeled estimated until native checks.
PNG pHYs and JPEG density metadata record the chosen DPI.

Native request has pages, pageNumbers, workspacePageCount and options. Labels are
strictly increasing, unique, positive, within count<=10000; they are captured
workspace naming metadata, never permission/geometry authority. Repeated source
pages with distinct occurrence IDs remain distinct.

Use a short register command before start; bind the single admitted job to its
window. Cancellation atomically wins from running to cancelled; final commit
atomically wins from running to committing. Accepted cancellation prevents success;
after committing begins, output is allowed to complete. Check cancellation after
non-preemptible PDFium/encoder stages. Registration/unmount/window teardown clean
job tokens; no registry lock is held across callbacks or work. Native picker cancel
and typed outcomes distinguish cancellation from completed output. Terminal return
is authoritative, progress may be dropped. Progress includes the job ID.

Hold independent image leases without broadening provenance and a job-specific
FontRegistry snapshot consumed by composition, so global font teardown cannot
invalidate an admitted batch. Sources are stable during the single worker request.
Missing resources before snapshot admission fail without output.

Canonicalize the parent and exclusively create a fresh child. Lock the owned
Windows directory against rename/deletion while exporting; track created file
identities. Cleanup is nonrecursive: remove only tracked same-identity files, then
the empty owned directory. Unexpected contents, identity changes or deletion
failures retain the incomplete directory and produce an explicit cleanup error and
path. The marker is best-effort crash evidence; no automatic prefix scavenging.
Flush/close each file before commit, remove the marker as publication, and surface
any failure. Encoded limits are enforced during writes; aggregate counts encoded
image bytes (marker excluded). JPEG acceptance is mean absolute RGB channel error
below3 for the defined synthetic90-quality corpus, plus exact dimensions and
independent decode; PNG pixel equality uses an identical render configuration.


## Implementation review corrections (2026-10-24)

F1: Windows cleanup marks the original retained file handle for deletion with
SetFileInformationByHandle, never deletes a checked pathname. It also reports a
replacement still present at that name. Directory ownership stays locked through
handle-based final deletion. API reference:
https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle

F2: Late directory-removal failure restores the incomplete marker with create_new
while the original directory is still locked. Existing unexpected entries or marker
replacements are preserved and errors surfaced. DELETE access on the retained directory
handle excludes later handles lacking delete sharing.

F3: Cleanup responsibility is established immediately after exclusive creation.
When identity setup fails, fail closed and report the primary error, leftover path,
and inability to establish ownership. Never use recursive temporary-directory cleanup.

F4: PNG requests use valid default quality 90 independently of an invalid hidden JPEG
draft. JPEG still validates its draft.

Platform boundary: this first image-export publication implementation requires Windows.
Other platforms fail before creating output until an equivalent safe ownership/deletion
protocol is implemented. Cross-platform support is not claimed.

Second review corrections: F5 closes the create/open replacement gap by using
Windows NtCreateFile with FILE_CREATE and FILE_DIRECTORY_FILE. Exclusive creation
returns the original directory handle already excluding delete sharing. Canonical
verbatim DOS/UNC paths are converted to NT namespace paths; existing entries fail
without adoption. Reference: https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile
F6 restores missing markers based on actual path identity, closes stale tracking,
and uses create_new without overwriting any replacement. File handles permit reads
and renames, but exclude concurrent writes while the export is active.
