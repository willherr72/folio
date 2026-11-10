# Split the edited workspace into PDFs

## Intent and scope

Continue the user-authorized autonomous quality loop. The preceding paragraph workspace checkpoint is verified; batch splitting is an open organizing gap. This milestone exports contiguous parts of the current workspace as editable PDFs. Originals, workspace history and the current saved state stay unchanged. No release or parity claim.

## Approach and implementer decisions

Use the existing native Save composition for each part in one serial worker job. Repeated frontend Save calls would require many dialogs and lose batch cancellation/ownership. Raster export would discard editing and navigation. A separate low-level copier would duplicate the preservation and protection pipeline. These are implementer choices under the user's standing opt-out of routine approval questions.

The dialog offers every N pages, after selected page numbers, or at top-level bookmarks. Show part filenames and one-based inclusive page spans before choosing a destination. All partitions cover the entire current ordered workspace exactly once. Every N is an integer 1..10,000; N >= page count produces one clearly displayed part. Manual boundaries accept comma-separated numbers/ranges, deduplicate and sort; only 1..pageCount-1 is valid. Bookmark starts are valid level-zero internal destinations, sorted by workspace position and deduplicated; always retain pages preceding the first bookmark. No useful boundary after page one yields an explanation to choose another mode. External destinations and nested bookmarks do not define boundaries. Bookmark titles are not filenames. At most 10,000 pages and 1,000 parts; one-part output is allowed. No overlapping custom groups, file-size splitting, cross-part link rewriting or arbitrary naming templates.

## Native contracts and fidelity

SplitPdfRequest contains pages once, parts [{start,end}] using zero-based half-open indexes, and optional workspace bookmarks. Native validation requires nonempty contiguous parts from zero through pages.len(), unique page IDs and the existing page/overlay limits. Validate bookmarks against the full workspace before pruning each part with bookmarks::remap. Absent bookmarks use inherited_bookmarks once. Unreadable bookmark state blocks UI export until explicitly resolved in the Bookmarks panel.

Convert physical page plans to raw units once, capture font/image leases before queueing, and compose one part at a time with CompositionIntent::Save and flatten=false. Preserve current content, overlays, crop, rotation, forms, comments, applicable bookmarks, internal links whose targets remain in the part, and existing encryption. Links to excluded pages follow the existing extraction policy; do not invent cross-file actions. Validate every part and authorize every composition before creating an output folder. Explicitly require assembly rights for all protected sources even for one-part output. Existing protection-combination constraints remain authoritative per part. A denied later part must leave no folder.

## Publication and lifecycle

One Windows folder picker creates a unique Folio split-UUID subfolder; files are part-0001.pdf onward. Reuse the handle-bound OwnedImageFolder protocol with a private typed folder purpose and neutral incomplete marker text. Preserve image-export prefixes and ownership tests. No replacement of existing files, no recursive cleanup, no adoption of changed directories. Sequential composition releases each PDF buffer before the next; per-PDF existing 512 MiB limit and total batch 1 GiB limit. Cancellation checks between validation/composition/writing; native composition itself is not interruptible. Atomic commit decides whether cancellation or completion wins. Failure/cancel removes only owned outputs; unsuccessful cleanup reports the incomplete path and keeps/restores its marker.

Use a window-bound, single pending/running job registration with 120-second expiration for unstarted jobs, matching image export. UI freezes workspace snapshot in a modal, blocks competing document mutations, shows progress and Cancel export, ignores late/foreign progress and cancels on unmount. Cancel during the OS picker is observed after it closes. Completed exports do not change document dirty state or history.

## Acceptance

Pure frontend partition tests: bounds, ordering, duplicate boundaries, bookmarks and limit handling. Async tests: pre-registration cancellation, foreign/late events, dialog errors/busy cleanup and App snapshot. Native tests: malformed partition rejection, bookmark pruning, current edits/rotation/crop/forms/links, protected assembly denial before publication, cancellation after a part, unexpected files and late cancellation. Re-run shared folder regressions. Fresh frontend/native suites and a fingerprinted Windows build; desktop every-N/manual/bookmark previews, actual folder export, cancel and source/history preservation; independent reader inspection of editable outputs. One design check and one final scoped correctness review, then one verified correction pass. Retain failed evidence and list remaining gaps honestly.
