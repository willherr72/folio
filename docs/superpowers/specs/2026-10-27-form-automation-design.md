# Existing form automation

User direction: keep developing Folio autonomously; existing-PDF fidelity and
CLI/MCP automation matter. Coordinator choices below use that authorization.
Previous turn made progress: paragraph automation accepted with stronger saved
attribution evidence. Preserve all originals, dirty work and old artifacts.

## Approach

Reuse native list_form_fields/fill_form_field, then the shared immutable full-source
preview publisher and Save/Apply lifecycle. Direct PDF dictionary edits would
duplicate the native answer/appearance proof. Flattening would remove future form
editing. Neither meets this workflow. Implement inline and use one scoped final
review under the established execution method; no routine approval gate or commit.

## Interface and identity

CLI inspect-forms INPUT --page N [--offset N]; preview-form INPUT --request EDIT.json
--out-dir EXISTING. MCP folio_inspect_forms(documentId,page,offset) and
folio_preview_form(documentId,page,fieldId,value). Save/apply/discard stay shared.
Inspect returns native field records plus fieldId, dimensions, source fingerprint,
offset/nextOffset/totalFields. Identity is domain-separated SHA256 of source bytes,
1-based page and bounded native field serialization (including annotation index).
Reinspect and resolve exactly one match before filling; derive expectedValue from
the fresh native record. No caller annotation indices, resources or expected values.

Values are strict tagged text{text}, toggle{checked}, choice{indices}; empty text
and an empty list are permitted by the DTO, with native kind-specific validation.
Normalize CRLF/CR using existing native semantics. Native permissions apply: listing
requires fill or copy; filling requires fill, not content edit. Hidden/read-only,
radio deselection, invalid choices and unsupported content remain native refusals.
Related widgets/radio siblings may change across pages as one logical field; preview
images show the selected page and the full saved PDF retains all native updates.

## Bounds and publication

Input request at most256KiB serialized, text at most32768 UTF-8 bytes after newline
normalization (matching inspectable native text), choice at most2000 ordered unique
indices. Native UTF-16 and field restrictions still apply. No truncation.
Inspection: each serialized field at most512KiB, adaptive pages up to25 records and
512KiB inner result. Full CLI/MCP inspection envelope at most2MiB including newline,
escaping and duplicate text representation; oversized individual records refuse.
Native retained inspection budget remains8MiB. Stable offsets are over native order.

Preview manifest v3 adds optional form metadata {oldValue,value}; selectionId is the
fieldId. Legacy text-summary strings are empty for v3; v1/v2 keep their exact shape
and64KiB manifest limit. v3 manifest at most512KiB, request retention at most256KiB.
All metadata remains inside the shared512MiB bundle/session limits. v3 validation
requires canonical values, valid page/fieldId, no paragraph metadata, no text summary.
Old versions cannot carry form metadata. Shared candidate hash/exclusive saving and
immutable Apply remain. Native fill returns a complete DocumentInfo; publish with
the original full page order, retain encryption, reopen and render saved bytes.

## Acceptance and limits

Actual CLI/MCP inspect/fill/preview/apply/reinspect/fill/save on text, multiline,
checkbox/radio and choice fields. Verify originals, retained field values/resources,
other fields/page graph, protected fill-only permission and refusal without fill.
Test stale IDs, nested unknown fields/nulls, mismatched kinds, read-only fields,
wire limits, manifest tampering and preview cleanup. Independent readers must bind
saved answers and appearance checks to intended fields, including group side effects.
No native preservation policy changes, form authoring, field formatting, batch fill,
MCP password entry or desktop UI change in this milestone. No parity/release claim.

MCP input frame ceiling512KiB including newline replaces64KiB, to carry an escaped
256KiB form request plus protocol metadata. The bounded32-frame queue remains.
