# Form Automation Implementation Plan

> Use superpowers:executing-plans; inline implementation with one fresh final review.

**Goal:** inspect and fill existing forms through CLI/MCP while preserving native
answer/appearance semantics and immutable preview/apply/save ownership.
**Architecture:** source-bound field IDs and strict automation DTOs call native
inspection/fill; shared preview manifests add backward-compatible v3 form metadata.
**Tech Stack:** Rust/Serde, existing PDFium/lopdf, official local MCP SDK acceptance.
**Spec:** docs/superpowers/specs/2026-10-27-form-automation-design.md

## Global constraints
No commits, cleanup, release or original overwrite. One Cargo, locked/offline,
shared target, jobs1. Keep native8MiB inspection,512MiB source/bundle/session limits.
Form request256KiB; canonical text32768UTF8bytes; choices2000. Inspection record and
inner result512KiB, max25records; full response2MiB. v1/v2 manifest64KiB; v3 512KiB.

## Review focus
Fill-only permissions versus content editing; test protected reader fill/save.
Shared widgets/radio side effects; test saved group identity and neighboring fields.
Escaped/native-size data; test exact transport cap and raw/normalized input limits.
Manifest version confusion; reject v3 metadata in old versions and null/unknowns.
Candidate ownership on refusal/cancellation; verify no retained bundle and EOF cleanup.

## Task1: CLI field inspection and previews
- [x] Add tests/form_automation.rs actual missing-command RED, using existing native
form fixtures; inspect/fill/save/reinspect, stale IDs, kinds, refusal, permission.
- [x] Add automation/form_automation.rs: strict FieldValue, FormEdit, FormMetadata;
inspect(engine,info,page,offset), preview(engine,info,edit,parent,password), identity.
- [x] Extend automation Request and CLI parsing; inspect response envelope cap.
- [x] Extend shared PreviewDescription/Manifest with optional form metadata and
version-specific budgets; preserve absent-field v1/v2 shape. Publish complete source.
- [x] Run form_automation and prior paragraph/text targets; correct failures.

## Task2: MCP integration
- [x] Add actual subprocess missing-tool RED for inspect/fill/apply/reinspect/save.
- [x] Add protocol Call/schema/parser, worker private request path, result cap and
session preview-capacity routing; PreviewStore adopts v3 with explicit request limit.
- [x] Run MCP tests including lifecycle, invalid values, metadata and version tests.
Update docs/automation.md with supported values, limits and related-widget behavior.

## Task3: final review and native/package verification
- [x] Freeze scoped baseline/snapshot/hash delta for one independent final review.
- [x] Run native library + CLI/MCP + form/paragraph/text automation/inspection gates.
Reproduce Important review findings before fixing; retain failures and dispositions.
- [x] Build matching portable CLI; official SDK legacy/modern consecutive forms
workflow and independent saved answers, appearances, graph, input/cleanup checks.
- [x] Record checkpoint, review response, quality loop, next step; keep goal active.

MCP input frame ceiling512KiB including newline replaces64KiB, to carry an escaped
256KiB form request plus protocol metadata. The bounded32-frame queue remains.
