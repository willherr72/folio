# Source-bound ligature implementation plan

> Execute inline in the existing managed worktree. User opted out of routine questions. No commits, cleanup or package selection. One final fresh review when the integrated feature has completed runtime evidence.

**Spec:** docs/superpowers/specs/2026-11-13-ligature-source-editing-design.md
**Goal:** Existing source ligatures remain editable with exact unchanged glyphs, honest spacing and paragraph reflow.
**Architecture:** One immutable verified-symbol codec; source-bound atom alignment; shared metrics/count/boundaries across operator replacement and paragraph publication. Existing proof and publication checks remain authoritative.

## Global constraints

Preserve parent5356 source/checkpoint, selected050 and reviewed051, private files and all failed evidence. Existing native target/temp and single-job constraints. No broad native/performance/keyboard/release claims from a narrow correctness test. Standalone rustc harness compiles the actual module and is a fast TDD gate, never a substitute for engine/native/GUI integration acceptance.

### Task 1: Bounded encoded glyph model

**Files:** src-tauri/src/source_glyphs.rs, source_glyphs_tests.rs; artifact rustc harness/driver; text_edit.rs test-only module registration until integration.
**Interfaces:** Codebook accepts verified Symbol(code,text,gid,width), produces immutable BoundText/EncodedText with text, bytes, glyph count, cumulative glyph byte/Unicode boundaries and advance. Source proof belongs to later task; model alone authorizes no PDF.
- [x] Record baseline fingerprints and reproduction provenance.
- [x] RED actual module tests for fi/ffi source identity, growth and separated edits, changed ligature decomposition/refusal, immutable failure, malformed/ambiguous symbols and resource limits.
- [x] GREEN bounded decoder/encoder/alignment; exhaustive small reference oracle; scalar compatibility, supplementary Unicode, glyph vs character spacing evidence.
- [x] Record exact source/test hashes and limits. Mark integrated behavior still pending.

### Task 2: Verified mappings and source-bound text editing

**Interfaces:** Extend shared structural CMap parser without weakening complete/scalar callers; physical glyph proof for sequences; attach exact source encoding to RunSnapshot; encoded bytes, width, count and TJ boundaries flow from one result.
- [x] RED native public source fixtures and malformed mapping/width/ambiguous/refusal controls.
- [x] Implement CID sequence proof/binding and source-aware original/substitute encoding.
- [x] Complete dependent cursor fidelity for the supported root/shared-form original/substitute routes: current production, saved re-edit and fixed-mask independent checks pass; see cursor2-completion.json.
- [x] Run native unchanged bytes/pixels, insertion/deletion/inside-ligature edits, Tc/Tw/TJ, following text, save/reopen/re-edit and partial-repertoire refusal tests. This native result does not waive I1.

### Task 3: Paragraph occurrence metrics and replay

**Interfaces:** Shared atom model consumed by metric measurement, layout/wrap and native publication; style IDs remain Unicode editor-facing. No atom split on span or line boundary.
- [x] RED mixed occurrence/fi/ffi/repeated letters, paragraph no-op/growth/wrap/substitute/re-edit.
- [x] Integrate native glyph metrics, baseline/highlight/collision and source-order proofs.
- [x] Run complete library correctness and affected ordinary optimized integrations with fingerprints. Current correction:1643library/35ligature/41paragraph correctness passes;110ordinary optimized passes. Earlier1663/236frontend evidence is historical with unchanged frontend inputs, not a fresh correction run.

### Task 4: Compiled acceptance and final review

- [x] Current normal optimized GUI/CLI build, public original-font/explicit substitute/paragraph/neighbor workflows, save/reopen, independent raster/text/font audit and negative controls.
- [x] Freeze exact evidence; one fresh final reviewer; one correction pass if Important, explicit dispositions for coverage limits and declined judgments.
- [x] Verify component checkpoint and preserve selected builds; broad goal remains active.

## Final-review correction checkpoint (2026-11-14)

Current source17acb3d41c00fab8f66f88cdda629cecd29232072f53f64c0aee3970a001caed. Exact evidence: artifacts/ligature-source-editing-20261113/review-correction-qualification.json. I2/I3 are resolved, including scalar-only legacy paragraph policy, saved re-edit, adjacent/zero-effect TJ boundaries and genuine-gap refusal. All26new transition/paragraph/highlight independent pairs and8controls pass. The earlier root/form regression still has17passing/3failing pairs and5rejected controls; these failures are I1 and remain unwaived.

The sole whole-feature review is complete. All10declined judgments and cost-if-wrong rulings are recorded in the ledger; no minor findings were raised. Current compiled GUI/CLI acceptance is pending after source correction. The old f536 compiled evidence is preserved with its original meaning. No component/portable/release/parity approval; selected050 and separate051 are preserved. Next bounded I1 experiment is pinned in artifacts/ligature-source-editing-20261113/review-i1-next-experiment.json. Do not restart the resolved old CLI receipt failure, repeat the sole final review, or promote the diagnostic width probe.

## I1 correction implementation steps

User continues to authorize inline autonomous work. The selected package stays unchanged. One final reviewer is already complete; no further reviewers. Keep exact source atoms, static proof limits, original font programs/GIDs/ToUnicode, and existing native/independent outside masks. No invisible source text/glyph cursor trick and no renderer/proof weakening.

The current-engine diagnostic cursor-resource-name-native-2 proves that distinct legal PDF BaseFont/FontDescriptor names isolate the selected cloned width resource and retain exact no-op editing. The identical-name negative remains refused; therefore no global actual-resource ambiguity refactor is needed. This supersedes the proposed broader resource-binding experiment as the next implementation step.

- [x] Preserve the17acbaseline and reproduce unique-name proof/no-op versus ambiguous-name refusal with a current native engine test. Original stale expected-text test failure is preserved separately.
- [x] TDD dependency-free cursor_widths.rs: bounded widths/counts, exact aggregate fractional-minus-nearest-integer residue, unchanged retained widths, only caller-authorized new glyph codes, nominal1.01width limit, finite0..10000limits. Prefer exact one-code solution, then bounded multi-code allocation/representability repair; fail atomically when no proved correction is found. Standalone tests include repeated-code weighting, an exact feasible two-code repair, invalid/inconsistent inputs and unchanged inputs; this module supplies no font authority.
- [x] Integrate cursor_font.rs with the actual source Tf resource and verified program/GID metrics. Clone only the selected Type0/CIDType2 font dictionary/descriptor/widths, retain the program/mapping, use a unique internal PDF font name, select and restore around exactly one show. Compute dependent advance in1000em units from the same resolved glyphs; the typed integral/fractional policy must match the actual installed writer; forms retain their fractional-width policy.
- [x] Selected-style validation may accept only the exact planned unique resource name with the unchanged original program/layout (or explicitly authorized substitute program); other runs retain exact name/program/style/geometry checks. Reopened resources remain subject to existing full proof and no-op validation. No prefix/name alone grants authority.
- [x] Run current native root/shared-form original/substitute save/reopen/re-edit and immutable refusal tests, including legacy scalar/CFF/WinAnsi/paragraph/context controls. Re-run independent fixed-mask root/form audits and all current transition/paragraph/highlight pairs, with negative controls. Do not closeI1 unless the real production outputs and reopened edits pass.
- [x] Rebuild current combined diagnostic CLI/GUI only after source acceptance. Re-run compiled and owned desktop workflows with current fingerprints; preserve old source/runtime evidence and all declined review rulings. Broad Adobe/Foxit goal remains active.

Ruling: exact representability and a newly introduced eligible glyph are conditions for this correction strategy, not claims of a universal PDF solution. If bounded planning cannot prove a correction, it must refuse rather than alter retained glyph widths or publish unselected drift. Costs and broader editing coverage remain part of the active project, with paragraph reflow a separately validated route. Unique PDF resource labels remain internal; visible font-family labels come from the unchanged embedded program.

I1 current correction checkpoint: production root/shared-form original/substitute edits and saved re-edits pass native checks and independent reader pixel/resource checks (52pairs/20controls); full1 native/optimized/freshfrontend pass at their recorded source. Candidate pipeline1 was deliberately stopped for a confirmed inline-font name-reservation gap before runtime acceptance, not an observation timeout. Corrected name-search tests pass; current source verification restarts under cursor-full-native-2. Partial build and all prior receipts remain historical. See the ledger and cancellation.json; no component/release/parity completion.

## Verified correction checkpoint (2026-11-14, current source)

Source e1b1129cf669b7af7aa66251613d6f439ccaa7b96d1d1e1d3deb4f5c8e48810b. Authoritative receipt: artifacts/ligature-source-editing-20261113/cursor2-completion.json. All three Important review findings are addressed within the stated source-proof policy: I2/I3 retain the earlier failing/passing regressions, and I1 now passes production root/shared-form original/substitute edits, saved re-edits and exact no-ops in native and independent checks.1655 library/11ignored,38ligature/0 and41paragraph/1 passed at Folioopt0 correctness;12ordinary optimized targets152/2;52independent pairs and20rejected controls;1663frontend tests/236files; fresh normal combined build;44actual CLI commands/12saved pairs; owned Windows desktop3saved pairs/2rejected controls and verified shutdown. All source/native input guards passed. Current receipt pins exact logs, binaries and26renderer resources. No further reviewer, selected launcher change, package promotion or broad parity certification. Earlier dated checkpoints and stopped attempts remain historical evidence.
