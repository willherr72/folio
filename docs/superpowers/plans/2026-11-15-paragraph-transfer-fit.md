# Paragraph transfer fitting implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans inline. One fresh final review, no implementer agents.

**Goal:** open a typed replacement in paragraph editing with a native-fitted draft area and one Undo operation.
**Architecture:** a small draft-fit helper calls the existing native fitter against an immutable prepared editor. App supplies source-page dimensions/crop; initial dialog feedback explains the result. Keep source ownership and font leases through preparation.
**Tech Stack:** existing React/TypeScript editor transactions and native fit adapter; Vitest, owned Windows WebView and MuPDF/pypdf acceptance.
**Spec:** docs/superpowers/specs/2026-11-15-paragraph-transfer-fit-design.md

## Global constraints
No Git mutations, new dependencies, version/package/launcher changes, private inputs or clipboard operations. Preserve old artifacts. Native/frontend/build/UI serial; one Cargo job and existing D: cache/TEMP. Frontend min/max two workers, default5000ms. All fitting is a draft proposal; Preview/Apply remain mandatory.

## Review focus
- A failed/missing fit must retain the exact transferred text/font/caret and manual recovery.
- Treat returned geometry as adapter-owned; copy/validate before adopting and respect visible crop.
- Preserve the source baseline and make transfer plus fitted geometry one Undo/Redo item.
- Late native responses must not enter a different tab/source session or release a font while in use.
- No unchanged draft, unsupported adapter or user-defined area should trigger extra publication or silent font/size changes.

### Task1: Draft fit and integration
- [x] Add focused failing tests for native proposal, baseline/selection/style and one Undo, no-op/error/malformed/crop/provider-mutation cases.
- [x] Observe RED, implement a focused helper and optional prepared fit feedback.
- [x] Connect App preparation geometry and dialog feedback; add ownership/font-lifetime integration tests.
- [x] Focused GREEN, full frontend and typecheck; qualify unchanged native inputs.

### Task2: Actual desktop acceptance and review
- [x] Build normal optimized diagnostic candidate from exact qualified source/resources.
- [x] Run actual transfer without manual Fit, Undo/Redo, refusal/manual recovery, save/reopen and independent saved-output checks.
- [x] One fresh final scoped review; one RED/GREEN correction pass if Important findings.
- [x] Record completion, statuses and explicit limits; broad goal stays active.
