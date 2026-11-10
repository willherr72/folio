# Mixed Word session acceptance plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final scoped review.

**Goal:** Verify real Word mixed paragraphs survive independent tab editing and
recovery after owned process termination without their input paths.
**Architecture:** Existing desktop controls, passive request observations, isolated
native launch/dialog helpers, same recovery directory across two owned app instances.
**Tech Stack:** Packaged 0.26, Playwright/CDP, Python MuPDF/pypdf/fontTools, PowerShell.
**Spec:** `docs/superpowers/specs/2026-10-30-mixed-word-session-design.md`

## Global constraints

- Preserve all prior evidence, original Word exports, user processes and dirty work.
- Only owned app PID/executable/start identity; no product hooks or injected IPC.
- No build or product edit until demonstrated failure. One native workload at a time.
- Existing isolated worktree; no commits, cleanup or release. User autonomy opts out
  of approval questions. Retain ledger/artifacts instead of skill cleanup boilerplate.

## Review focus

- Distinct highlighted text and source ownership must not cross tabs (task 1).
- A checkpoint must contain the latest edits/rotations, not merely two tabs (task 1).
- Restoring must use durable source payloads with original paths absent (task 2).
- Saving one restored tab must not mark its sibling saved (task 2).
- Equality comparisons must bind each output to its own original and reference (task 3).

## Task 1: Combined editing and durable checkpoint

Create `artifacts/mixed-word-session-20261030/session.mjs`, `run.ps1`,
`start-session.ps1`, `recovery_payload.py`. Consume mixed-word-layout originals and
package binding. Produce edit workflow, two reference PDFs and decoded checkpoint.

- [x] Bind inputs/package and capture executable helper bytes.
- [x] Edit A and B differently with native keyboard input; verify requests.
- [x] Switch with keyboard, verify exact per-tab Undo/Redo and sibling isolation.
- [x] Save references, rotate A90/B180 and wait for both latest dirty checkpoints.
- [x] Stop owned app and preserve/move only test copies after exact path/hash checks.

These checks passed in `edit-1` on the unchanged 0.26 package. The corrected
candidate also passes the fresh `edit-5`/`restore-5` pair. `restore-1` remains failed.

## Task 2: Same-data restore and saved outputs

Consumes task 1 checkpoint and reference output record. Produces restore workflow
and two restored saved PDFs using a fresh owned process and the original data path.

- [x] Launch same package with original data directory and fresh WebView profile.
- [x] Restore using the dialog; verify active tab, both independent views and dirty flags.
- [x] Save A then B, proving saving A leaves B dirty; reopen and check unchanged preview.
- [x] Stop owned apps; retain originals and all failure evidence.

## Task 3: Independent recovery audit and review

Create `audit.py`, `checkpoint.json`, and continuation ledger. Consume both workflow
records, references, restored PDFs, and persisted recovery envelopes.

- [x] Compare complete output text/styles/geometry/pixels/font bytes/graphs per tab,
  allowing only the requested first-page rotation; validate original source hashes.
- [x] Verify durable payload checksums and all owner bindings; negative comparisons
  must reject swapped tabs, wrong rotation and outside pixel changes.
- [x] One frozen scoped final review, local corrections if necessary, and update
  quality roadmap. Keep overall goal and original memory failure open.

## Evidence-driven amendment: rotated reading order

edit-1 passes both independent histories and latest durable checkpoint. restore-1
restores both dirty views and saves independently, then fails reopening recovered-A:
the native reading layer reorders words after intrinsic rotation is serialized.
MuPDF/pypdf prove source-order text, raw glyph records and unrotated raster remain
exactly equal to the reference PDFs. This is a reading-path issue, not evidence of
lost document content or failed recovery.

Exact packaged PDFium probes identify seven text objects per page with no reader
characters and point-only bounds. `reader_order::simple_forward_text` rejects the
first such object, bypassing source-axis reading. The new synthetic regression in
`src-tauri/tests/text_geometry.rs` must reproduce this at 90/180/270 degrees. An
explicit ignored test binds the actual local authored Word outputs.

- [x] Observe focused regression fail on the original source.
- [x] Allow only proved point-bounded, filled, non-clipping text objects with no
  reader characters to be neutral in direction admission. Keep all matrix/font,
  ambiguous visible singleton, script and layout guards.
- [x] Run focused text geometry/search and explicit Word cases; expand appropriate
  native checks after the correction. Build a separate candidate with source binding.
- [x] Resume the failed desktop saved-reopen step on the corrected candidate,
  then complete independent recovery audit and the single final milestone review.

The optimized 0.26 package remains unchanged and retains the reproduced limitation.

The single scoped review is complete (`review/review.md`). Its failed-run receipt
finding is corrected locally, with seven negative receipt cases and one positive
case passing. The independent pixel control now uses the actual outside-mask
predicate; execution against corrected desktop outputs now passes. No
second review dispatch is required. Full native session 69523 passed 1,756 tests
with 18 intentional skips across 129 targets. The source-bound candidate build,
`edit-5`/`restore-5`, and `audit-1` pass; `final-evidence.json` reconciles receipts,
PDFs, recovery payloads, source hashes, and owned-process closure. Earlier failed
runs retain the reading-order defect and corrected observer errors. Overall parity
and the original preview-memory failure remain open.
