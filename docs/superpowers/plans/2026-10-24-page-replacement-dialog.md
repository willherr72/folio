# Page replacement dialog implementation plan

Execute inline with superpowers:executing-plans, test-driven-development, and one
final scoped review. Existing isolated worktree and autonomous authorization apply.

Spec: docs/superpowers/specs/2026-10-24-page-replacement-dialog-design.md

Status: all five steps completed. Evidence and explicit limitations are recorded
in `.superpowers/sdd/2026-10-24-page-replacement-dialog/progress.md` and
`artifacts/page-replacement-dialog-20261024/checkpoint-accepted/verification.json`.

1. Add failing model/adapter tests. Implement PageContentReplacement type,
   occurrence request/validated commit helper, native IPC and adapter wiring.
2. Add failing dialog lifecycle tests. Implement native PDF choice, incoming page
   selection, current/result previews, Apply handoff, stale/cancel cleanup, and
   accessible responsive layout using the existing Modal and composed renderer.
3. Add App integration coverage. Wire Page actions, frozen request, modal blocking,
   permissions and atomic history update. Verify undo/redo and retained page state.
4. Run focused then full frontend verification and native desktop build. Exercise
   the actual Windows workflow, save/reopen/recovery, source hashes, and independent
   PDF text/pixels/interaction checks. Record explicit remaining limitations.
5. Obtain one final scoped review; reproduce/correct important findings once, run
   affected checks, and update the persistent quality ledger. Keep parity pending.
