# Positioned Form Text Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for inline implementation. Steps use checkbox syntax.

**Goal:** Edit supported nested-form TJ arrays without losing specified character positioning or changing shared copies.

**Architecture:** Share the existing logical-character-slot rewrite between top-level and form editing. Form occurrences retain the source text-show Operation; mapping and chain cloning recognize glyph-bearing Tj/TJ and skip preserved numeric-only cursor arrays. Keep native publication proofs unchanged.

**Tech Stack:** Rust/lopdf/PDFium, generated fixtures, MuPDF/pypdf, Windows WebView2 acceptance.

**Spec:** `docs/superpowers/specs/2026-10-24-form-positioned-text-design.md`.

## Global constraints

- Existing isolated dirty worktree; no commits, scratch deletion, original-document overwrite or external release. User waived routine approval questions.
- Form arrays <=16,384 elements and combined encoded bytes <=8192; adjustment values finite and abs<=1,000,000. Preserve existing top-level bounds.
- One glyph-bearing Tj/TJ per BT/ET with explicit local font/position. Numeric-only TJ inside BT/ET contributes no object path. Empty-string-only arrays are refused in forms.
- No new dependencies or UI; retain current source-state/font/protection/clipping/collision/no-op/outside-raster rules. Same-text arrays retain trailing gaps; changed shorter strings drop gaps at/beyond their end; longer strings append after original slots.
- Baseline executable/fingerprint: `artifacts/form-text-state-20261024/desktop-build/Folio.exe`, `artifacts/form-text-state-20261024/build-source-fingerprint.json`.

## Review focus

- Source two-byte segments split across strings must be refused at listing, not only apply (Task2).
- Source/target byte widths can differ during font substitution without moving logical gap boundaries (Task2).
- Numeric-only arrays before/after a show must not shift sibling object identities (Task2).
- Leading/trailing/consecutive adjustments, empty segments and shorter/equal/longer strings must preserve existing top-level semantics (Task1/2).
- Scale/rise/affine transforms must not bypass clipping/collision or cause unrelated pixels to change (Task3).

## Task 1: Shared encoded-array rewrite

Files: new `src-tauri/src/text_show.rs`, `text_edit.rs` module declaration,
`positioned_text.rs` refactor, new `src-tauri/tests/form_positioned_text.rs` RED fixture.

Interfaces (all internal to text editing):
- `text_show::boundaries(text, encode)` builds checked cumulative byte boundaries by encoding complete logical characters; accepts a closure `FnMut(&str)->EngineResult<Vec<u8>>`.
- `text_show::validate_segments(values:&[Object], expected:&[u8], boundaries:&[usize])->EngineResult<()>` verifies exact source segments, finite bounded numbers and complete character boundaries.
- `text_show::rewrite_array(values:&[Object], expected:&[u8], changed:&[u8], source_boundaries:&[usize], replacement_boundaries:&[usize], same_text:bool)->EngineResult<Vec<Object>>` validates maps before slicing; preserves original numeric objects/order and the established slot rule.
- `text_show::encoded_bytes(operation:&Operation)->EngineResult<Vec<u8>>` validates bounded form Tj/TJ syntax and concatenates source bytes; numeric-only arrays return empty. Used only by the form route; no new top-level admission cap.

- [x] Write native nested/shared fixture replacing a leaf Tj with a TJ array. Assert two supported occurrences and unchanged/shorter/longer original-font edit pixels against separately authored references. Run it; expect existing unsupported-TJ refusal.
- [x] Add unit examples for character slots, source/target byte-width change, leading/trailing/consecutive gaps, exact no-op, malformed boundary maps, split codes and invalid scalar/operand types. Implement the shared helper.
- [x] Replace top-level inline TJ rewrite with helper calls, retaining its caller policy, original source matching and Unicode-unchanged decision.
- [x] Run helper unit tests and existing top-level `text_edit`/`embedded_text` positioned regressions. Native form fixture remains expected RED until Task2; record explicitly.

## Task 2: Native form mapping and encoding-aware publication

Files: `src-tauri/src/form_text.rs`, `form_text_state.rs` unit caller adaptation,
`src-tauri/tests/form_positioned_text.rs`, fixture helper if needed.

Interfaces:
- Occurrence adds its original `Operation` while retaining encoded bytes, path, ancestor boxes and effective state.
- A common form predicate classifies glyph-bearing Tj/TJ, excluding the existing `positioned_text::cursor_only` classification after operand validation.
- Model replacement and recursive clone-chain consume a complete replacement `Operation`; no-op cloning consumes the original occurrence Operation.
- Original boundary map uses `positioned_text::encoded(selected, character)`. Installed Identity-H target uses UTF-16BE per supported character; verified same-program reuse retains source encoder. Build source/target maps before cloning, use shared rewrite for TJ and ordinary encoded string for Tj.

- [x] Extend flatten admission to bounded glyph-bearing TJ and preserved numeric-only cursor arrays inside BT/ET. Keep one shown object/local font/position; reject empty-string-only shows.
- [x] Update native-tree validation and clone path indexing consistently. Validate source segment boundaries during listing and publication. Pass complete replacement show through font/resource-chain isolation.
- [x] Run Task1 RED fixture to GREEN. Add standard/simple/CID source cases, new-font code-width transitions and same-program reuse; preserve nonzero-Tw restriction.
- [x] Add numeric-only prefix/suffix and inherited-state fixtures, actual unchanged array preservation, positive/negative/zero/leading/trailing/consecutive gaps, malformed/wrong-count/nested/huge arrays, split-CID refusal, stale identity, unchanged other occurrence/source.
- [x] Run new suite and existing `form_text`, `embedded_form_text`, `form_text_state`, `form_font` regressions. Fix demonstrated failures without weakening source proofs.

## Task 3: Independent output and runnable acceptance

- [x] Capture nonredundant original/independently authored reference/edited PDFs across affine transforms, rotations, CropBox/UserUnit, same-font/substitution and state scopes. Add collision/ancestor clipping, protected editing and recovery checks.
- [x] Verify saved output through MuPDF/pypdf, including unchanged other pages, extraction, numeric adjustments/logical boundaries, outside-region pixels and repeated re-edit. Use fixtures/reference code independent of production helper.
- [x] Run relevant current-source native suites and TypeScript/build checks; no broad frontend rerun required without frontend changes. Request one fresh scoped implementation review and handle demonstrated findings with RED/GREEN evidence.
- [x] Build fresh fingerprinted executable; use repository owned Windows/native-dialog workflow to select actual target, edit, undo/redo, save/reopen, restore, and substitute fonts where supported. Independently inspect outputs and close owned processes.
- [x] Record accepted evidence, source/executable fingerprints, preserved inputs, diagnostics and remaining gaps in existing-text docs, quality loop, verification JSON and ledger. Keep overarching goal active.

Self-review: exact state, byte/character distinction, native path mapping, font installation, shared-copy preservation and actual saved output map to the three tasks. Builder check reports nothing important left.
