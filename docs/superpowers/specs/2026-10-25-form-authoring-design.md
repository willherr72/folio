# Create fillable fields on an existing PDF

The password-security checkpoint is verified. The next missing everyday workflow
is making an existing or blank PDF fillable. Continue autonomously under the user's
standing authorization; preserve prior checkpoints and source PDFs.

## User workflow

Add **Create form field…** to the document tools. Open a page workspace with the
actual edited page, a draggable rectangular placement area and numeric position/
size controls, plus field properties. Support text (single/multiline), checkbox,
drop-down and list fields (single/multiple selection). A name and accessible label
are provided; required flag and physical font size are configurable. Text and choice
fields start empty; checkboxes start unchecked. The user can fill them immediately
through existing Fill forms after Apply. Preview must show the native candidate;
Apply only adopts that exact candidate in one Undo step. Changing any property
invalidates the candidate; cancel/error/stale replies release private sources.

Names are unique within the complete source, including hierarchical prefix conflicts;
new top-level names cannot contain a dot. Names/labels support Unicode with <=256
Unicode scalars and <=1024 UTF-8 bytes. Choice labels are unique nonempty strings,
<=256 scalars each, <=256 items and <=32768 aggregate UTF-8 bytes. No automatic
guessing of form areas or grouping unrelated fields by name.

Coordinates and font size are physical points. Place inside the retained workspace
crop; support intrinsic/workspace rotations and UserUnit through the existing source
geometry contract. Field text faces upright in the current workspace orientation.
Text/choice font size 1–300 pt, default 12. Fields are at least 4pt on each axis.
Transparent interior, black 1pt border and black text; new default font is standard
Helvetica/WinAnsi, kept in a uniquely named AcroForm resource. Checkbox uses vector
off/on appearances. Existing field/default resources must remain untouched.

## Native transaction and preservation

Native creation requires content-editing AND annotation rights; duplicated source
pages additionally require assembly. Fill-only permission never permits creation.
Use the same complete-source, selected-occurrence mapping as page replacement.
If a source page is repeated, append a preserved copy before changing only the
selected occurrence. Existing shared-field values/identity, annotations, comments,
links, bookmarks, content/resources, protection and overlay ownership remain intact.
Original bytes and runtime source do not mutate. Candidate publishing uses open_bytes
with restored policy; abandoned replies release candidate resources.

Parse through bounded existing AcroForm admission, rejecting signed/XFA/ambiguous
graphs. Validate before cloning direct resources. Add a new field/widget with its
own unique field name, local appearance resources and collision-free default-font
name; preserve the existing field array and page annotations by copy-on-write.
Reopen the candidate natively and verify field identity, kind, flags, bounds and
empty value before returning it. Existing export/recovery/filling paths remain used.

## Acceptance

Native tests prove all four types, filling and save/reopen, unique names and prefix
conflicts, readonly/protected rejection, repeated-page isolation, other field values,
annotation/resource preservation, physical scales/rotations, invalid limits and
abandoned-reply cleanup. Frontend tests prove geometry, preview ownership, stale
results, property changes, cancel, one-step Undo, source mapping and Fill forms access.
Full frontend/build plus relevant native form/protection/preservation suites. One
scoped final review, coordinator corrections with red/green tests, no repeat review.
Isolated Windows acceptance creates each type on a synthetic existing PDF, fills it,
undoes/redoes, saves/reopens and verifies appearance and values independently with
MuPDF/pypdf. Confirm untouched pixels outside new field bounds and source hashes.

This milestone adds common field creation. Existing-field design editing/deletion,
radio-group authoring, custom fonts, auto-detection, JavaScript calculations, XFA,
signing and complete accessibility/tagging remain explicit broader authoring gaps.

Reference: Adobe's documented form workflow includes text, checkbox, dropdown and
list creation on existing PDFs: https://helpx.adobe.com/acrobat/using/pdf-forms.html

Implementation ruling: names and accessible labels retain Unicode. Choice labels are restricted to the explicit Helvetica WinAnsi repertoire until field font selection is implemented; reject unsupported labels clearly rather than publish missing glyphs.
