# Sticky-note fidelity and native controls

Evidence: `artifacts/comment-fidelity-red.log` demonstrates that saving an untouched
F4 Text annotation replaces its author with Folio, replaces dates and bounds, and
drops its custom appearance, icon name and popup. This contradicts ordinary PDF
editing expectations even before the user edits the note.

Keep every saved Text annotation native on normal open, including notes created by
Folio. Newly added workspace comments remain overlays during the current editing
session; after export/reopen they use the native comment editor. No new private
ownership marker is needed. Highlights and owned text/ink/image import stay as-is.

Native comment controls support plain-text editing, numeric Left/Top positioning
in the displayed page's coordinates, and deletion. Where no custom AP exists,
allow changing the standard icon color by changing C. Custom appearances retain
their colors with an explanation. Preserve all other annotation fields, appearance
bytes and popup geometry. Popup-parent repair remains active during composition.
Moving a note updates only its Rect; deleting removes its page placements and
associated popup placements without rewriting other annotations. Shared workspace
copies update together as one undo step. Refuse deletion of a note referenced by
reply annotations. Locked, hidden, rich-text/state/reply and signed cases retain
their existing read-only guards. Native annotation permissions are authoritative.

Recovery compatibility is required. Add a per-source legacy-comment-import mode
to immutable runtime sources and recovery snapshots. Old manifests default to the
legacy mode, which strips comments already owned by saved overlays. New ordinary
sources use native comments. Full-source form/comment replacements inherit the
source mode; generated page sources whose bytes already omit imported overlays
use native mode. New manifests use annotationImportVersion3 and require explicit
per-source mode. Old software must refuse newer checkpoints rather than misread
them. Upgrade old cached snapshots with the actual runtime mode on save.

Keep original PDFs, portable packages and prior uncommitted work untouched. No new
dependencies, version bump, commits, or publishing. User authorized autonomous
implementation; execute inline with one fresh focused review at completion.

This improves metadata fidelity and native note controls; it does not claim full
comment parity. Thread authoring, rich text, arbitrary appearance recoloring, and
on-page native-note dragging remain later work.

## Review correction: downgrade isolation

A version bump alone is insufficient: older readers fall back over unsupported generations and may garbage-collect them. Application recovery therefore lives in a separate `recovery/comment-format-3` slot, while holding the original recovery lock for compatibility. On first use, an existing legacy draft is restored and durably checkpointed in the new slot before retiring the old slot. Later work created by an older app remains separate; it is not automatically merged into a newer draft. Current load/save/garbage collection refuse newer version headers before strict schema parsing or cleanup. Explicit discard remains available.

Deferred fidelity detail: native position/color-only updates currently normalize Contents to UTF-16 while preserving its decoded text. Exact unchanged string representation is a recorded minor gap.
