# Paragraph entry focus

The current desktop candidate reproduces two keyboard failures: transitioning
from Edit text to Edit paragraph focuses Zoom out; direct paragraph entry leaves
focus on BODY. Both open the editor correctly, but the next keystroke misses it.
Probe evidence: artifacts/paragraph-entry-focus-20261030/probe-1.

Use the existing shared ParagraphWorkspace as the focus owner. On mounting a
new workspace, focus its named panel immediately, including while inspection is
pending. When initial inspection settles, move focus once to Paragraph text or
the paragraph-piece fieldset after a refusal, provided focus is still on that
panel. If the user has moved to another control, retain their choice. Later
draft updates, previews, adjustments and font dialogs retain their existing focus
behavior. Closing continues to return to the appropriate editing tool.

Pass initial loading state from both styled and uniform paragraph dialogs. Use
the workspace's existing semantic text/fieldset elements as targets; no new
dependency, global focus trap, modal behavior or native PDF change is needed.
Changing Modal's global fallback would affect unrelated dialogs and would not
address direct paragraph entry. Separate autofocus in each editor would duplicate
ownership and risks moving focus again after later async updates.

This is a coordinator-selected approach under the user's standing autonomous-work
instruction. Verify both editor types, delayed success/refusal, deliberate focus
movement, unmount-before-completion and nested picker restoration. Desktop checks
must use real Enter/Tab input, record focus without assigning it after entry, and
cover both routes plus a narrow window and cancellation. No screen-reader-wide
or commercial-parity claim follows from focus checks alone.
