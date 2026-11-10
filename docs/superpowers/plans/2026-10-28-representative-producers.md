# Representative Producer Investigation Plan

> Use superpowers:executing-plans inline for the investigation. Product changes
> require a subsequent diagnosed implementation plan and one final scoped review.

**Goal:** choose the next compatibility improvement from actual realistic PDFs.
**Architecture:** immutable authored producer exports, accepted packaged CLI,
separate discovery/edit/fidelity reports. **Tech stack:** existing Python producer
libraries, owned Word COM instance, Folio CLI, independent MuPDF/pypdf readers.
**Spec:** docs/superpowers/specs/2026-10-28-representative-producers-design.md

## Constraints and review focus

Preserve all originals, packages and prior evidence. No product/native edits,
Cargo, commit, publishing or cleanup. No user app access. Verify producer markers,
page counts and linked targets before evaluating Folio. Distinguish unsupported
discovery, failed mutation, and failed independent fidelity. Do not substitute
modified inputs for the actual producer originals or declare a refusal correct
merely because the current implementation intentionally refuses it.

## Task 1: authored producer originals

Create artifacts/representative-producers-20261028/generate.py and inputs-1 with
six two-page PDFs: Word tagged/untagged, ReportLab standard/embedded, MuPDF
standard/embedded. Include body, styles, table, link, header/footer and callout.
- [x] Capture accepted package and current source bindings; generate originals.
- [x] Record producer versions, owned Word identity/shutdown, exact hashes, text,
  expected markers and source page images. Expected: six valid original PDFs,
  two pages each, all authored markers present, no user process modified.

## Task 2: native discovery and saved edits

Create probe.py and per-source reports using the accepted extracted CLI.
- [x] Inspect all pages and nested text with pagination; record actual supported
  and refused nonempty runs with exact reasons and source identity.
- [x] Attempt body, table and callout text/paragraph insertions on separate copies;
  save/reopen/re-edit successes. Preserve structured errors and failed bundles.
  Expected: complete attempt inventory, not an assumed all-pass result.

## Task 3: independent fidelity and next scope

Create audit.py and diagnosis.md under the investigation artifact directory.
- [x] Audit successful saved PDFs against originals and intended replacement;
  include untouched second page and link preservation, font checks and bounded
  surrounding-pixel comparison. Keep any oracle limitation explicit.
- [x] Reproduce the highest-impact discovered failure, identify its source path,
  and choose a product fix with concrete acceptance gates. Update the roadmap;
  do not claim a product improvement from this investigation alone.

Completed evidence: `artifacts/representative-producers-20261028/diagnosis.md`.
Six sources, 36 attempts, 21 two-save workflows, 42 audited outputs. Forty pass all
checks; two retain the known unused-font-resource loss. Next scope: conservative
stroked-rectangle occupied regions for callout editing. No product changes yet.
