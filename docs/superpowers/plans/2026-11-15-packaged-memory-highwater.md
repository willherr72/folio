# Packaged memory high-water implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. One final fresh scoped reviewer; no implementer agents or repeated reviews.

**Goal:** Measure sustained packaged editing with kernel-accounted lifetime process-tree commit and independently verified saved output.

**Architecture:** An owned suspended-start Windows job wraps only the unchanged055GUI and its children. Reuse the exact public300-page history schedule and auditor; keep observer/driver processes outside the job. Never substitute polling for a missing high-water counter.

**Tech Stack:** Python ctypes/Win32 job APIs, PowerShell, owned Windows WebView CDP, existing MuPDF/pypdf public-fixture auditors.

**Spec:** docs/superpowers/specs/2026-11-15-packaged-memory-highwater-design.md

## Global constraints

Preserve existing packages, selected050launcher, dirty tree, public/private originals and failed evidence. No Git/deletion/publication/new private or system clipboard operations. Unchanged packaged055 source23b7d28e; new evidence root artifacts/packaged-memory-highwater-20261115. Serial observation, no concurrent native/frontend/build/app work. Product-only job,8GiB operational cap,1GiB lifetime tree quality target, original native512MiB/tree1GiB after105sample targets separate. No forced GC or weakened fixture/history/oracle.

## Review Focus

- Process startup and descendants: assignment before resume, no child escape, nested WebView membership; no unrelated process included or terminated.
- Short-lived spikes: actual allocate/free/exit control retains nonzero job peak between samples; no polling or API-error-to-zero acceptance.
- ABI/accounting meanings: exact64-bit structure sizes, API result checks, commit versus resident/sample distinctions and explicit8GiB cap.
- Workload/fidelity drift:105/100/100 on the same300-page authored fixture with unchanged independent pixel/glyph references, packaged renderer and no forced GC.
- Scope overclaims: finite measured lifetime job commitment does not prove leaks, physical8GB, all producers, physical input, printing or parity; failed quality gates stay open.

### Task1: owned kernel-accounting observer and real controls

**Files:** Create artifacts/packaged-memory-highwater-20261115/{win_job.py,test_win_job.py,allocation_control.py,observer.py,controls/}. No production code.

**Interfaces:** JobProcess launches owned binary suspended with explicit environment/cwd, assigns before resume, exposes pid/creation/exit, snapshot() kernel peak and active PID/accounting, close() scoped cleanup. observer.py writes compatible process.json plus job samples/final receipt for Task2.

- [x] Write failing actual Win32 tests for startup ownership, a child96MiB spike freed/exited before the final query, excluded unowned sentinel and cleanup. Add record mutation checks for absent/zero/invalid counters and ownership.
- [x] Observe RED, implement minimum typed ctypes definitions/API checks and owned-only cleanup, then GREEN. Retain exact structure sizes, caps, kernel counters, child accounting and observer hashes. Fail closed on API/assignment errors.

### Task2: unchanged packaged long history under the observer

**Files:** Create package identity, helper provenance, adapted startup/controller and fresh history/audit folders under this component.

**Interfaces:** Consume Task1 validated JobProcess/observer and055sealed identity. Produce actual105/100/100 runtime, observer lifetime high-water receipt, original discrete samples, all300independent saved-page references and terminal cleanup.

- [x] Verify055source/checkpoint/package and current process absence, copy only required pinned053helpers and public fixture authority. Bind every transitive auditor and changed startup byte.
- [x] Run actual extracted055history with job assignment before startup; preserve original action/oracle schedule. Wait on its authoritative live handle; never restart on observation timeout.
- [x] Independently audit300saved pages and existing eight corruption controls; reconcile observed WebView/native PIDs with job membership, loaded packaged renderer, final counters and cleanup.
- [x] Classify target results honestly. If lifetime tree>=1GiB or original sample limits fail, preserve failed quality evidence and diagnose; do not claim acceptance or lower the workload. No repeat tests/builds unless justified by a correction.

### Task3: review and durable result

**Files:** Scoped review, dispositions, completion/diagnosis and finite pins; update live resource/quality status only after terminal evidence.

**Interfaces:** Consume validated observer controls and actual current package runtime/audits. Produce reviewed result with exact measurement meanings and remaining/failing gates, broad goal active.

- [x] Request one fresh most-capable review with frozen inputs and the Review Focus verbatim. Handle Important/Critical findings in one RED→GREEN pass; no re-review. Record all rulings/minors/declines.
- [x] Reverify exact package/source/helper/input/output/observer pins, cleanup and final result; preserve all attempts. Record measured quality failure as open if present. Keep broad goal active.
