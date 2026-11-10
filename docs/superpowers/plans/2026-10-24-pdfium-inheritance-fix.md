# PDFium form inheritance correction implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Render and safely edit valid inherited form text with a verified PDFium correction.

**Architecture:** Pinned source build with the distributor's existing packaging
patches plus a minimal state-transfer patch; isolated acceptance before integration.

**Tech Stack:** PDFium C++, GN/Ninja/Clang, Windows SDK, Rust, Python, Tauri/WebView2.

**Spec:** `docs/superpowers/specs/2026-10-24-pdfium-inheritance-fix-design.md`

## Global constraints

- Preserve sources, dirty worktree, all appearance guards and previous checkpoints.
- Pin source/distributor/depot_tools revisions exactly as specified.
- No V8/XFA, automatic restart, commit, external publication or parity claim.
- Do not replace the product DLL on compile or direct-probe success alone.

## Review focus

- Child local settings and return-to-caller state must remain independent.
- Text positions/matrices must not be inherited as new graphics-state fields.
- Embedded/CID fonts, transformed forms and unrelated renderer workflows.
- Fresh setup or packaging silently reverting to the previous DLL.
- Candidate results accidentally mixing different source/DLL fingerprints.

## Task 1: Pinned candidate

- [x] Verify/download signed SDK prerequisites and isolated pinned source/tooling.
- [x] Retain the observed normal-probe failure against the bundled DLL.
- [x] Create `patches/pdfium/form-text-state-inheritance.patch` for only the missing scalars.
- [x] Apply distributor patches and build a separate DLL with recorded arguments/dependencies.
- [x] Run direct probe normal mode and inspect the resulting geometry/pixels.

## Task 2: Native compatibility acceptance

- [x] Parameterize candidate test loading without replacing the production resource.
- [x] Turn affected inherited-state tests into real edit/export/reopen checks with authored references.
- [x] Cover local overrides and child-return state; retain malformed/clipped/collision refusals.
- [x] Run broad native tests and independent-reader output checks with explicit DLL identity.

## Task 3: Reproducible integration and Windows acceptance

- [x] Add the pinned source-build recipe/provenance and include dependency identity in fingerprints.
- [x] Integrate the verified candidate; prevent setup silently reverting to the old dependency.
- [x] Build Folio and run isolated Windows workflow acceptance from the spec.
- [x] Obtain one fresh scoped final review and complete one verified correction pass.
- [x] Record exact results/executable/remaining gaps; keep the overarching goal active.
