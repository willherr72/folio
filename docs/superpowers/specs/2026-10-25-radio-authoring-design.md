# Radio-group creation

Folio can fill existing radio buttons but cannot create them. Add a radio-group
choice to Create form field so a person can build mutually exclusive choices on a
flat PDF. Standing authorization permits routine design decisions and execution.

Chosen approach: create the complete group in one native transaction. Alternatives
were adding individual buttons to arbitrary existing groups (more flexible but
requires a separate group-discovery/definition-editing workflow), or rendering
independent checkboxes (does not provide radio semantics). The complete-group
approach delivers real exclusivity and one creation Undo using the existing editor.
Existing-group editing remains a separate capability, not an implied feature here.

The request gains `field: {kind:"radio", options:string[]}`. Require 2–32 distinct,
nonempty choices under the existing 256-character/1024-byte per-label and 32768-byte
aggregate limits. Display labels use the existing Helvetica WinAnsi repertoire.
Names and accessible group labels retain existing Unicode support and unique names.
Do not silently shorten labels or reduce their font size.

The placement rectangle encloses evenly spaced vertical rows, ordered top-to-bottom
in the current page view. Each widget includes a 10 pt circular control and a visible
label in both off/on appearances. Label text begins 18 pt from the row's left edge;
2 pt right inset. The caption baseline centers Helvetica's complete AFM vertical
extent (-225..931 units per 1000 em), including capital accents and descenders.
Rows need at least max(16 pt, fontSize*1.2+4 pt) height. Native
Helvetica advance metrics must fit every label; otherwise Preview explains whether
the group needs more width or height. Switching to Radio group gives a sensible
initial row height while preserving crop containment. Mouse, keyboard, numeric
placement, physical units and intrinsic/workspace quarter turns follow existing
field placement. The entire group stays within the visible crop.

One terminal `/Btn` field owns all widget `/Kids`; Radio and NoToggleToOff flags
are set, RadiosInUnison is not. `/Opt` stores the choice labels; each widget has
numeric on-state name matching its zero-based control index, plus Off. All start
unchecked and the parent value is Off. Fill forms continues using Toggle(true),
switching the group selection and retaining complete export labels. This aligns
with the renderer's index-backed Opt handling; validate on the bundled renderer,
not solely upstream source assumptions. Reference inspected:
[PDFium CheckControl/GetCheckValue](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdfdoc/cpdf_formfield.cpp).

Each row gets explicit vector/text appearances with local resources, clipped ink,
and a rotation matrix. No font-dependent radio symbol is needed. Reuse existing
Helvetica metrics and WinAnsi encoding rules. The group is independent of existing
field roots/resources; copied page occurrences isolate the chosen occurrence.
Preserve protected-source policies, existing fields, links, comments and bookmarks.

Creation returns all newly created widget indices/bounds internally. Native
validation inspects only those widgets with bounded per-widget budgets, checks the
group name/kind/flags/empty state/bounds and matching option export value, and never
enumerates unrelated fields. The public candidate/source transaction is unchanged.

Acceptance: first/middle/last selection replaces the previous choice; complete
labels and values survive filling, export and reopening. Test rotations/UserUnit,
duplicate occurrence isolation, invalid count/labels/geometry, no source mutation,
native preview, candidate disposal, one Undo/Redo, cancel, narrow controls and an
independent PDF reader/raster check. One scoped final reviewer, parent-owned gates.
No releases/commits or parity claims. Custom fonts, arbitrary button arrangements,
existing-group edits, calculations, XFA and signing remain separate work.
