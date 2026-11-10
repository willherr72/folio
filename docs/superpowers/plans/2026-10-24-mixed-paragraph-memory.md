# Faithful mixed paragraph memory

Continue inline under the authorized full project goal. Milestone 5 of the mixed
paragraph plan; UI integration follows, not a substitute for this work.

Use a separate versioned FolioStyledParagraphs page key; leave uniform v1 intact.
Hints store a bounded draft, style/resource samples, area/alignment/spacing and
actual show bindings. Text in hints is never authority: independently replay from
verified current resource fonts, compare every encoded operator, effective state,
fragment/style and actual glyph origin/ink. Ignore malformed, stale, ambiguous or
overlapping hints. No stored glyph coordinates authorize reproduction.

Preserve newline-only styles using private font probes made from actual page
resources. Each sample is bounded and independently decoded/validated by the
existing font/resource admission path. Probes never enter the published PDF.
Resolved source styles measure through these resource probes, so restored palette
entries need not have a visible source object. Empty or newline-only styles retain
their size/color/font and therefore their blank-line layout and future typing.

Bound each draft to 4096 UTF-8 bytes, palette to 64, rows to 256 and output fragments
to 4096; metadata to 4 MiB, 64 records and 4096 total fragment bindings. Imported
ordinary selections remain limited to 64 pieces; larger selections require a
fully verified complete saved paragraph. Page snapshot's 5000-object and existing
content/work limits remain. Benchmark a large fragmented paragraph.

Restore full logical runs, palette, area, alignment and spacing. Reordered logical
or style metadata must reproduce the actual saved operators and geometry before
use. Publication retains independent text/glyph/raster/comment/permission checks,
writes bounded hints, then reopens again to prove writing hints did not change
appearance. Repeated no-op edits must render identically and stay re-editable.

1. [x] Add failing roundtrip tests for mixed style/hard-soft breaks/spacing,
   newline-only styles, whitespace-only fragments and large fragmented output.
2. [x] Extract reusable source-resource metrics/fragment verification and implement
   bounded read/write/replay. Keep source hints disposable, collision/state guards.
3. [x] Integrate inspection/publication, selection fields, expanded verified-only
   fragment policy and exact no-op behavior.
4. [x] Test forged/stale/overlapping hints, resource changes, Tc/Tw tampering,
   registry lifetimes and repeated save/reopen. Run focused regressions, independent
   reader/raster checks and scoped review. Record verified scope and remaining gaps.

Verified native checkpoint: 203 selected tests passed, three ignored; 32 independent
first-publication cases, 128 independent saved exports and four maximum-fragment
exports. Four review rounds; no unresolved demonstrated P0–P3 findings. Resource
name tampering is covered; direct program/Widths/ToUnicode mutation remains a
coverage gap. Capacity retry and ownership fallback combinations are listed in
`artifacts/mixed-paragraph-memory-review/acceptance-notes.md`.

Manifest: `artifacts/mixed-paragraph-memory-20261024/verification.json`.
The 4096-fragment debug no-op cycle remains about 31 seconds; this is functional
acceptance, not interactive-latency acceptance. No new GUI executable. Next plan:
`2026-10-24-styled-paragraph-editor.md`. The overall project goal remains active.
