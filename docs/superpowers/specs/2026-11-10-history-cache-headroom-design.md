# Full-history memory headroom

The extracted037 candidate passed105edits/100Undo/100Redo and independent300-page glyph/RGB audits, but its actual after105tree sample was1,088,143,360bytes, above the unchanged1GiB target. Native462,999,552bytes remained below512MiB. The earlier cache GUI sample had only13.61MiB tree headroom. Preserve the failed037 package/evidence, keep036launcher, and investigate a real reduction rather than accepting this failed sample or rerunning for a lucky pass. Standing user autonomy supplies authority for inline work.

## Evidence and choice

Role breakdown and complete current source83db425988b6a68ac083bdaa166bdad00a8111658e0fe377222fffcbab1c2043 are preserved in artifacts/history-cache-headroom-20261110/baseline.json and baseline-source/. The extra whole-tree bytes span native, renderer, GPU and browser processes; this does not prove a specific browser leak. Browser GC/compositor variation makes the former narrow margin insufficient.

SourceStore currently retains up to128MiB decoded hot source bytes in addition to64MiB encoded resident backing, encrypted anonymous spill and caller leases. Its source is byte-identical to the sealed sustained workload whose300-page after100sample held134,057,285raw cached bytes and63,043,737encoded resident bytes. Those logical counters are historical evidence about the same storage policy, not a fresh GUI ownership measurement.

Test reducing the decoded hot cache to64MiB while preserving encoded64MiB, source/backing identities, exact reconstruction, encrypted spill, error retry semantics and all100Undo/Redo states. This is a hypothesis for useful headroom, not a proven fix. Reducing encoded storage instead would add spill I/O; changing browser/GPU behavior needs separate allocation diagnosis. The selected change uses the existing tested cold-reload mechanism; additional cold history work is a real tradeoff to measure.

## Required proof

Before implementation, a real SourceStore regression must fail under the current default: retain enough independent multi-MiB sources to exceed64MiB, trim with the normal callback, then require raw cache<=64MiB while every handle and exact byte sequence survives cold lease/reload. Preserve external lease and spill/security/corruption tests. Tests must measure behavior, not read a constant.

Only default hot-cache policy and meaningful tests may change initially. No history truncation, source loss, forced browser GC, hidden style/viewport changes, or weakened512MiB native/1GiB after105tree targets. Product changes invalidate037's metadata-only native reuse: fresh appropriate native correctness and normal optimized GUI verification are required. Preserve current package resources for a fair candidate (the earlier GUI-only candidate had fewer resources); use complete037 resources with the new actual executable identity. Do not overwrite Folio-v0.37 or previous packages. An eventual new combined package is a separate versioned integration.

Run the same actual300-page105edit/100Undo/100Redo schedule, source/golden glyph/box/RGB72/144 audit,8corruption controls and exact owned cleanup. Require original memory targets and document headroom. Compare edit/history timings without a general causal or cross-host claim; investigate substantive regressions rather than trading responsiveness away silently. Selected small/quiz/font/shared-form and recovery/spill/error cases remain required. One fresh final scoped review after actual verification; inline fixes, explicit rulings/limits, historical seal. Do not claim broad leak freedom, peaks, printing, low-memory hardware or Adobe/Foxit parity.
