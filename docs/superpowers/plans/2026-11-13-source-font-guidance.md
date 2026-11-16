# Source font guidance implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Existing managed worktree; user opted out of routine questions. Preserve artifacts and dirty work; no commits/cleanup/new checkout. One final fresh-context review after implementation and runtime verification.

**Goal:** Make explicit replacement-font selection easier when an embedded PDF subset lacks newly entered characters.
**Architecture:** Optional source label plus stable exact normalized name ranking in the existing lazy FontPicker; existing font acquisition/coverage/ownership/native publication stay authoritative.
**Tech Stack:** React/TypeScript, Vitest, existing Tauri/Windows WebView2 QA.
**Spec:** docs/superpowers/specs/2026-11-13-source-font-guidance-design.md

## Global Constraints

Preserve baseline keyboard component fingerprint5dd59ffe51c42efde15c2e729f2414b6ccbc06037cef8bf96b29b9a72c72b9f1, selected050 and reviewed051. Frontend two workers/default5000ms; serialize fullfrontend/build/owneddesktop. Cargo onejob, D: cache/temp. Private quiz stays ignored; no clipboard access/mutation. Native equality is reuse only; no fresh-native/all-project/parity claim. No new package selection.

## Review Focus

- Different weight/style and partial family names must not become exact matches.
- Malformed subset labels, empty names, Unicode names and duplicate catalog names must stay deterministic.
- Filtering, disabled entries and roving keyboard focus must agree with visible order.
- Opening/canceling must load no new font automatically or alter the replacement/draft.
- Mixed styles must not imply a single source face; explicit selected substitute must retain source context.

### Task 1: Source context and exact name matching

**Files:** Create src/editor/source-font-name.ts; modify src/components/{FontPicker,ExistingTextDialog,TextParagraphDialog,StyledParagraphDialog}.tsx; optional existing font-picker.css; tests/source-font-name.test.ts, tests/font-picker.test.tsx and relevant existing dialog tests.
**Interfaces:** `sourceFontName?:string` is presentation only. Existing choose/apply callbacks remain unchanged. A small pure normalization/matching helper owns exact matching policy.
- [ ] Snapshot baseline source/native test inputs and touched files before production edits.
- [ ] RED: normalization/style/unsupported/duplicate/no-source cases; chooser order/context/lazy explicit apply; all three editor integrations and mixed selection omission.
- [ ] GREEN: implement minimal optional context and stable matching; keep existing search and button accessible names.
- [ ] Focused affected suites then fresh full frontend with raw JSON/log/source/input guards.

### Task 2: Normal Windows candidate and practical workflow

**Files:** artifacts/source-font-guidance-20261113/{baseline,source,build-1,desktop-1,qualification helpers}.
**Interfaces:** Exact declared frontend delta against baseline; unchanged native/shared inputs/resources. Compiled candidate bound to fresh source.
- [ ] Normal optimized custom-protocol GUI build, unchanged26renderer resources and source/input guards.
- [ ] Owned isolated Windows workflow using public source text: original source context, exact name match if available, no auto-selection, search alternatives, cancel preservation, explicit font preview/apply, edited PDF save/reopen and independent text check. Keep manual system paste/IME gate open.
- [ ] Private quiz may be checked separately; never place private text/input/output in public review or package.

### Task 3: Review and completion

- [ ] One fresh final review, no per-task agents; re-grade findings, one Important fix pass if required, ledger declined judgments/minors.
- [ ] Bind final source/test/build/runtime/review artifacts in a component checkpoint; report exact evidence and limits, keep broad goal active and prior packages unchanged.
