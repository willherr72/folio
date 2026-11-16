# Cancel obsolete page renders during navigation

The broad goal remains Adobe/Foxit usefulness and quality. This component fixes
unnecessary native page work during rapid navigation; it does not claim to solve
all late-session renderer memory growth. Existing0.44 remains selected.

Current usePageImage dispatches adapter.renderPage for every new cache entry
immediately. It retains up to60idle images/96MiB estimated decoded bytes, but
those bounds do not prevent already-issued obsolete native work. All consumers
can disappear before a render completes; old work still competes with visible
pages. PageRaster canvas integration is separate: expanded focused/cropped/
fractional-DPR evidence has large display differences, not merely1-level rounding.

Use the current native/SVG rendering and existing cache limits. Extract cache
ownership into src/editor/page-image-cache.ts, with a PageImageCache acquiring
leases by adapter identity, immutable source/page and rounded render width.
Only two adapter renders may be in progress. Queue referenced entries FIFO;
when a queued entry loses its last consumer, cancel it without calling the
adapter. Started work finishes safely; evicted/disposed URLs revoke exactly once.
Shared leases do not cancel another viewer. Errors free a slot and allow retry.
Clear-source requests keep mounted images valid, but cancelled queued entries
cannot be reused. Resolved idle images retain existing bounded LRU behavior.

Alternatives: lower bitmap-cache limits leave obsolete native work unbounded;
AbortSignal IPC would require backend cancellation throughout PDFium and encoding;
canvas substitution has unresolved display failures. The lease scheduler directly
addresses avoidable requests without altering PNGs, display geometry or exports.
Two is an implementation choice to keep visible work moving while limiting waste;
actual Windows navigation must establish the behavior, not a speed guarantee.

No normal-user-app auto-launch, commits, pushes, merge/public release, deletion of
managed worktrees/ledgers or replacement of044. Standing autonomy opts out of
routine design/approval questions. Keep failed evidence. One scoped final review
after actual acceptance, no per-task agents. Use current managed worktree inline.

Acceptance: observed failing concurrency/abandonment regression; shared ownership,
clear/retry/errors/adapter-isolation/LRU regressions; fresh fullfrontend and TS/Vite;
actual current044baseline and new normalGUI fast300page navigation with passiveIPC
observation, matching native rasters and source hashes, no synthetic delays/forcedGC;
actual text/paragraph edit/history/save smoke; scoped final review and honest limits.
Native source/shared inputs must stay identical; any reuse of2747native passes
retains original profiles and is explicit frontend-only qualification, never a new
full native execution. The GUI is a development component, not a new portable.
