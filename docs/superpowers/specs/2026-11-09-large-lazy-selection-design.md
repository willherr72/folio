# Large-document lazy-selection acceptance

Continue the standing Folio quality goal by checking the current optimized GUI on the existing generated 300-page dense-text fixture. This is validation of the qualified selection and instruction-row corrections, with no proposed product change. User standing autonomy applies; the approach is the implementer's call.

## Approach

Reuse the source-bound two-page native workflow, replacing only the fixture and accurately labeling the scope. Run 105 real existing-text changes, all 100 retained Undo states and all 100 Redo states. Check exact text/raster states, current accessible reading text, restored selection geometry/copy, Highlight activation and save/reopen. Measure discrete native/WebView private bytes, JS heap and mounted elements at the existing milestones. No injected CSS, forced GC, fetch/IPC patches, browser emulation or concurrent compiler/resource observer.

A second comparison run is unnecessary for establishing larger-document correctness; prior measurements retain their original source, binary, workload and display identity. Native-only acceptance would miss WebView costs and the user-visible history path. This run establishes an observation for the current candidate; it does not create a new before/after performance claim.

Bind the candidate to the instruction-strip checkpoint, the fixture to its existing SHA256, and the saved result to the previously independently audited large-document golden output through its preserved checkpoint/result hashes. Compare all 300 pages using an independent PDF reader: exact extracted glyph data and whole-page RGB at 72 and 144 dpi. Do not mask changed pixels. Preserve the original PDF and all earlier artifacts.

## Resource criteria and limits

Retain the existing operational stop at sampled 6 GiB whole-process-tree private memory. Report the existing quality targets separately: at edit 105, sampled native private bytes below 512 MiB and whole-tree private bytes below 1 GiB. A missed quality target remains open; it does not invalidate correct history or justify changing the target. These samples do not prove a peak bound, absence of leaks, low-memory hardware behavior, physical printing or screen-reader/native clipboard correctness.

## Acceptance

All history, semantic and saved-output assertions pass, owned process identities and private profiles are absent afterward, and helper/source/binary hashes are unchanged. Record any failed attempt and distinguish product failures from test-helper mistakes. Keep accepted 0.34 and its launch pointer unchanged. Qualify a combined development portable only after the next integration decision; no release or commercial-parity certification is implied.
