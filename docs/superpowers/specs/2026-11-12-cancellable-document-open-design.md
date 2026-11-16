# Prompt cancellation during document opening

Folio must return control when a user cancels opening a PDF or checking its
password. Existing `usePasswordWorkflow.cancel()` aborts the signal and removes
the prompt, but `open()` and `unlock()` await the native adapter before their
callers leave the busy state. A slow native reply therefore keeps the workspace
disabled after Cancel. Plain opening has no application Cancel action once the
native picker has closed. These are separate recovery gaps; this component does
not claim to explain or fix the intermittent OCR reopen timeout retained in
`artifacts/portable-046-20261112/component-completion.json`.

Use the existing React/TypeScript cancellation workflow and AbortSignal. A small
pure await/cancel helper must settle its caller promptly on abort, consume late
rejections, and dispose late successful values. Normal completion and errors keep
their current behavior. The existing native adapter continues cancelling password
challenges and disposing late native opens. Adapters that ignore the signal must
also have their late documents released by the coordinator. Owner unlock must
release late replacement sources. An old completion must never dismiss a newer
prompt or install an unwanted document. Do not retain passwords in state, logs,
workspace history, recovery records, or diagnostic packets.

Move the existing font/image acquisition and release preparation into the open
session as an optional preparation callback. Cancelling that preparation must
return promptly and never launch a picker when preparation eventually settles.
Preserve its ordering and resource ownership. Actual desktop acceptance exposed
an unparented Windows picker that leaves Folio enabled and survives app Cancel.
Inject the invoking Tauri window into `begin_open_pdf` and set it as the picker
parent. This is the only Rust change; client arguments, engine, dependency,
renderer, version metadata and password formats remain unchanged. The modal
picker owns native Cancel until selection; application Cancel remains available
after it closes or during preparation/password verification.

Expose an `opening` state from the coordinator. While opening without a password
prompt, show **Cancel opening** in the empty workspace or existing status bar.
The control remains usable while other document actions are blocked. Escape
should use the same action when no password modal owns the interaction. Restore
focus to the Open control when cancellation settles. Password prompts retain
their existing Cancel action. Cancellation leaves all existing tabs, dirty
history, Undo/Redo and selected page intact; late results cannot enter them.

Cancellation stops waiting and installation, with eventual resource cleanup. It
does not forcibly terminate a native parser/thread, guarantee instantaneous native
resource release, or prove that a stalled native service recovered. Preserve the
original OCR failure, observation and successful repeated runs as distinct facts.

Acceptance requires RED-to-GREEN regressions for prompt cancellation before a
reply, late successful/error cleanup, cancellation during preparation, immediate
new sessions and stale finalizers, owner verification, empty/existing workspace
controls, keyboard/focus behavior, and dirty document preservation. Run the full
frontend suite with the unchanged 5,000 ms timeout and two workers, TypeScript and
frontend build. Prove the exact five-path product delta against selected046 and
unchanged native/shared test inputs. Run fresh native suites with explicit profile
meanings for the native ownership extension. Require real picker ownership,
disabled parent and native Cancel recovery. Build a guarded ordinary
optimized development GUI/CLI candidate with all26 resources on D: and one Cargo
job. Test actual cancellation and late cleanup with an owned WebView and a real
native response held at the CDP response stage, plus the public OCR workflow.
Record any inability to establish real response interception rather than accepting
an empty observer as evidence.

Keep selected046 and all prior attempts intact. Use the existing isolated managed
worktree, inline execution and one final scoped review after actual verification.
Standing authorization replaces routine approval questions. No commits, pushes,
merges, public release, worktree/ledger deletion, or normal app auto-launch. This
is a development component, not a replacement portable package or parity claim.
