# Editing interaction quality

The autonomous quality loop continues after the verified0.15.0-dev.1 OCR checkpoint.
This is a bounded pass over recorded usability/review gaps, using the existing React
and native architecture. The user has opted out of routine design questions; these
are implementation decisions, not new user requirements.

## Behavior

Paragraph editing accepts at most64 text pieces. Checkboxes already enforce this,
but the original-page SVG targets still announce clickable buttons. At the limit,
unselected targets must announce `aria-disabled=true`, leave sequential keyboard
navigation and ignore mouse/Enter/Space activation. Selected targets remain usable
for deselection; freeing a slot restores the other targets. Show the current count
out of64 and a concise status explaining how to free a slot. Use one frontend limit
constant for the dialog and preview; the native limit remains64. Disabled styling
must be visible without hiding the underlying PDF text.

Image import guidance must state the existing32,768-pixel limit per dimension as
well as32million total pixels and64MiB per image. Preserve current native behavior.

Cover remaining lifecycle cases: cancelling a paragraph after its candidate exists
but while annotated composition is pending must release the eventual URL and source
exactly once and never adopt it; unmounting an image dialog during its native picker
must not install the result or create a PDF; late Find results while an image-insert
dialog is open must not change its destination, and completed insertion must select
the first inserted page and close Find while preserving the query.

## Approach and scope

Keep the existing64-piece scope and async ownership guards. Raising the native limit
or redesigning paragraph selection would enlarge this pass without solving the
reported accessibility gap. Add targeted interaction regressions and only fix a
lifecycle implementation if those tests expose incorrect behavior. No native PDF
writer changes, dependencies, source-document mutations or replacement portable
archive. The verified checkpoint remains available; source builds gain these changes.

## Acceptance

Observe a failing preview interaction regression before fixing the limit behavior.
Run paragraph/image/app tests, full frontend suite and production build. Verify the
limit and layout in a real browser at900x600 using a clearly isolated fixture with
the real components and synthetic adapters. One fresh focused review covers this
whole pass; record any remaining limitations. Existing native340-test gate remains
relevant because no native code changes are planned.
