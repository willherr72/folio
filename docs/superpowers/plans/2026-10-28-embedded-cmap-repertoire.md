# Embedded CMap Repertoire Implementation Plan

> Use superpowers:executing-plans inline, TDD and one final scoped reviewer.
> Do not begin product changes until the standard-font Latin milestone closes.
> User direction overrides routine confirmation, per-task delegation and commits.

**Goal:** enable faithful original-font editing in the identified embedded-font
producer PDF, including paragraph growth and saved re-editing.
**Architecture:** separate complete-repertoire verification from bounded text
editing proof. Account for every declared code but grant encode/decode authority
only to unique, independently proved characters. Preserve source PDF objects.
**Tech Stack:** existing Rust/lopdf/ttf-parser/PDFium engine, desktop, CLI/MCP,
Python independent readers; no dependency addition.
**Spec:** docs/superpowers/specs/2026-10-28-embedded-cmap-repertoire-design.md

## Global constraints

Keep 2 MiB CMap, 16 MiB font and 65,536-code bounds. All assignments, including
unusable destinations, count toward structure, overlap, codespace and work limits.
Do not repair destinations or source bytes. Preserve originals, accepted packages,
failed attempts and inherited dirty work. One Cargo at a time, locked/offline,
jobs1, serial native tests; freeze product/test/build inputs during live gates.
Shared target: `C:/Users/WilliamHerr/Desktop/Code/Folio/src-tauri/target`.
Complete form-repertoire proof remains strict. No commits or publication.

## Review focus

1. An excluded assignment overlaps a later valid one: order must never determine
   authority. Task1 owns both-order tests and codespace/budget accounting.
2. Encoded UTF-16 range increments cross a surrogate boundary: no scalar-number
   reinterpretation or wraparound. Task1 owns boundaries and array syntax.
3. A malformed explicit CID mapping resembles omission: never adopt the default
   after a present value fails. Task2 owns absent/null/name/stream distinctions.
4. Native extraction omits an excluded source code: valid surrounding characters
   must not authorize an incomplete source edit. Task2 owns exact-source refusals.
5. A shared font is used by forms and ordinary text: text policy must not loosen
   all-scalar appearance proof or alter another page. Tasks2/3 own regressions.

## Task1: bounded CMap destination classification

Files: `src-tauri/src/embedded_font.rs` and its existing unit tests; public fixture
regressions in new `src-tauri/tests/embedded_cmap_repertoire.rs` when integration
requires them. Artifacts under `artifacts/embedded-cmap-repertoire-20261028`.

Private interface: introduce an explicit policy such as
`CMapPolicy::{Complete, LatinEditing}` and retain all assignments in a bounded
`BTreeMap<u16, Option<char>>`. A `None` entry has no text authority but still owns
its code for overlap, codespace and budget validation. Keep code counting separate
from the size of the eventual verified-character repertoire.

- [x] Preserve touched-source baseline, product/test manifests and exact original
  producer hashes. Record the current original-file failure before implementation.
- [x] Add failing parser regressions for valid Latin plus unused unusable string
  destinations. Complete mode must still refuse. Run and inspect the red result.
- [x] Add both-order overlaps, duplicate unusable definitions, out-of-codespace
  exclusions, malformed counts/types/arrays, inherited maps and 65,536 boundaries.
- [x] Implement classification after structural admission. Keep non-string targets
  and arrays in bfchar invalid. Support bounded byte-wise range increments; test
  surrogate pairs, invalid sequences, multi-scalar strings and overflow explicitly.
- [x] Run parser/library regressions with frozen inputs. Do not interpret an
  unavailable Unicode mapping as a glyph name or substitute a nearby code.

## Task2: source editing proof and omitted Identity mapping

Files: `src-tauri/src/embedded_font.rs` (`verify`, `verify_domain`,
`choice_font_codes`), new public native suite above; existing `embedded_text`,
`embedded_form_text`, `text_group`, paragraph/form/occurrence/automation tests as
needed. Read `source_text.rs` and `positioned_text.rs` before changing consumers;
prefer keeping public APIs and the existing verified-font representation intact.

Consume Task1's map according to the caller's policy. `VerifiedFont` stores only
usable characters after its existing glyph/advance/resource proof. Preserve valid
entry verification even for entries not used by the current selection. Distinguish
an absent `/CIDToGIDMap` from a malformed present object. Text proof may use the
specified Identity default for absence; complete form proof keeps its contract.

- [x] Add independent failing cases for unused unusable mappings and absent
  Identity, plus the untouched combined producer case. Record each actual failure.
- [x] Connect editing policy without relaxing `choice_font_codes`; add a negative
  complete-repertoire test for the same font accepted by text editing.
- [x] Implement missing-entry Identity only for the intended policy. Verify null,
  unknown names, odd/truncated streams, out-of-range glyphs and bad widths refuse.
- [x] Prove selected excluded codes and unsupported replacement characters refuse
  without source mutation, including native extraction that omits a source code.
- [x] Prove uniqueness and glyph agreement for valid codes: conflicting Unicode
  subtables, duplicate-character mappings, same program/different resources and
  mismatched PDF widths retain their existing rejection behavior.
- [x] Exercise original-font single/grouped text, dependent Tj/TJ/quote cursor
  behavior, uniform/styled paragraphs and reusable-form text through immutable
  APIs. Include save/reopen/re-edit, rotations/physical units and neighbor guards.
- [x] Run affected library/text/group/paragraph/form/context/occurrence/automation
  regression. Retain every terminal result and ignored reason. Expand only for
  actual changed consumers or unresolved preservation concerns.

## Task3: original producer and integrated acceptance

Files: milestone-local harnesses/reports, persistent plan/ledger/checkpoint and
the existing quality roadmap. Preserve exact input
`artifacts/producer-text-20261026/inputs-2/mupdf-True.pdf` and all predecessor builds.

- [x] Build a fresh isolated GUI/CLI candidate tied to completed source manifests.
- [x] On the untouched original, insert text, grow a paragraph, save/reopen and
  edit again through CLI and both MCP protocols. Exercise explicit substitution
  separately; do not import a replacement font for original-font acceptance.
- [x] Use the owned Windows/WebView2 workflow for Preview/Cancel/Apply, exact
  Undo/Redo, save/reopen and search. Stop only the owned process by exact identity.
- [x] Independently verify complete text and exact font/CMap bytes with pypdf/
  MuPDF, all nonselected glyphs, outside-area RGB pixels and other pages. Verify
  corrupt-output negative controls, including each RGB channel and font mismatch.
- [x] Reinspect the six unchanged producer inputs. Distinguish new support from
  remaining Word or unrelated producer refusals; do not infer general compatibility.
- [x] Freeze evidence and obtain one final scoped source review. Correct confirmed
  findings with regressions and any necessary fresh candidate acceptance.
- [x] Reconcile terminal gates, source/executable/harness/input hashes, document
  limits and the next gap; keep the overarching goal active until parity is proven.

## Readiness and discretion

Implementation and verification are complete; the accepted local source checkpoint
is `artifacts/embedded-cmap-repertoire-20261028/checkpoint-final.json`. Exact progress,
failed attempts, correction rationale and terminal gates are in
`.superpowers/sdd/2026-10-28-embedded-cmap-repertoire/progress.md`.
Diagnostic probe versions and source hashes are in
`artifacts/embedded-cmap-diagnosis-20261028/round-2`; they are not production parsers.
Private enum names and fixture organization may change to fit existing code.
Any additional refusal must be diagnosed independently before widening policy.

## Discovered writer compatibility correction

The stricter final-byte rule exposed Folio’s historical all-BMP range. New
resources now emit byte-bounded ranges excluding surrogate code units. Exact
historical font resources retain original-font editing through a complete
content-addressed resource comparison and the source-operator route; general
CMap parsing and complete form proof remain strict. Legacy annotation recovery
compares the entire regenerated appearance with only its owned FolioFont CMap
allowed to match the historical literal. Red/green, tamper controls and saved
re-edit evidence are in the execution ledger. Full native verification passed
1,613 tests/14 ignored; two explicit original Word controls and independent audits
of all ten captured Word outputs also passed. The single final review's F1 was
corrected and verified; no second review, release or parity claim.
