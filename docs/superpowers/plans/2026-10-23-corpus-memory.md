# Large-document export memory plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Investigate the measured scan export memory increase and remove redundant
temporary ownership if confirmed, retaining output fidelity and protection.

**Architecture:** Keep the existing in-memory composition and atomic publication
pipeline. Release a serialized source buffer once a changed parsed document owns
all necessary data; release that parsed document before subsequent protection.
Do not remove limits, verification, security checks or preservation passes.

**Tech stack:** Rust, PDFium/lopdf, existing release corpus runner and Windows
process-memory counters. This plan is also the scoped design, under the user's
authorization to choose routine implementation details without approval gates.

## Evidence and constraints

The current six-run release benchmark passes but peak sampled private memory for
the 96 MiB scan is about 653 MiB versus the historical 519 MiB. Current peak working
set is about 555 MiB versus 529 MiB. Those measures are distinct. No leak is assumed.
Three fresh native processes per fixture; same machine, Rust and power plan. The
portable 0.21 UI separately passes tabs, scrolling, six copy cases and retention.

The old source reader uses the same bounded vector read as current code; changing
its allocation policy would not establish the cause of this regression. Test-only
phase snapshots first locate the transient peak. Product code remains unchanged
until that evidence supports the proposed lifetime change.

## Tasks

- [x] Run the existing independent corpus validation, three native measurements
  per large fixture, and portable desktop copy/cache/scroll/retention workflows.
- [x] Use `src-tauri/tests/corpus.rs` phase snapshots to locate the scan peak.
  Retain existing aggregate sampling and correctness assertions. The snapshots
  occur after each timed phase, keeping their overhead outside latency samples.
- [x] If serialization is the measured peak, record a failing environment-specific
  memory acceptance check: the same scan workload must stay below 600 MiB sampled
  private memory on this machine, leaving room above the historical baseline.
  This is a diagnostic target, not a universal app limit or ordinary unit-test gate.
- [x] Make only the supported ownership change in `WorkerRuntime::compose_pdf`.
  The unchanged-output branch must return the original bytes. The changed-output
  branch can discard that buffer before serializing. No parsed borrowed references
  may remain when disposing of a buffer; Rust ownership and existing loaders matter.
- [x] Repeat three fresh-process release scan runs and the dense corpus, compare
  both private memory and working set. Run native export/form/comment/bookmark/
  protection and output-limit checks, then the applicable broader native suite.
- [x] Request one scoped review; reproduce and address material findings.
- [x] Record exact source/test/binary hashes and the measurements; preserve the
  earlier packages and report any remaining performance/low-memory limitations.

## Review focus

Unchanged composition; changed annotations/forms/bookmarks; protected export;
bounded-writer failure/partial output; peak attribution and workload comparability.
No new package or Adobe/Foxit/8 GB hardware claim follows from this investigation.
