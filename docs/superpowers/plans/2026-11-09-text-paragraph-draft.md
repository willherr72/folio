# Text-to-paragraph draft implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, task by task. The existing worktree and user authorization apply; no commits or approval gates.

**Goal:** Keep a typed replacement and chosen font when moving from existing text to paragraph editing.

**Architecture:** Prepare the checked paragraph and an immutable local editor state before closing the original dialog. A bounded native operation maps the selected source object into that paragraph, including verified saved replay; normal inspection/publication DTOs remain unchanged.

**Tech stack:** React, TypeScript, Vitest, Rust, existing serialized PDF worker and PDFium.

**Spec:** `docs/superpowers/specs/2026-11-09-text-paragraph-draft-design.md`

## Global constraints

- Original PDFs, accepted 0.35, its launcher, archive and evidence remain preserved.
- No writer, renderer, dependency, security-policy or CLI/MCP payload changes.
- Paragraph limits: 4,096 UTF-8 bytes, 64 styles, at most 4,096 verified saved pieces; existing fresh/form limits remain.
- UTF-16 external endpoints must be valid character boundaries and exact substring matches.
- Serialize suites, compiler, native desktop and resource observers. Fresh native changes require fresh verification.

## Review focus

- Identical text in adjacent source objects must change only the selected owner (task 2).
- Trimmed padding and generated separators must not duplicate or erase user whitespace (task 2).
- Saved hard/blank lines and justified fragments must retain exact logical mapping (task 3).
- An async response after source/tab changes must not install a stale draft (task 4).
- A substitute font must survive old-dialog cleanup and local Undo/Redo, then release on Cancel (task 4).

### Task 1: Reproduce the lost draft

**Files:** `tests/existing-text-dialog.test.tsx`, `tests/occurrence-paragraph-app.test.tsx`.

- [ ] Add a dialog test: type `Original this text`, set selection 9..13, click Edit paragraph, expect immutable payload `{originalText,replacement,selection:{start,end,direction},font:null}`.
- [ ] Add an App regression: mapped leading piece becomes `one this two`, sibling styling stays unchanged, Preview retains original expected text.
- [ ] Run the focused tests before implementation and preserve their failing output under the new artifact root.

### Task 2: Draft projection and editor transaction

**Files:** new `src/editor/paragraph-draft.ts`, new `tests/paragraph-draft.test.ts`; existing styled editor/model exports as needed.

**Interfaces:** `ExistingTextDraft`, `ParagraphDraftMapping`, `prepareParagraphDraft(adapter,sourceId,pageIndex,anchor,draft)` returning a prepared styled editor state. Mapping carries source/page/membership/anchor/exact texts plus `sourceStart/sourceEnd/paragraphStart/paragraphEnd`.

- [ ] Write failing pure tests for repeated pieces, Unicode/grapheme boundaries, preserved unchanged styles/highlights, leading/trailing padding, malformed identity/ranges, capacity and one local Undo item.
- [ ] Implement immutable payload validation, bounded grapheme diff and source-padding projection; apply edits from right to left to preserve unchanged formatting. Keep original native expected text.
- [ ] Support safe styled single-piece fallback; reject ambiguous unsupported mapping and changed handoffs into older uniform-only adapters before closing the original dialog. Official native styled editing covers uniform PDF formatting too. This explicit correction addresses review R1/R2 rather than adding a second local-history subsystem.
- [ ] Run `npm test -- tests/paragraph-draft.test.ts --maxWorkers=2 --minWorkers=2`; require all passing.

### Task 3: Native read-only source mapping

**Files:** new `src-tauri/src/paragraph_draft.rs`; modify `text_paragraph.rs`, `paragraph_memory.rs`, `styled_paragraph_memory.rs`, `occurrence_paragraph.rs`, `form_paragraph.rs`, `types.rs`, `app.rs`; new `src-tauri/tests/paragraph_draft.rs`.

**Interfaces:** `PdfEngine::map_paragraph_draft` and `map_form_paragraph_draft`; commands `map_paragraph_draft`, `map_form_paragraph_draft`. Both return only source-bound text/range metadata. Internal helper reuses fully checked paragraph baseline/projection and exact source range provenance.

- [ ] Write native tests for fresh repeated/multibyte/trimmed objects and stale expected text; run to establish red.
- [ ] Add bounded request/response DTOs, fresh mapping, mapping-enabled verified saved uniform/styled replay, and occurrence/form handlers. Never expose private projection IDs or change ordinary serialized inspection/layout.
- [ ] Prove saved logical correspondence by complete ordered consumption of verified emitted fragments, skipping only logical LF hard breaks; require exact source/fragment substrings and no unconsumed visible characters. If existing layout can omit any other characters, carry private atom provenance instead.
- [ ] Add restored hard/blank lines, wrap/justify, permissions, form ownership and source preservation regressions.
- [ ] Run focused native target, affected paragraph/replay targets, then appropriate fingerprint-bound full gate with existing profiles. Do not reuse 0.35 receipt as fresh evidence.

### Task 4: Install prepared draft with session/font ownership

**Files:** `ExistingTextDialog.tsx`, `App.tsx`, `StyledParagraphDialog.tsx`, `TextParagraphDialog.tsx`, `adapter.ts`, `form-paragraph.ts`; existing dialog/App/form tests.

- [ ] Change callback to `(draft:ExistingTextDraft)=>void`; capture exact source text, replacement, caret and optional font. Unchanged original-font entry keeps old path.
- [ ] Add adapter commands/form-bound mapping; prepare before closing old modal. Acquire a parent font lease synchronously before await; transfer to session ownership only after owner checks succeed.
- [ ] Add optional prepared initial state to the styled paragraph editor; install once, load FontInfo before editor leases, prevent late suggestion overwrite. Reset/selection adjustment restores originals without reapplying the transfer. Remove incomplete uniform-only initial installation and preserve its unchanged entry route.
- [ ] Test preparation failure preserves input/font; stale source/tab/unmount rejects; Cancel releases; transferred change is one local Undo/Redo item. Test unchanged direct paragraph route remains identical.
- [ ] Run all affected tests, full frontend suite with two workers/default timeout, and `npm run build`.

### Task 5: Compiled acceptance and independent review

**Files:** new evidence/harness files only under `artifacts/text-paragraph-draft-20261109`; existing private source remains private.

- [ ] Freeze product/tests/shared fixtures and record precise fresh gate results. Build a separate optimized GUI candidate.
- [ ] On the original quiz, type after a leading-fragment word, observe overlap refusal, enter paragraph editor without retyping, Preview/Apply, Undo/Redo, save/reopen. Audit original fonts and unaffected glyphs/pixels using source-defined exclusion areas.
- [ ] Exercise explicit font transfer and reusable forms on generated public fixtures; clean only owned test processes/profiles and verify absence.
- [ ] Request one scoped final review, resolve reproduced findings, record limitations and exact evidence. Keep broad goal active and 0.35 launcher unless separately packaging/qualifying a successor.
