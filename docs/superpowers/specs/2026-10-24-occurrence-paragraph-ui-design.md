# Complete-source desktop paragraph editing

Wire the accepted native paragraph APIs into both uniform and styled dialogs.
Keep the current drafting/formatting UI and preserve the entire source when a
selected paragraph belongs to a shared-field or duplicated page.

Coordinator choice under standing autonomous authorization: explicit preview values
carry DocumentInfo, native pageIndex, optional PageContentReplacement and idempotent
release. A shared transaction module chooses complete capability sets, maps read
methods, validates fresh results and builds preview ownership. Alternatives were an
adapter facade disguising a full source as a one-page result (hides mapping) or
duplicating validation in both dialogs (risks inconsistent ownership). Read-method
adaptation is local; preview results remain explicit.

Requirements:

- Prefer complete native paragraph capability sets. Keep legacy adapters usable;
  never retry isolated editing after a native occurrence rejection. Optional fit
  and suggestion methods cannot cross from new inspection into legacy isolation.
- Freeze the current document/page for the request. Use the existing occurrence
  request/mapping/protection validation before rendering and again at Apply.
- Reject borrowed IDs from any tab or undo/redo history before acquiring ownership.
  Fresh invalid candidates are closed; borrowed results are never closed. Release
  is idempotent and respects live workspace ownership.
- Render candidate.pageIndex with the existing overlays and source axes/crop UI.
  Do not assume page 0. Preserve visual crop/rotation/overlays and source mapping.
- Cancel, draft invalidation, stale/native/render failure and unmount release only
  owned candidates. Preserve draft after rejection. Hold substitute fonts until
  native requests settle. Detach before synchronous successful Apply/unmount.
- Apply remaps all current same-source pages with one undo step, preserves other
  sources and metadata, rejects stale document arrays and validates again globally.
- Native commands and adapter protocol agree for uniform/styled inspection,
  suggestion, fit and preview. Diagnostic layout remains native-only for now.

Verify helper/protocol, both dialog lifecycles and App remapping/undo. Run full
frontend/type/build checks, one scoped review, and actual Windows styled reflow on
a cropped/rotated duplicate with shared forms, Cancel/rejection/re-edit/Undo/Redo,
save/reopen/refill and recovery; test uniform fallback through component/native
coverage. Independently inspect saved outputs and exact recovery pixels. Retain
all input/evidence and explicitly distinguish tested behavior from remaining gaps.
