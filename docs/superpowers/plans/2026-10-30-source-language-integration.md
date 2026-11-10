# Source language integration implementation plan

> Use superpowers:executing-plans under standing autonomous authority. Preserve old evidence, source PDFs and unrelated work; no commits or releases.

**Goal:** Preserve source language through whole-source Save, including unmarked page text, ordinary reusable Forms, native annotation text and leading Info.

**Spec:** `docs/superpowers/specs/2026-10-30-source-language-integration-design.md` and its builder-check rulings. Parent structured-source assembly remains active.

**Architecture:** Native graph changes only. Retain the leading catalog default, frame differing-default page streams, assign compatible previously unstructured annotations whole-object language owners, then reconcile all roots with the existing structure merger. Bounded typed admission must reject unresolved residual strings and clipping-text ownership. No inferred language, operator re-encoding, new Form wrapper or appearance flattening.

## Global constraints and review focus

- All graph IDs canonical, source sharing retained, failure atomic before reusable state/output publication.
- Depth 64; at most 100,000 visited objects/annotation occurrences, 1,000,000 traversal steps, and 16 MiB staged annotation metadata. Parser/header/retained graph budgets remain cumulative.
- Existing structure owns existing tagged annotations. A new whole owner must not absorb already structured appearance content. Widgets require their own adapter.
- New standard annotation roles live in a separate temporary root so original RoleMap remapping cannot reinterpret them during reconciliation.
- Source-wide string admission distinguishes exact supported text/identifier fields; unknown strings are not assumed binary. Form clipping text remains refused until its language ownership is established.
- This is not full tagging/PDF-UA/printing or commercial parity certification. The exact authored combined workflow remains an acceptance requirement, not a substitute fixture.

## Task 1: Annotation language owners

Files: new `composition_annotation_language.rs` and tests, private registration.

Interface: `prepare(document: &mut Document, pages: &[ObjectId], original: &StructureInfo, language: &[u8], pdf2: bool, check) -> EngineResult<Prepared>`. `Prepared` returns an optional temporary `StructureInfo` for new owners and the canonical set of annotation IDs with established language ownership. Caller supplies a private composition graph and merges original/new roots before publication.

- [x] Write and run failing tests for unstructured text/link annotation owners, raw strings/AP identity, shared page occurrences and aliases, existing owners, explicit PDF2 Lang, Widget/structured-AP refusal, role remapping, budgets and cancellation.
- [x] Prepare all additions without mutating the document. Stage fresh root/ParentTree/elements and owner keys within the metadata budget. After the final check, commit without fallible traversal; no mutation on refusal/cancellation.
- [x] Verify each generated graph through existing merger and ownership inspection, and run focused composition tests.

## Task 2: Content and residual-string admission

Files: new `composition_language.rs` with bounded source traversal, plus tests.

- [x] Add source fixtures for default/nested marked/structured language, inherited Form rendering modes and reuse, leading Info, unknown/null defaults and residual string fields.
- [x] Read complete page Contents with aggregate limits and preserve original references/encoded streams. Inspect invoked Forms recursively for unresolved clipping text and malformed context; do not overwrite caller overrides.
- [x] Add explicit typed rules for retained strings. Refuse unresolved string fields when the source default changes. Apply the verified page envelope only after admission.
- [x] Carry both the native cancellation predicate and ordinary checkpoint function through graph/catalog assembly. Preserve old call sites with a default wrapper only where no native monitor exists.

## Task 3: Save integration and acceptance

Files: `composition_catalog.rs`, graph/worker integration, existing/new native tests and independent audit helpers.

- [x] First reproduce current unmarked text/leading Info language changes; retain failures.
- [x] Use leading source Lang, source-owned original roots and Task 1 synthetic roots in source order. Validate final page/structure ownership and retained graph budgets.
- [x] Run scoped native gates, independent effective-language and exact raster/object audits, fresh debug build and the authored combined desktop save/reopen/re-edit workflow.
- [x] Request one final implementation review; correct demonstrated defects with RED/GREEN evidence. Persist final hashes, capabilities and remaining limits without closing the broad goal.


Completion evidence: artifacts/source-language-integration-20261030/checkpoint-evidence.json.
The one final review finding was reproduced and fixed within the review correction
pass. Corrected scoped native/build/desktop and independent semantic/raster gates
passed. No release or broad-goal completion is asserted.
