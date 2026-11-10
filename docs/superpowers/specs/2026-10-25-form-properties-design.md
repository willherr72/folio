# Existing form field properties

Previous goal turn made verified progress: saved-answer preservation checkpoint,
297 native/1215 frontend, real desktop and independent output checks. Source
a58aba09e189eda1a9b0f0ba3762f3ba714813450ba1e1c5738be02a8e2736a7 revalidated.
The broad Adobe/Foxit goal stays active. Standing user authorization opts out of
routine questions; implement this next everyday workflow inline.

## Behavior

In Fill forms, **Field properties…** edits an existing logical field's partial
name, tooltip/display label, Read only and Required settings. Explain that all
linked appearances change together. Show the full qualified name and linked widget
count. Keep values/defaults, export mappings, options, fonts, appearances, geometry,
page organization, overlays, bookmarks and opaque unrelated metadata intact.
Apply is one Undo step, with saved/reopened fields retaining the settings. Cancel
does not alter the document. Read-only fields can be unlocked by an authorized
editor; ordinary field value editing remains disabled while read-only.

Names are field identifiers, not visible page text. Only the terminal field's
partial name changes; preserve ancestor names and prevent duplicate fully qualified
names. New names: nonempty, no period/control/bidi formatting characters, <=1024
UTF-8 bytes. New labels: <=4096 UTF-8 bytes; allow ordinary multiline whitespace,
refuse unsafe controls/bidi formatting. Unchanged oversized inputs are not rewritten.
Only explicitly changed settings are sent. Empty label writes an empty alternate
name; the UI falls back to the identifier. Do not erase inherited flag bits:
materialize effective Ff at the logical field and change only bits1/2.

Native source-bound inspection returns expectedKey, name, fullName, label, readOnly,
required, linkedWidgets and nullable renameReason. Bounded AcroForm preflight already
rejects signed/XFA, ambiguous names and malformed ownership. Identify the terminal
field from the actual AcroForm tree and selected page widget, rather than trusting
only a Parent pointer. Preserve the graph and field IDs. Pure widget children share
their parent; a merged field/widget is its own logical field. Bound node depth/count
and decoded names/labels before allocation. No field type conversion or choices/default
editing in this cycle; those remain separate explicit capabilities in the parity goal.

## Rename dependencies

Preserve object-reference targets and calculation-order references. Update exact
fully qualified name strings in standard ResetForm/SubmitForm Fields and Hide T
targets, including arrays and indirect values. Do not rewrite arbitrary document
strings or mutate a shared string/array object globally. Copy a changed target value
locally at the action dictionary. Traverse direct object structures once per object,
with <=100000 objects/values, <=64 nesting and <=16 MiB of inspected/copied string data.
Check names and field attributes with the existing source budgets as well.

Renaming is unavailable when JavaScript, ImportData or an unsupported action with
unverifiable field dependencies is present. Other property edits remain available;
actions are retained and never executed. Expose the specific reason near the name
control. No silent script rewriting, script execution, or dropping actions.

Primary reference: [Adobe PDF Reference1.4, field dictionaries and actions](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.4.pdf)
and [Adobe Acrobat Forms API](https://opensource.adobe.com/dc-acrobat-sdk-docs/acrobatsdk/apireference/Acrobat_Forms_Layer/AcroForm.html).
T is a partial name; Ff is inherited; name-based actions can target fields, including
nonterminal names. These stable format semantics inform graph preservation, not a
claim of commercial parity.

## Architecture and choices

Use a small native metadata module plus worker/IPC integration. The chosen approach
edits the original lopdf graph and returns a complete immutable source version.
Recreating widgets with PDFium would regenerate appearances and risk loss of producer
metadata; per-widget properties would split linked identity. Neither fits the workflow.
Follow commitFormReplacement for source-wide frontend history, using captured tab,
document and adapter ownership; reject borrowed source IDs and close orphan results
with the producing adapter. No alternate page geometry or replacement occurrence map.

UI uses a compact settings dialog with loading/error/busy states, Reset and Apply.
No raster preview: these metadata changes do not paint the page. Values remain in
the existing Fill forms editor. Dirty value drafts must be applied/reset first.
Check edit-content and annotate permissions in the native worker and frontend.

## Acceptance

Native: merged/shared/nested fields, text/check/radio/choice, inherited flags,
Unicode names/labels, duplicates/stale keys, protected owner/read permissions,
action name arrays/refs/shared values/indirection, script rename refusal, preserved
values/defaults/AP/resources/geometry, save/reopen/later fill and bounded hostile inputs.
Frontend: loading, changed-only requests, reset, dirty value gate, shared scope,
rename explanation, readonly editing access, busy/error/cancel/stale tab/adapter/borrowed
result cleanup and one-step Undo/Redo. Real Windows checks at normal/narrow sizes,
save/reopen and independent PyMuPDF/pypdf field/appearance/pixel checks.
One final scoped review, parent fixes/no rereview. Retain failed evidence and prior
binaries, no commits/publishing/cleanup, one Cargo/build at a time. No release/parity
approval from scoped gates.

## Verified checkpoint and review decisions

Implemented and verified at `artifacts/form-properties-20261025/checkpoint-final`.
311 native passed/3 ignored, 1225 frontend passed; Windows and five independent
saved outputs passed. Exact evidence and limitations are in verification.json.
The three final-review P2 findings were reproduced and fixed. Unnamed terminal
fields keep tooltip/flag editing but refuse ambiguous renaming. Canonical bounded
names and full-name confirmation prevent encoding collisions and wrong targets.
Empty tooltips remain empty while native display-label confirmation uses its name
fallback. Desktop acceptance additionally fixed obsolete action objects affecting
later filling; only reachable actions are counted and every live action must survive.
This last parent-owned correction had regression and desktop gates, not a rereview.
