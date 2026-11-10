# Toolbar visibility

The autonomous goal continues after verified portable0.17. The previous turn was
progress: image controls, review fixes, real Windows and packaged acceptance passed.
Actual screenshots show whole-toolbar horizontal scrolling clipping Save and other
primary actions. Existing markup places creation, editing and utilities in one row;
styles.css reserves53px and review.css enables horizontal overflow.

## Approaches and decision

Keeping one row and shrinking labels further would hurt legibility. An overflow
menu would hide editing capabilities and add menu behavior. Use two logical rows:
document creation/opening and utilities first, editing actions second. Let editing
actions wrap when necessary. This is the agent's design choice under the user's
explicit autonomous-work authorization, not a separately approved preference.

## Behavior and constraints

- At900x600 and1280x820 CSS pixels, all document actions (including Save options),
  Undo/Redo, zoom, Find and Print are inside the viewport without toolbar scrolling.
- Editing labels and icons remain readable and do not shrink or wrap internally.
  The editing row may wrap as whole buttons; include the reader's Unlock editing.
- Toolbar height follows its contents. The remaining workspace keeps bounded
  scroll containers, with no page/body horizontal overflow at supported widths.
- Preserve command handlers, permission/disabled states, keyboard shortcuts and
  Save menu focus/anchor behavior. Use named groups and ordinary Tab navigation;
  do not introduce a new roving-toolbar keyboard model in this layout pass.
- Native page content, image ownership and ready portable0.17 stay unchanged.
  No release/version change, commit or publication is part of this source pass.

## Acceptance

Use a synthetic fixture of the real App and adapters with no native file writes.
Reproduce the old overflow in a real browser, then measure every toolbar button
at900x600,1280x820 and a larger window, including a protected reader and dark theme.
Check browser zoom/reflow, keyboard reachability, and opening/closing Save options.
Run the existing frontend suite/build and one fresh focused review. Native writer
tests need not repeat for unchanged native code. Record layout evidence and limits.
