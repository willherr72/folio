# Reuse unchanged paragraph source rasters

The user authorized autonomous development and opted out of routine design
questions. This is a coordinator-selected correction within the active quality
goal, not a claim that the resource failure is solved.

## Evidence and decision

The optimized 0.25 package passes functional desktop and CLI checks but exceeds
the unchanged 128 MiB process-tree growth budget during repeated paragraph use.
Dialog-only cycles grow about 55 MiB. Opening and cancelling the paragraph
workspace without any edit or candidate grows about 245 MiB and renders the same
unchanged source/page/600px combination 24 times. Complete editing grows about
309 MiB. URL instrumentation proves all created URLs are eventually revoked;
native memory remains comparatively flat. These facts establish redundant source
rendering, but do not prove that eliminating it resolves all GPU retention.

Reuse the existing PageView image cache for the workspace's unannotated original
page. A separate workspace cache would add another retention budget and duplicate
invalidation. Content-based candidate deduplication could hide the repeated-draft
test's cost while leaving distinct edits expensive, so it is outside this change.

## Behavior and ownership

Export the existing `usePageImage` hook with a minimal page-identity/geometry type
and an optional retry revision. Keep its source/page/pixel-width key, mounted
reference ownership, 60-entry/96 MiB limits and source-close invalidation.
ParagraphWorkspace acquires this shared original raster when overlays are empty.
Its existing serialized local renderer remains responsible for annotated source
pages and all candidate previews. Candidate retain/release and late-result cleanup
remain intact. Retry must still work after an original rendering failure.

Reopening the same source at the same width reuses its raster, including while
another view owns it. Closing/invalidation releases an idle raster and defers
release until the last mounted owner leaves. Changed source, page or pixel width
must request the correct new raster. Annotation composition must never use a raw
cached page in place of the composed image.

## Validation and limits

Observe failing reuse/ownership regressions before implementation. Cover pending
completion, invalidation, retry, annotations, candidates and width/source changes.
Run focused frontend regressions, the complete bounded-worker frontend suite and
production build. Bind a fresh local candidate to the changed source and measure
the original full 23-cycle workflow with unchanged assertions and budgets, then
the mixed and narrow saved-output workflows. A reduced render count is not a
memory-budget pass. Preserve the failed package, all diagnostic results and old
executables. No forced GC, package overwrite or original-PDF mutation.

Browser memory tracing and runtime detached-image mutation were rejected by
automatic approval review and are not part of this implementation. This change
uses ordinary source code, regression tests and the authorized owned desktop
workflow. One final scoped milestone review remains required after runtime gates.
