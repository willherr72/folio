# Large-document edit attribution implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final fresh-context review of consequential claims.

**Goal:** Locate the dominant editing cost and retained-source behavior before making a product change.
**Architecture:** Reuse the accepted raw desktop controller and optimized candidate. Add scalar-only native-call observation and separate UI phase timings, with a paired control and an independently checked one-page derivative.
**Tech Stack:** Node, PowerShell, Python MuPDF; current Tauri/Rust application unchanged during diagnosis.
**Spec:** docs/superpowers/specs/2026-11-03-large-edit-attribution-design.md

## Constraints and review focus

Preserve original inputs, accepted evidence and packages. No commits or publication. No Network/Tracing/Emulation/forced GC. Process samples are discrete/non-atomic; 6 GiB is only an operational stop. Logs retain no response buffers or request objects, and stop at 4096 rows. Do not infer private engine allocations from IPC identities or add overlapping durations. Check observer errors/overflow, stale-page acceptance, and exact response/exception preservation.

## Work

- [x] Verify baseline fingerprints, bind the 300-page source and prove one-page derivative first-page equality.
- [x] Add `invoke-observer.mjs` with tests for original promise/receiver/argument preservation, rejection/synchronous throw, scalar-only logging and bounded overflow; run RED then GREEN. Runtime proved this platform's invoke is read-only; superseded by `fetch-observer.mjs`, whose four tests also passed RED then GREEN. It never reads/clones response bodies or secret headers and preserves the exact fetch promise.
- [x] Add `session.mjs` and `run.ps1`: seven alternating edits, separated phases, original and repeated raster/text identity, Undo/Redo, Save, close/idle, identity-bound cleanup and copied-helper pins.
- [x] Run the preserved successful `control-300`, `observed-300-2`, `observed-1-2`, and the additional `observed-2` mounted-page control sequentially on the exact candidate. Audit saved files independently, logs/identities and forbidden-command counts. Earlier failed attempts remain evidence, not passing runs.
- [x] Analyze call and UI-phase durations without double-counting, qualify source ownership and memory evidence, and trace the dominant boundary in source. Native discovery/replacement request cost is established; internal parsing/page loading/verification attribution is not. No product optimization is yet justified.
- [x] One final review of evidence and resulting change, fix consequential findings, record checkpoint and next action. Minor image-matrix omission corrected with RED/GREEN and complete post-correction audit of existing records. Broad quality goal remains active.

Next engineering step: measure native worker subphases (`single_font_text_context`, isolated replacement, complete-source graph publication, `open_bytes_scoped`, verification and annotation-source retention), plus source discovery stages, on the 300-page and two-page fixtures. Preserve loader/admission policies and faithful output checks; do not infer a particular parser or cache fix from request-level timing alone.
