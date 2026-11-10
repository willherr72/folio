# Organizer history focus ownership

Native `artifacts/organizer-shortcut-focus-20261030/probe-1` confirms Ctrl+Z on focused Apply or Undo and Ctrl+Y on focused Redo disable the control and leave BODY focused. Escape no longer reaches the modal. Ctrl+Z on Cancel preserves focus and Escape works. The preceding click-only correction did not cover local shortcuts.

Capture the focused control that initiates history travel. After the resulting frame is committed, return to the frame's selected page only if that control has become disabled/disconnected and focus remains on it or BODY. Retain an available control's focus and do not steal focus from another user-selected control. Use the existing virtualized page reveal/focus mechanism. Grid-origin shortcuts keep their existing behavior; typing fields keep native text undo. Remove the click-only history-length special cases. No global key interception or modal changes.

Prove exhausted click/shortcut behavior and still-enabled controls with regressions; cover both Ctrl+Y and Ctrl+Shift+Z, Apply, Cancel, typed fields and virtualized selection. Run full frontend/type/build gates, one scoped review, and native normal/narrow acceptance with the source/candidate/helpers bound. Then advance the combined editing/save/reopen workflow. Preserve prior failures/packages/originals and keep the broad goal active.
