# Images on existing PDF pages

The user authorized autonomous development toward everyday Adobe/Foxit usefulness.
Checkpoint0.16 is preserved. This feature adds image placement and retained editing;
existing-image replacement/crop/stacking remain explicit subsequent capabilities.

## Intended workflow

Choose a local PNG/JPEG, place it on an existing page, move and resize it using
handles or numeric controls, lock aspect ratio, rotate by quarter turns, remove it,
and undo/redo. Save/reopen and crash recovery retain editability without the original
image file. Preview/export/printing must agree, preserve transparency and respect
document permissions. Original PDFs and imported image files remain unchanged.

## Architecture

Use content-addressed image assets and a new image overlay. Retain original encoded
bytes, including alpha, bit depth, profiles and orientation metadata. Decode only
through bounded PNG/JPEG admission. Do not reuse image_pages' white alpha composite.
Animated PNG requires an explicitly chosen still frame; do not silently use one
frame while the browser animates the original source.
Preserve16-bit PNG samples in the asset/decode path; downstream appearance work must
explicitly verify profile/color handling instead of silently discarding it.

Each acquisition receives a unique native lease. Duplicate imports share immutable
bytes but hold independent leases. Releasing a cancelled import cannot remove another
owner's asset. Sources, pending dialogs and workspace histories must each retain their
ownership until release. Native operation Arc borrows survive registry release.

Portable image appearances will use an embedded image resource and an alpha mask
where needed, plus bounded versioned Folio metadata. Reimport must prove the full
appearance/resource graph matches the editable geometry and original asset, following
the existing portable-text proof. Mismatched third-party edits remain native.
Recovery stores image assets through current-user DPAPI and validates identifiers,
lengths/checksums and references before restoring. User-facing image controls arrive
after persistence/recovery and ownership are integrated.

## Resource policy

At most64 MiB encoded bytes and32 million pixels per image; dimensions1–32,768.
PNG receives a best-effort256 MiB decoder allocation hint. JPEG's decoder has no
total-allocation limiter: pixel/dimension admission bounds its output, but progressive
coefficient buffers can exceed256 MiB together with output. This is not a per-decode
memory ceiling. Registry holds at most64 distinct images,256 MiB
encoded bytes and512 MiB aggregate decoded sample size, plus4096 active leases.
Admission is atomic for a batch and deduplicates identical encoded sources. These
are resource admission limits, not a claim of a whole-process memory cap. Original
bytes are the canonical source, avoiding repeated lossy JPEG encoding.

## Phases and acceptance

1. Native immutable assets and lease ownership: PNG alpha/16-bit samples, JPEG EXIF
   orientations, malformed/oversized input, bounded path reads, duplicate owners,
   stale/idempotent release, concurrent acquisitions and atomic capacity rejection.
2. Native image overlay and portable proof: crop/page rotation, alpha and image
   orientation, color/profile handling, editable and flattened output, roundtrip
   tamper refusal, permissions, immutable sources and private recovery. Independent
   readers must verify pixels inside/outside image areas and unchanged native text.
3. UI placement, properties/drag/resize, asset lifecycle and undo/redo; cancellation,
   tab/Find races, keyboard controls and real Windows save/reopen/recovery acceptance.
4. Fresh integrated review, complete relevant checks and a verified portable build.

Adding a registry alone is not a user-facing image feature or completion of this spec.
The broad parity objective remains unchanged.
