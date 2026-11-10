# Faithful paragraph re-edit implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Reopening a Folio paragraph edit preserves explicit newlines, edge spaces,
the chosen edit area, alignment and line spacing, including after save/reopen.

**Architecture:** Store bounded, versioned paragraph hints on the PDF page. Bind each
record to the actual rendered text operators, font/style and origins. Reconstruct
text from the current PDF strings plus newline counts; never store a second copy
of the paragraph's words in metadata. Validate hints before use and retain existing
geometry-based inference when absent or stale. The user authorized autonomous
implementation choices without approval gates.

**Tech stack:** Existing Rust/lopdf/PDFium pipeline, native integration tests, existing
Windows desktop acceptance and MuPDF/pypdf verification. No new dependencies.

**Spec:** The design and acceptance criteria below are the scoped specification.

## Evidence and design

`artifacts/paragraph-reedit-red.log` reproduces the defect on unchanged production
code: a saved paragraph containing `  Alpha beta\nGamma  delta  ` is inspected as
`Alpha beta Gamma  delta`. The failing test additionally requires the original edit
area and explicit Justify setting. `prepare` joins rows with a space and trims ends;
the baseline then estimates area and alignment from ink. Visual rows cannot reveal
which breaks were intentional or how large the original text box was.

`artifacts/paragraph-metadata-transport.log` confirms both page-dictionary and
resource-dictionary private values survive the current real copy/preview/export
paths. Choose a page-dictionary record to avoid mixing editing data into resources.
This probe establishes transport only, not stale-record safety or full acceptance.

Alternatives considered: changing all row joins to newlines would freeze soft wraps
and lose normal reflow; geometry alone cannot recover author intent; workspace-only
data would fail after closing/reopening a saved PDF. A bounded PDF-local hint is the
chosen implementation approach, following the existing overlay persistence model.

### Record and trust boundary

- New `paragraph_memory.rs` under `text_paragraph` owns serialization, bounds and
  validation. Versioned JSON in a private page dictionary string, maximum 512 KiB,
  at most 64 records/page and 64 rendered rows/record. Unknown/malformed/oversized
  hints are ignored; they must not make an otherwise editable PDF fail to open.
- Records contain raw-PDF edit bounds, size/alignment/spacing, per-row newline
  counts and trailing newline count. Soft wraps concatenate the actual decoded
  source strings unchanged, preserving all encoded spaces. Empty hard-break rows
  are represented by newline counts. No duplicate paragraph words are embedded.
- Bind rows to a SHA256 of their actual text-show operands plus verified font/program,
  size, color and raw matrix. Do not depend on PDF object IDs or PDFium object indices,
  which can change during copy/export or another edit. Require an unambiguous match.
  Ruling: also retain bounded glyph coordinates (no character strings) and compare
  them on restore. A dedicated RED test showed that changing `Tc` leaves text-show
  operands and row origins intact; glyph geometry invalidates that stale hint.
- Validate finite/in-page bounds, current font/style and row membership. Reconstruct
  bounded text from actual PDF strings, then re-wrap/reposition using the recorded
  settings and the same font metrics. Require agreement with actual row text,
  origins and word spacing before using a hint. A hash is an integrity binding,
  not authentication; fields from arbitrary PDFs remain untrusted.
- Store raw geometry so intrinsic/workspace page rotations transform the restored
  area correctly. Crop/frame changes that invalidate the area fall back to inference.
- Only a complete matching paragraph uses its hint; partial manual selections retain
  existing behavior. Automatic suggestion can use a validated record containing its
  anchor, including blank lines or wider spacing that geometry grouping would split.
- Existing source admission, mixed-format/tagged-content restrictions, overlap,
  preservation and output verification remain. Gap admission moves behind validated
  hint recovery when necessary; invalid hints must not relax ordinary selection rules.
- On paragraph replacement, discard its old record and prune stale records, retain
  valid unrelated paragraphs, and add the verified replacement. Other edit paths
  may leave stale hashes but no hidden old paragraph words; those hints are ignored.
  Capacity exhaustion may discard oldest hints, never visible PDF content.

## Tasks

- [x] Reproduce saved hard-break/edge-space loss with a failing native integration
  test in `src-tauri/tests/paragraph_justify.rs`; retain the RED log.
- [x] Probe actual PDFium page-copy/preview/export transport before choosing storage.
- [x] Add focused persistence/validation tests for valid records, malformed/versioned
  data, bounded counts, modified text/operators/formatting and ambiguous matches.
- [x] Implement the record helper and integrate writing only after verified candidate
  output. Integrate restoration before inferred text/area/formatting takes precedence.
- [x] Integrate validated anchor-based paragraph suggestion. Test partial selection,
  blank lines, extreme supported line spacing and unrelated paragraphs on one page.
- [x] Pass roundtrips for leading/trailing/repeated spaces, soft versus hard wraps,
  multiple blank lines, all rotations, substitute Unicode fonts, editing a paragraph
  again, editing another paragraph, page reorder/duplicate and protected export.
- [x] Remove the temporary ignored transport probe after its evidence is recorded.
- [x] Run relevant native suites and one scoped review; fix reproduced findings.
- [x] Build a fresh debug executable and exercise actual Windows edit/save/reopen/
  re-edit/undo, preserving native comments and neighbors. Independently compare text,
  fonts, geometry and pixels before/after a no-op re-edit. Inspect screenshots.
- [x] Record exact source/binary/test evidence and update product-quality-loop and
  existing-text documentation. Preserve earlier packages and the original quiz.

## Review focus

Untrusted hint bounds and parser allocation; stale metadata after other tools edit
the PDF; exact whole-paragraph versus partial matching; no hidden deleted text copy;
rotation/crop handling; complete blank-line reconstruction; foreign PDFs without
hints; output fidelity on no-op re-edit. Metadata survival alone proves none of these.
