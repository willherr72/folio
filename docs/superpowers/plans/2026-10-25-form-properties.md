# Existing Field Properties Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans inline. One final scoped review, no implementation delegation.

**Goal:** Edit existing field names, labels and required/read-only flags without losing linked identity or saved content.
**Architecture:** Bounded metadata edits to a complete immutable source; captured frontend source replacement in one history transaction.
**Tech Stack:** Rust/lopdf/PDFium, Tauri, React/TypeScript.
**Spec:** docs/superpowers/specs/2026-10-25-form-properties-design.md

## Global Constraints

Preserve source PDFs/worktree/binaries/failed evidence. No commits or publishing.
One Cargo/build at a time. Names1024 bytes, labels4096 bytes; reject controls/bidi and
periods in changed partial names. AcroForm4000 nodes/64 depth; action scan100000
objects/values,64 depth,16MiB byte budget. Native edit-content+annotate permission.
Preserve original field IDs and appearances. Shared logical fields update together.

## Review Focus

- Terminal owner versus widget Parent aliases; partial-name ancestor preservation.
- Field flags inherited from ancestors and unrelated bits/options/defaults retained.
- Shared indirect action target values must not globally rewrite unrelated content.
- Rename refusal for opaque scripts/actions without blocking unrelated property edits.
- Async source ownership and stale tab/adapter results never commit or close borrowed IDs.

## Task1 — native source-bound properties

Create src-tauri/src/form_properties.rs, form_properties_engine.rs and
src-tauri/tests/form_properties.rs; modify lib.rs,engine.rs,app.rs.
Types: FormProperties {expectedKey,name,fullName,label,readOnly,required,linkedWidgets,
renameReason}; FormPropertyChanges with four optional settings; SetFormPropertiesRequest
{pageIndex,annotationIndex,expectedKey,changes}. Public inspect_form_properties(source,page,index)
and set_form_properties(source,request)->DocumentInfo. Worker uses editing_document,
immutable open_bytes and original policy/bookmark/path/annotation ownership.

- [x] Archive exact scoped baseline. Public RED: existing shared field becomes readonly,
  required and relabeled while retaining both widgets and their AP bytes.
- [x] Identify logical terminal ownership via bounded tree; inspect effective metadata.
- [x] Implement changed-only mutation, collision/stale validation and bounded standard
  action target rewriting. Preserve all other object values; expose renameReason.
- [x] Add/run native nested/merged/shared/type/Unicode/action/protection/budget matrix.

## Task2 — user-visible properties and source history

Create src/editor/form-properties.ts, src/components/FormPropertiesDialog.tsx and
tests/form-properties.test.tsx; modify adapter.ts,FormFillEditor.tsx,App.tsx plus
focused App integration coverage. Adapter methods inspectFormProperties/setFormProperties.
Dialog owns draft only; captured App operation owns native result/cleanup. Apply uses
commitFormReplacement, all-tabs borrowed-source checks and producing-adapter cleanup.

- [x] RED changed-only draft/Reset/readonly/rename-reason and source-owner tests.
- [x] Add optional field-properties entry in both form entry views, dirty draft gate,
  permission gating, loading/busy/errors and shared-scope explanation.
- [x] One-step source replacement updates current form session and Undo/Redo.
- [x] Run focused then full frontend and build checks.

## Task3 — final review and runnable checkpoint

- [x] Freeze snapshot/hashes and invoke one final scoped reviewer; parent reproduces
  and fixes consequential findings, no rereview.
- [x] Appropriate native/full frontend gates and fingerprinted desktop build.
- [x] Windows normal/narrow property edits, linked/read-only behavior, Undo/Redo,
  save/reopen/later filling; independent saved graph/field/pixel verification.
- [x] Stop owned app, bind evidence, update README and quality loop; leave goal active.
