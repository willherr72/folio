# Atomic text-edit automation implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline under standing
> autonomous authorization. One final fresh feature review after all tasks; keep
> this managed worktree, original files, failed attempts and existing packages.

**Goal:** Publish one validated multi-edit candidate or no candidate, through
native, CLI and MCP automation; prepare for later document replacement planning.
**Architecture:** Descending original object paths, preflight/rebinding proof,
worker-owned staged sources, version-6 preview metadata and generic capabilities.
**Tech stack:** Rust/Tauri, serde, existing PDFium/lopdf, Python Windows QA.
**Spec:** docs/superpowers/specs/2026-11-14-atomic-text-batch-design.md

## Global constraints

1..64 edits,32 pages,1..13 path entries;16KiB expected/4096byte replacement;
512KiB aggregate strings/request/manifest;32MiB PNG and512MiB bundle/session.
No new deps/version/portable/launcher/Git/clipboard/private inputs/cleanup.
Serialize native/build/GUI, one Cargo job, D:/Folio-build-cache/t3-8962a327 and
corresponding-temp. Frontend two workers/default5000ms; retain profile meanings.

## Review focus

- A failed late step or abandoned reply must release every intermediate source
  and preserve the complete original and already-open documents.
- Original IDs/paths must not silently bind a different run after earlier edits,
  including shared/nested objects and resource/font changes.
- Every affected page image and metadata byte must participate in manifest,
  session budget and tamper checks without weakening legacy versions.
- Protected credentials, edit/copy permissions and exact no-op bytes must survive
  CLI/MCP Apply/chaining without credential or document-content error disclosure.
- Large/duplicate/overlapping selections and source/output aliases must refuse
  atomically before publishing a partial bundle or overwriting a user file.

### Task 1: native complete-source batch transaction

**Files:** new src-tauri/src/text_batch.rs; modify engine.rs,lib.rs,
occurrence_form.rs; tests in the new native module.
**Interfaces:** `TextBatchEdit {page_index,object_path,expected_text,replacement,
font_id}`; `PdfEngine::replace_text_batch(&str,&[TextBatchEdit]) ->
EngineResult<PageContentReplacement>`; worker request owns all stages.

- [x] Capture current Find component/source/native anchors and exact preimages.
- [x] Add native tests before behavior; minimal API skeleton may establish an
  executable RED. Observe multi-edit success and atomic cleanup assertions fail.
- [x] Implement budgets, original-target resolution, descending paths, staged
  rechecks, ownership and abandoned-reply cleanup using existing edit routes.
- [x] Run focused native cases: same-page reverse ordering, two pages, nested
  sharing, late collision/font failure, stale/duplicate/prefix/budget refusal,
  no-op, rotated/UserUnit dimensions and unchanged source bytes. Complete affected
  occurrence regressions. Expected: assertions pass and input guards stay exact.

### Task 2: bounded batch previews and automation transports

**Files:** new automation/text_batch_preview.rs; modify automation.rs,
automation/text_preview.rs,cli.rs,mcp/{protocol,worker,previews,session}.rs;
tests in those modules and native integration targets.
**Interfaces:** strict version1 batch request; version6 manifest batch metadata;
`preview-text-batch` and `folio_preview_text_batch`; generic Save/Apply/Discard.

- [x] RED strict schemas/budgets, all-original-ID resolution, batch images,
  legacy metadata refusal and complete tamper/aggregate-budget coverage.
- [x] Implement one captured original snapshot, font installation, native batch,
  exact no-op publication, per-page images and generic capability adoption.
- [x] RED/GREEN CLI parsing and MCP launch ownership/capacity/credential/chaining
  checks. Expected: one exact candidate or no bundle; every refusal preserves
  originals and existing destinations; legacy preview tests remain green.

### Task 3: exact-binary acceptance and one final review

**Files:** controllers, fixtures, input guards and evidence under
artifacts/atomic-text-batch-20261114; docs/automation.md and component ledger.

- [x] Run complete library correctness and affected ordinary optimized native
  integration, default/no-default builds and appropriate unchanged-frontend
  qualification. Build normal combined GUI/CLI with26exact renderer resources.
- [x] Run actual CLI/MCP batch preview/save/Apply/Discard on authored mixed-page,
  nested/shared, protected and no-op inputs. Audit saved PDFs independently and
  reject deliberate content/geometry/image/manifest corruptions. Stop owned apps.
- [x] Freeze bounded whole-feature delta/evidence for one fresh final reviewer;
  rule on declined judgments; one Important fix pass with RED/GREEN and relevant
  suite; no re-review. Record verified component completion and pending global
  Find/Replace planning without claiming the broad Folio goal is complete.
