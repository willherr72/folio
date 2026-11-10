# Faster page admission after complete-source text editing

Continue Folio toward everyday commercial-editor usefulness by removing measured
redundant page-content loading after an existing-text edit. Standing user autonomy
opts out of routine questions. Keep the qualified 0.39 package and launcher intact;
this is an implementation milestone before any new combined portable.

## Evidence and choice

Current source d327b9bf85fda671fd3a1d856ecac9752b62754ac660780b9ce74cbdd82f42b9
has fresh desktop boundary measurements and an optimized native diagnostic probe
b977d781a73b907df1682703c5c2ae09c5a9140c159184ff18dfbcd9fb6a27c5.
All604 diagnostic saved pages match raw glyph/geometry and whole RGB72/144dpi;
15 trace-validator controls passed. Repeated observed300-page replacement takes
about1074ms, with385ms publication graph,237ms page admission and108ms independent
annotation retention. The two-page replacement is about75ms. Timings are selected
single-host wall observations, not a universal or CPU comparison.

The desktop control/observed cohorts differ materially (2472ms/1550ms), so their
difference is not accepted as observer overhead or a speedup. All three diagnostic
sessions are complete and absent. An immediate post-shutdown cleanup failure is
preserved; the added bounded exit observation occurs only after measured actions.

Alternatives: extending the original pinned parse template through output reopening
was rejected: bookmarks/units use distinct validation policies, annotation retention
has independent coupled resource control, and page admission loads native contents.
Reusing old page/annotation metadata would add source-ownership hazards. Chosen:
use the same renderer's size-only API when a new, writer-owned certificate proves
the serialized publication has no annotation dictionaries. Its local binding and
PDFium implementation compute the same bounding-box/quarter-turn dimensions as
the existing full-page path, without ParseContent. Prove this with native tests.

## Architecture and constraints

Add an engine-private page_admission module. Its opaque Publication owns serialized
bytes and an optional Proof; neither is deserializable. The factory serializes the
actual final mutable lopdf document, then grants a proof only for32..100000pages,
at most8MiB encoded output, no encryption_state, and a complete direct-object walk
with no dictionary key Annots anywhere, including trailer, nested dictionaries and
stream dictionaries. Charge at most100000Object values and depth64; any uncertainty,
budget exhaustion, empty/null/malformed Annots key or unsupported size yields no
proof and preserves the existing admission path. References need not be followed:
all indirect objects and the trailer are scanned. Scan failure never rejects a PDF.

The proof binds exact serialized byte length, SHA256 and page count. The annotation
census and all metadata remain local to this publication. A matching proof allows
the renderer's pages().page_size(index) to produce empty-overlay PageInfo; invalid,
nonpositive/nonfinite or unavailable sizes fall back to the existing full-page
geometry/import path. Physical UserUnit normalization remains unchanged. No raster,
font, collision, graph, annotation-retention, source-policy or security check is
removed. Annotation-bearing, small, large-byte and uncertified documents retain
the existing path, with no new refusal class.

Generic page replacement still returns bytes. Only exact complete-source text
publication carries the opaque result into a new private engine opener. Preserve
all open_bytes/open_bytes_scoped callers and font-import semantics through a common
private implementation with optional proof. No frontend/IPC/CLI/MCP schema changes,
new dependency, renderer change, global cache, source/history limit or public flag.
Preserve decoded64MiB/encoded64MiB/backing/security policies and100Undo/Redo states.

## Verification and acceptance

Write meaningful failing tests against an uncertified/no-size-only stub first.
Cover eligibility thresholds, every annotation location, depth/value exhaustion,
exact byte/page binding and tampering; compare native size-only and full geometry
over crop/nonzero origin/quarter rotations/inheritance/UserUnit. Check full opener
PageInfo equality and fallback imports. Keep the original failing results.

Run existing native correctness gates with their actual optimized integration and
Folio-opt0 library profiles. Reuse frontend tests only with exact unchanged frontend,
test/config/dependency membership; compiled normal GUI checks are fresh. Measure
fresh before/after native control/observed workloads with balanced order, unchanged
probe/fixtures/renderer and independent saved-page checks. Require a material
large-document benefit (target at least10% repeated replacement median), no meaningful
small-document regression, and explain any inconclusive evidence rather than claiming
success. Never raise budgets or remove preservation checks to pass a speed gate.

Use a fresh normal profiling-disabled GUI with complete accepted resources for the
original quiz and generated300-page105edit/100Undo/100Redo/fidelity workloads, same
source masks/display/history and original after105native512MiB/tree1GiB targets.
Preserve first resource attempts; no lucky rerun or altered viewport. Check the three
handoff cases and exact owned process/profile cleanup. No forced GC or injected CSS.
One fresh final review after concrete verification, consequential fixes inline and
all declined/minor/ruling dispositions explicit. Seal immutable implementation
source/helper/binary/output evidence; no promotion of an instrumented binary or
automatic user-app launch. No commit, push, merge or public release. Keep broad
producer, printing, accessibility, signatures, low-memory and parity work active.
