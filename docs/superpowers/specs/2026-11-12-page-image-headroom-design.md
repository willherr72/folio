# Page-image cache headroom at normal high-DPI displays

The unselected047 package passed all functional stages and complete300page
105edit/100Undo/100Redo fidelity, but its after105 owned-process tree sample was
1,084,231,680bytes, above the unchanged1GiB target. Native286,908,416bytes passed
512MiB. A controlled046 run at the same1280x820/DPR1.5 measured1,057,042,432bytes;
the older selected046 record had instead used1028x749/DPR1. Native/JS/DOM and GPU
samples are not causal proof, and a passing repeat cannot erase the047 failure.

The page-image cache already estimates decoded RGBA bytes and retains up to96MiB,
with mounted leases protected above budget. Reduce that retained-image allowance
to32MiB to reserve headroom; retain the60-entry ceiling, two render slots, full
pixel width/resolution, ownership, source isolation, 100 history states and retry
behavior. This trades some idle revisits for less retained image memory; it does
not reduce image fidelity or evict images still borrowed by visible consumers.

Use observable RED-to-GREEN tests for decoded-byte pressure, recent reuse and
mounted/shared leases. Then build and test a new candidate under the same current
display/workload and original512MiB/1GiB targets. If it does not provide headroom,
continue diagnosis; do not claim the cache change fixes the measured resource
issue merely because the unit tests pass. Preserve failed047 and selected046.
No renderer/DPR emulation, forced GC, style injection, history truncation or target
relaxation. No native/dependency/API changes. A later portable must have its own
fresh extracted runtime, review and seal; existing047 results cannot stand in.

Standing autonomy chooses this bounded cache experiment rather than changing the
rendering backend or hiding the failure with a smaller display. Original OCR reopen
timeout remains unresolved. Keep the managed worktree, evidence and broader goal.

Follow-up evidence: direct debugger observations confirmed decoded-image retention
fell from82,581,920bytes to33,032,768bytes. The normal cache32 workload passed all
fidelity/history checks but sampled1,065,218,048bytes, leaving only8.13MiB below
1GiB. This does not establish reliable process-tree headroom.

A separate, explicitly injected diagnostic disabled only modal background blur.
The same binary/display and full105/100/100 actions sampled971,960,320bytes, with
GPU private memory321.04MiB versus384.21MiB in the normal run; page rasters, text
and history states matched. Native CPU/edit timings varied, so no latency claim
follows. Keep the dim rgba(42,37,32,.42) backdrop and remove only its blur(3px)
property in production. The page navigation styling, modal geometry, focus and
interaction stay intact. Scope now includes this exact src/styles.css change.
The injected diagnostic remains separate from acceptance: a fresh normal build,
full frontend and extracted-package checks without injection must prove the result.
