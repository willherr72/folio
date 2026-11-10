# Reuse immutable page text inspection

The resource audit observed71top-level and48nested-form listings across23 Word
Preview/Cancel cycles. PageView removes ExistingTextLayer during paragraph editing;
the viewport itself also remounts. Returning to an unchanged page repeats native
inspection and queues work behind serialized rendering. Improve repeated editing
responsiveness while keeping all native edit/preview validation unchanged.

Approaches considered: retaining a hidden layer cannot cover viewport remounts;
engine-wide caching changes native lifetime and security scope unnecessarily;
a bounded frontend inspection cache fits the existing source-text/raster lifetime.
Coordinator chooses the frontend cache under the standing autonomous authorization.

Read merged top-level/form runs by exact adapter identity, source ID, page index,
and the selected reader functions/capability route. Share pending requests and
completed successful inspections. Preserve legacy/native selection and partial
failure behavior; never silently fall back after a native refusal. Do not retain
rejections or reason-bearing results. Retain each successful provider independently
when its sibling reports a partial failure; retry only the unsuccessful provider.
Return defensive copies of
run records, bounds and occurrence paths; retain an independent copy of native
results so consumers cannot mutate later selections.

Bound retained entries to32, including pending entries, and estimated retained
payload to2MiB (512bytes/run plus UTF-16 string bytes and8bytes/path entry).
This is an accounting budget, not an RSS claim. Oversized results remain usable by
the current caller but are not retained. Evict least-recently-used entries; late
completion after eviction/clear must never reinstall them. No unbounded error,
source, or adapter registry; use weak adapter identity keys.

Integrate the reader only into ExistingTextLayer. Clear it through clearRenderCache
when workspace sources are released and before/after native closeDocument; demo
close also clears source entries. Current crop filtering, focus disabling, source
change guards and full native expected-text/output proofs stay in place. No cache
of mutation authorization, paragraph previews, or filled values.

Prove real remount reuse; adapter/page/source/reader isolation; partial failures and
retry; caller/adapter mutation isolation; bounded eviction and oversized results;
late completion after clear; native close and workspace cleanup invalidation.
Run affected frontend tests, full frontend suite and production build, then build
a fresh local candidate and repeat native-IPC UI acceptance with exact helpers.
Require reduced unchanged-source inspection counts and unchanged Cancel/Apply/
Undo/Redo/save/reopen behavior. Latencies are measured, not promised. One scoped
final reviewer and one correction pass. Preserve current packages and all originals.
