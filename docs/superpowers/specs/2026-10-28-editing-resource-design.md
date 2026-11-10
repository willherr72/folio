# Sustained paragraph editing and resource acceptance

Everyday editing must stay responsive and release cancelled previews. Measure the
current verified debug candidate through actual Windows UI/native IPC on two
small PDFs: a tagged original Word document and the mixed highlighted fixture.
Use three warmup and twenty measured Preview/Cancel cycles in isolated processes.
No forced GC. Preserve exact text/raster/document dirty state after cancellation.

Observe native preview responses and close_document calls by immutable source ID,
not just counts. Confirm every cancelled candidate receives a successful native
close. Use a positive control proving the collector sees preview/render traffic.
Snapshot native and descendant private bytes, JS heap and DOM counts at startup,
after warmup, every five cycles, and after closing tabs plus natural ten-second
idle. Process identity must include binary path and exact UTC start ticks.

Initial investigation budgets for these small fixtures on this development host:
preview p95 <= 2 seconds after warmup; cancel p95 <= 1 second; all cancelled
candidate closes complete within 5 seconds. Native private memory <=512MiB and
tree private memory <=1GiB, with final-minus-warm native growth <=64MiB and tree
growth <=128MiB. These are coordinator-selected thresholds, not Adobe/Foxit parity
or minimum-hardware certification. A failure requires diagnosis, not retroactive
threshold relaxation. Record cold samples separately, all timings and memory
snapshots, machine configuration, binary/source/helper/input hashes and limits.

Also exercise a Word body paragraph at a narrower area that demonstrably adds a
line while retaining source baseline, neighbors, font and tagged-document graph.
Verify too-small area refusal retains the draft, then corrected Preview/Apply,
Undo/Redo, saved reopen and independent output. Do not substitute a dimension-only
change that cannot affect layout for this requirement.

Approaches: unit-only lifecycle checks cannot measure the packaged process;
forced-GC memory snapshots hide ordinary retention; full-suite repetition does
not establish latency. Choose actual UI + passive CDP/native request observation
and independent saved-output checks. Reuse the verified candidate while source
is unchanged. Rebuild only after a reproduced product defect requires a fix.
No source-PDF edits, existing package overwrite, user-app restart or commits.
