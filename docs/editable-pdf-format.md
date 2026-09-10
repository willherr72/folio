# Editable PDF additions (Folio 0.6)

**Save a copy** and Ctrl+S create an ordinary PDF with editable additions. The PDF
contains all the data Folio needs to reopen those additions; it does not depend on
original file paths, sidecars, a local database, or embedded copies of source PDFs.
Source page content remains vector content where the source provides vectors.

## What other readers see

Added text uses PDF FreeText annotations. Drawings and drawn signatures use Ink
annotations. Each has a self-contained vector appearance stream, so readers can
display it without understanding Folio metadata. Text appearances use Helvetica
and the verified Latin/WinAnsi character repertoire supported by Folio's exporter.
Unsupported characters produce an export error instead of disappearing.

Highlights and comments continue to use standard Highlight/Text annotations.
Drawn signatures are visual ink marks, not certificate-based signatures.
PDF readers can differ in how they expose annotation editing controls.

## Saving versus flattening

The Save button's arrow offers **Flatten text and ink**. It paints added text,
drawings and signatures into page content, removing their Folio editing handles.
Highlights and comments remain annotations. This is intentionally a text/ink
flattening option, not a promise that the entire PDF is uneditable. It preserves
source vector content and adds vector text and paths, rather than rasterizing pages.

A flattened export leaves the current editable workspace's saved state unchanged.
Use ordinary Save a copy to keep a portable editable version of those additions.

## Metadata and coordinates

Owned FreeText/Ink annotation dictionaries have a `/Folio` JSON string containing
format `version: 1`, an overlay, the original page coordinate frame, and a SHA-256
appearance fingerprint. Standard `/AP /N` streams contain the actual drawing;
standard annotation geometry, content, color and stroke fields accompany them.
The metadata is ordinary document data, not a cryptographic signature or an
independent trust guarantee.

The coordinate frame records the source crop bounds and intrinsic rotation.
Folio converts geometry through raw PDF coordinates when reopening. Text also
records clockwise orientation around its top-left anchor. This preserves
placement when pages are rotated, duplicated, reordered or combined with others.

On import, Folio validates the version, bounded metadata, geometry, supported
styles, public annotation fields, appearance bytes and resources. It removes an
annotation from the in-memory source rendering only after successful import,
then draws it as an editable overlay. The original source bytes remain immutable.

Unknown versions, malformed metadata, unsupported flags or appearance changes
made by another reader remain native annotations. Their appearance is retained;
Folio does not silently replace them with potentially stale editing data. A reader
that rewrites even an equivalent appearance can therefore leave an addition
visible but without Folio editing handles. Standard highlights/comments use their
existing conservative import rules.

## Recovery compatibility

The local recovery manifest's annotation import marker advances to `2`. Older
workspaces acquire annotations that their original Folio version could not edit:
marker `0` imports the supported annotations introduced in 0.5 and 0.6; marker `1`
imports newly supported owned text/ink only. Marker `2` respects the saved overlay
list, including intentional deletion. Recovery remains a local crash checkpoint,
separate from saving this portable PDF.

## Regression evidence

Native persistence tests cover repeated edits and deletions, source independence,
rotation/crop geometry, malformed or stale metadata, explicit flattening, and
recovery migration. Browser tests cover rotated text display, selection, search,
thumbnail placement and dragging. The desktop persistence smoke exercises the
built Windows app through native dialogs. See the release validation report and
[corpus guide](corpus/README.md) for commands and measured limits.