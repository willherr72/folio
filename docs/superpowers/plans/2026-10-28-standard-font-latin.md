# Standard-font Latin Editing Implementation Plan

> Use superpowers:executing-plans inline, with TDD and one final scoped reviewer.
> Preserve artifacts and inherited work; no commits or cleanup.

**Goal:** edit ordinary accented standard-font PDF text faithfully with the source
font, including paragraph reflow and saved re-editing.
**Architecture:** bind checked character mappings and nominal advances to each
source text operation's actual font resource; share them across native consumers.
**Tech Stack:** existing Rust/lopdf/PDFium engine, Windows desktop, CLI/MCP, Python
independent readers. No new dependency.
**Spec:** docs/superpowers/specs/2026-10-28-standard-font-latin-design.md

## Global constraints

Twelve Latin standard faces; named Standard/WinAnsi/MacRoman and existing equivalent
dictionaries. 4,096 UTF-8-byte strings, 16 MiB content, 100,000 operations, 64-level
traversal/state limits. No nominal proof for overridden maps/widths or glyph-changing
descriptors. Original inputs and accepted 0.23 package immutable. One Cargo,
locked/offline, jobs1, serial tests; shared target
`C:/Users/WilliamHerr/Desktop/Code/Folio/src-tauri/target`.

## Review focus

1. Same-name fonts with different encodings: bind by source operation/resource;
   collision tests belong to tasks1/2.
2. Duplicate code mappings and space behavior: exact source roundtrip and checked
   canonical replacement codes; alias/undefined-code tests in task1.
3. Correct letters but wrong following-text positions: TJ/quote boundaries and
   advance/cursor checks in task2, independent pixel audits in task3.
4. Paragraphs mixing incompatible resource proofs: style/fragment replay cases
   and saved re-edit tests in task2.
5. Hidden resource overrides or allocation amplification: bounded admission tests
   in task1; source/font preservation and unchanged original checks in task3.

## Task1: source-bound standard font proof

Files: new src-tauri/src/standard_font.rs, src-tauri/tests/standard_font_latin.rs;
text_edit.rs, source_text.rs; bounded use of existing encoding/metric helpers.
Interfaces: `StandardFont::encode(&self, &str) -> EngineResult<Vec<u8>>`,
`decode(&self, &[u8]) -> EngineResult<String>`,
`advance(&self, &str) -> EngineResult<f32>`; private `RunSnapshot.standard` holds
an optional shared proof. Source binding consumes `&Document`, page ObjectId and
mutable snapshots; it never changes native text/geometry and gives no authority
when operation/resource/native correspondence fails.

- [x] Preserve touched-source baseline and full product/test fingerprints.
- [x] Add public-API WinAnsi/MacRoman insertion and paragraph regressions. Observe
  existing ASCII admission failures, with no source changes during the red gate.
- [x] Implement bounded proof and resource binding with focused tests for all
  supported encoding classes, aliases, unsupported codes, same-name resources,
  indirect equivalent dictionaries, widths/ToUnicode/descriptors and size limits.
- [x] Verify the new proof tests and existing equivalent-encoding tests; record
  terminal results and source hashes. No unfinished gate counts as green.

## Task2: editing, paragraph and form validation consumers

Files: positioned_text.rs, text_edit.rs, text_substitution.rs, text_paragraph.rs,
paragraph layout/publication/replay helpers as required, form_text.rs; existing
native test targets plus standard_font_latin.rs.
Consumes: task1 proof and source binding. Produces the existing public engine APIs
with additional proved standard-font repertoire; no public request format changes.

- [x] Route encode/decode/advance and replacement admission through the proof;
  keep ASCII fallback and embedded font behavior unchanged outside this route.
- [x] Prove no-op preservation, non-ASCII Tj/TJ/quote segments and dependent
  following text. Add failing regressions before each required correction.
- [x] Integrate original-font uniform/styled paragraph layout and saved replay;
  include proof identity in style comparison and source-derived font measurements.
- [x] Bind flattened form validation resources and check original-font versus
  explicit-font substitutions without widening unproved resource admission.
- [x] Run all twelve faces, relevant encoding/rotation/unit/alias matrices and
  affected library, text, group, paragraph, form, context, occurrence and automation
  regression suites. Retain all failures and ignored reasons with immutable inputs.

## Task3: original producers, desktop and automation acceptance

Files: artifacts/standard-font-latin-20261028 public harnesses/reports; existing
producer originals used read-only; docs/product-quality-loop.md.

- [x] Build fresh candidate desktop/CLI with a matching checked source fingerprint.
- [x] Original ReportLab/PyMuPDF standard-font files: insert text, save/reopen,
  edit again, and reflow a paragraph using the source font. Exercise explicit font
  substitution and both MCP transports. Preserve and rehash original inputs.
- [x] Run owned Windows Preview/Cancel/Apply, Undo/Redo, save/reopen and search;
  inspect actual screenshots and independently audit saved PDF text, font/resource
  identity, following text, outside-area pixels and unchanged second pages.
- [x] Verify oracle negative controls and recheck the six-producer inspection so
  unchanged embedded-font limitations remain honestly separated.
- [x] Freeze scoped source/test/harness evidence; request one final reviewer,
  resolve demonstrated findings with appropriate red/green tests, and record
  checkpoint/roadmap. Keep the overarching Adobe/Foxit goal active.

## Self-review

Source/encoding identity, aliases, metrics, preserved publication, bounded work,
all target consumers and independent final output evidence map to the tasks above.
No engine change is justified solely by a familiar font name. User authorization
permits inline execution without another design confirmation.
