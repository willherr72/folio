# Paragraph paste implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline. User opted out of check-ins; preserve managed worktree and do not commit/clean up. One final combined review.

**Goal:** Preserve multiline clipboard content when moving existing-text editing into its paragraph workspace.
**Architecture:** Pure bounded paste construction, a staged dialog action, and the established source-bound styled draft transfer. No native API changes.
**Tech Stack:** TypeScript/React/Vitest and existing Windows owned-app harness.
**Spec:** docs/superpowers/specs/2026-11-13-paragraph-paste-design.md

## Global Constraints

4096UTF8bytes paragraph capacity;1000UTF16units ordinary text input; only LF hard breaks after CRLF/CR normalization; reject unsafe controls/surrogates/grapheme endpoints. Preserve selected0.50 and originals, dependencies/Rust/renderer/permissions/version. No commits/deletion/merge/push/public release; no private corpus. Serialize frontend/build/native/GUI with two frontend workers/default5000ms, one Cargo job andD:cache/temp. One final review/one Important fix pass/no re-review.

## Review Focus

- A later edit/paste cannot use a stale captured selection or replacement.
- Font picker/remount and failed async preparation retain correct font ownership and pending content.
- CRLF normalization occurs before byte/range accounting; emoji/combining characters cannot be split or truncated.
- Native source mapping/paragraph siblings, local history and document Undo remain faithful with inserted hard/blank lines.
- Text-only/older adapters and busy/session changes cannot apply pasted content through an unsupported route.

### Task1: pure paste and hard-break transfer

**Files:** new src/editor/paragraph-paste.ts and tests/paragraph-paste.test.ts; modify src/editor/paragraph-draft.ts and tests/paragraph-draft.test.ts.
**Interfaces:** prepareParagraphPaste(text:string,selection:{start:number;end:number},clipboard:string) returns immutable {replacement:string,selection:{start:number;end:number;direction:'forward'}} or throws a plain non-content error. transferStyledDraft accepts LF and bounded4096UTF8bytes while retaining exact mapping/font/history.

- [x] Write pure regressions for caret/range splice, normalized CRLF/CR, blank lines, original prefix/suffix, grapheme-safe emoji, invalid ranges/controls/surrogates, exact capacity and no mutation. Add transfer tests for hard/blank lines, style siblings, one Undo/Redo and byte overflow.
- [x] Run npm test -- tests/paragraph-paste.test.ts tests/paragraph-draft.test.ts --maxWorkers=2 --minWorkers=2; observe RED before implementation, then GREEN.

### Task2: staged dialog action and App ownership

**Files:** src/components/ExistingTextDialog.tsx; tests/existing-text-dialog.test.tsx and tests/occurrence-paragraph-app.test.tsx.
**Interfaces:** pending paste is local immutable replacement/range; existing onEditParagraph(ExistingTextDraft) is unchanged. Existing parent leases/checks govern transfer; preserve the current input when it fails.

- [x] Add RED dialog/App tests for multiline paste retaining selected surrounding input, explicit callback only, busy/unavailable paths, typed/second-paste invalidation, current font, failed mapping and local paragraph text/history. Then add the staged Paste as paragraph action with accessible error/status text.
- [x] Run focused tests above plus Task1 tests, paragraph/editor font/session regressions; require GREEN and inspect exact assertions.

### Task3: complete acceptance and review

**Files:** artifacts/paragraph-paste-20261113; this plan/ledger and docs/product-quality-loop.md after acceptance.

- [x] Capture exact product/test/native/shared inputs and selected050 identity. Fresh full frontend (default5000ms/two workers), npm run build, normal optimized separate GUI candidate with all26resources. Assert native source/resources byte-identical to selected050, not a fresh native suite.
- [ ] Run actual Windows clipboard multiline paste at a nontrivial selection, verify no single-line write, paragraph hard breaks and caret/font ownership, Preview/Apply, Undo/Redo, save/reopen and Cancel/failure. Independently compare complete saved text/glyphs and unaffected page pixels on public fixtures. Verify owned cleanup; do not launch normal user app or promote a package.
- [x] Freeze scope/evidence and request one final combined review. Fix Important/Critical once with RED/GREEN and green suite; retain all failures, rule on every declined behavior, defer Minors, no re-review.
- [ ] Close the real system-clipboard acceptance gate before recording complete component acceptance. Automated event-path acceptance and implementation are complete; Ctrl+V/IME remains unverified after host OpenClipboard error5. Broad goal stays active.

Post-review evidence: source46cba770167b14038b24706469659b15daeb3272a56cb41d0cc1d81e06c97836;
frontend-full-4:1601passed/232files; focused119passed/7files; build-4 normal optimized
GUI; desktop-6:3saved/reopened witnesses,1capacity refusal,3independent corruption
controls,owned shutdown. See artifacts/paragraph-paste-20261113/post-review/qualification.json.
