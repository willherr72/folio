# Text area copying contract

On October 22, 2026, the user approved accepting documented whitespace differences
in external PDF readers after reviewing the [Microsoft Word comparison](word-copy-comparison.md).
This supersedes the September requirement for identical external copying. The
target is exact logical text within Folio and readable, complete text in external
readers. It does not enable the desktop text-area tool or complete issue #16.

## Exact within Folio

The original logical Unicode remains authoritative: repeated spaces, nonbreaking
spaces, LF/CRLF, blank rows, trailing separators, combining marks and character
order must survive editing, undo/redo, editable save/reopen and recovery. Automatic
wrapping inserts no source characters. Folio's area copying and search must use
that logical source rather than reconstructing it from external-reader extraction.
Font, area geometry, layout and visible appearance must retain their existing
verification requirements. Overflow remains explicit; silent clipping is forbidden.

The native layout checkpoint already checks exact source ranges and serialization.
The persisted area schema, editor integration and end-to-end Folio copying/recovery
tests are still implementation work. A green external check does not claim those
features exist. Flattened export must meet the external-reader policy; it is not
promised to restore Folio editing metadata.

## External reader policy

The verifier calls this policy `whitespace-v1`. For comparison only, it replaces
each nonempty run of ASCII spaces, NBSP, LF and CRLF with one ASCII space and ignores
such whitespace at the beginning and end. The resulting strings must be identical
and nonempty. Neither source data nor raw reader results are rewritten.

This permits repeated spaces to collapse, NBSP to copy as an ordinary space,
automatic wraps to copy as line breaks, LF/CRLF differences, and loss or addition
of blank lines and edge whitespace. Those distinctions remain exact within Folio.

All other Unicode scalars and their ordering remain exact, including punctuation
and combining marks. Tabs, bare CR, Unicode line separators and zero-width
characters are not silently normalized. Words cannot disappear, merge or split:
`A\nB` may copy as `A B`, but `AB` fails. Even though Word exhibits that short-line
failure in PDFium, it is not accepted by this policy. Whitespace-only exports
remain outside the current shaper's supported subset.

The check requires fresh PDFium full-range text, actual MuPDF selection text and
pypdf extraction, both before and after save. Missing evidence cannot fall back
to another operation. Exact source ranges, unmodified sources, measured geometry,
raster and raw reader stability across the roundtrip remain required. Interactive
selection geometry and real-viewer acceptance remain separate release work.

## Verifier behavior and evidence

`scripts/inspect-text-area.py` defaults to `--copy-policy reader-compatible`.
`--copy-policy exact` retains the old raw-equality check. Both policies report
`exactCopyGatePassed` and `readerCompatibilityGatePassed`; `copyGatePassed` reflects
the selected policy. Raw strings and their exactness flags are preserved.
`--diagnostic` permits reporting a failed copy check only when the underlying
fixture evidence is valid. `releaseAcceptance` remains false for this development
inspector, including when the selected copy check passes.

Reinspection of the existing native fixtures gives **7/7 reader-compatible cases,
0/7 exact cases**, with two overflow cases correctly refused. The native PDF files
were generated previously; this result is a fresh inspection, not a new native
build or export. [Full report](issue16-reader-compatibility.json).

Fresh reads of the original/resaved Word controls pass 10/12 cases under this
policy. The two tagged/untagged short hard-break controls still fail because
PDFium returns `AB` for `A\nB`; Word behavior does not automatically define acceptance.

All **31 Python diagnostic tests** pass. The new coverage accepts the named
whitespace variations and rejects joined/split words, dropped/duplicated/reordered
content, Unicode changes, missing readers, missing original/resaved selection,
invalid source ranges and failed roundtrip evidence. The exact-policy CLI still
exits 1 on these fixtures; the default policy exits 0. Exact diagnostic mode exits
0 while retaining `copyGatePassed: false`.

```powershell
python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py'
python -B -X utf8 scripts/inspect-text-area.py --expected-cases 9
# Historical exactness diagnostic: expected exit 1 with the existing fixtures.
python -B -X utf8 scripts/inspect-text-area.py --expected-cases 9 --copy-policy exact
```

Use `--input` to select a saved native fixture directory and `--output` to retain
separate reports. The next implementation milestone is the persisted text-area
model and its exact Folio save/reopen/recovery tests, followed by editor controls.
