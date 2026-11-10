# Paragraph automation implementation plan

Spec: docs/superpowers/specs/2026-10-27-paragraph-automation-design.md
Execution: inline, then one fresh scoped final review, under existing autonomy.
Preserve dirty linked worktree, originals, old artifacts and apps. No commits,
push, releases or cleanup. One Cargo process; locked/offline, shared target, jobs1.

## Task 1: CLI paragraph previews and reusable bundle publication

Create automation/paragraph_preview.rs and tests/paragraph_automation.rs. Start with
an actual missing-command RED for inspect/preview/save/reopen/re-edit. Strict request
DTOs and local palette binding; resolve via paragraph_inspection without changing
ID semantics. Refactor existing text_preview publication into a shared descriptor
and candidate publisher. Preserve exact v1 shape, add validated optional v2 metadata.
CLI Request/parse route preview-paragraph. Test real wrapping, source/font/highlight
fidelity, graph/other-page retention, password owner, stale/malformed/unknown style
refusal, exact copied candidate and old text preview compatibility. Expected: RED
then green paragraph/text automation and inspection targets.

## Task 2: MCP consecutive paragraph edits

Add subprocess RED for tool discovery and full paragraph preview/apply/edit/save.
Integrate protocol strict schema, private worker and shared preview limits/adoption.
Preserve existing apply/save/close/discard semantics; expose paragraph metadata from
validated manifest. Expected: twelve tools, new and prior MCP tests green.

## Task 3: review and saved-output acceptance

Update automation guide. Freeze baseline/snapshot delta and hashes for one independent
final review. Run native library + CLI/MCP + both inspection/automation targets.
Correct important review findings with failing reproductions and green final gate.
Build portable matching-source CLI. Official SDK legacy/modern plus independent
PyMuPDF/pypdf examine saved Word/mixed/highlighted reflow, re-edit, graph and pixels.
Preserve failures and gate evidence, update quality record/checkpoint/next action.

Review focus: source and local-style binding across workers; optional null/unknown
fields and byte budgets; exact v1 compatibility/v2 schema; candidate ownership and
permission/graph preservation; fitting vs fixed bounds and mixed highlights; native
grapheme rejection; response/publication ordering; lifecycle/capacity cleanup.
