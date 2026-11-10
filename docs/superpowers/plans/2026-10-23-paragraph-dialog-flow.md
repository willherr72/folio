# Paragraph editor flow implementation plan

> Execute inline with superpowers:executing-plans. One focused fresh review and one fix pass.

**Goal:** Show the selected paragraph text immediately, with reversible adjustment of which PDF pieces it contains.

**Architecture:** Keep the validated paragraph/draft/candidate separate from temporary piece selection. Hide piece controls in editing mode. Adjust selection shows the original PDF and temporary pieces; Cancel adjustment restores the exact draft and candidate. Only a successful check of a different selection replaces the draft and releases its candidate. Preview/Apply remain unavailable during selection.

**Tech stack:** Existing React/TypeScript dialog and page preview; no native PDF changes.

## Context and decisions

Previous goal turn was progress: automatic paragraph suggestion source checkpoint verified in Windows with original quiz and rotated saved paragraph. Authoritative evidence: artifacts/paragraph-suggestion-verification.json, source fingerprint and cleanup. Reuse this linked worktree and preserve all prior binaries/packages.

Actual screenshot artifacts/paragraph-suggestion-verified-acceptance/quiz/preview.png shows the 250px piece list and redundant Check selection above the editor. Collapsing only the list would still leave selection changes destroying the typed draft and checked preview. Choose explicit editing and selection modes with reversible adjustment. Do not introduce a new editing engine, automatic application, or mixed-format support.

Editing mode shows font/selection summary, Adjust selection, paragraph text, area controls and page comparison. Selection mode shows pieces and Check selection. If there is an existing draft, explain that checking a different selection replaces it with the PDF text; offer Cancel adjustment. Same-selection confirmation preserves the draft/area/preview. Failed or canceled checks preserve them too; stale completions cannot commit.

Retain a candidate while selecting but force the original page in the preview without changing the user's comparison preference. Disable Preview/Apply until returning to editing. Explicit mode transitions move keyboard focus to the piece group or paragraph text. Closing the whole dialog still releases every owned source/URL once. Source changes invalidate old operations.

## Tasks

- [x] Add failing UI tests for text-first layout, adjustment cancel, same-selection check, changed-selection commit, rejected/delayed checks, focus and candidate ownership.
- [x] Implement modes, reversible adjustment and forced-original preview; preserve legacy manual selection and the64-piece limit.
- [x] Run affected/full frontend tests and production/typecheck build; one fresh scoped review with one fix pass.
- [x] Build an isolated copied Windows executable; verify actual quiz and rotated PDF save/reopen/independent output, plus usable text/actions at practical viewport sizes.
- [x] Record checkpoint/hash/cleanup and remaining gaps. Do not overwrite prior packages or declare overall parity.

Verified checkpoint: `artifacts/paragraph-flow-verification.json`. 684 frontend
tests in 100 files; TypeScript/Vite and native debug build pass. Actual Windows
quiz and rotated-PDF workflows plus independent PDF checks pass. Native source
fingerprint is unchanged from the preceding suggestion checkpoint. One review,
one fix pass, all owned jobs exited; original PDF and older packages unchanged.

Review focus: late native checks after cancel/source change; Apply during selection; leaked/double-closed candidates or URLs; unchanged-selection draft loss; keyboard focus and small-window primary controls. Native code remains at the previous verified fingerprint.
