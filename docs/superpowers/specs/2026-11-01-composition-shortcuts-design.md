# Composition-safe dialog and search shortcuts

Protect dialog drafts and search position while an input method owns Escape,
Enter or Tab. The shared Modal currently closes on composition Escape and
SearchBar closes/navigates on composition Escape/Enter. App and FormFieldPopover
already ignore `isComposing` or legacy key code 229.

Use that existing event guard after stopping propagation in Modal and SearchBar.
Leave default handling to the input method; ordinary keys retain existing behavior.
An application-wide rewrite would touch unrelated handlers without evidence;
per-dialog patches would duplicate the shared fix and miss other dialogs.

Acceptance: regressions for both event representations, preserved editable
document-properties draft, ordinary close/focus restoration, search navigation,
and actual keyboard traversal/edit/Apply at 840 by 640 in an owned browser harness.
Use the real React components, mock only the external metadata inspector. Browser
composition dispatch is synthetic and does not certify an installed OS input method.

No PDF engine, metadata serialization, dependencies or package/version changes.
Preserve existing packages, documents and inherited work. The user's autonomous
operating agreement permits proceeding without another approval checkpoint.
