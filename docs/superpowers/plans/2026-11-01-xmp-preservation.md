# XMP preservation implementation plan

> Execute inline with superpowers:executing-plans in the existing isolated worktree. One final fresh scoped review. Standing autonomous authorization applies.

**Goal:** Preserve ordinary/custom document metadata during composed Save without retaining recognized conformance claims or weakening explicit redaction.
**Architecture:** Bounded XML preflight, namespace-aware source-range filtering, private PDF metadata attachment, shared Save integration.
**Tech stack:** Rust/lopdf, pinned roxmltree0.21.2 and existing quick-xml0.42.0; PDFium; Python independent readers; Windows desktop acceptance.
**Spec:** docs/superpowers/specs/2026-11-01-xmp-preservation-design.md

## Global constraints
Preserve original/private PDFs, old packages, failures and inherited dirty work. No commits, merges, physical printing or user-app restarts. Decoded XML4MiB, depth64, events131072, attributes32768 (256 per element), namespace declarations256, names1024bytes; tree parsing only after preflight. No DTD/external resolution. Source ownership is first output source. Explicit redaction remains stripping. Do not claim universal conformance, metadata editing UI or parity.

## Review focus
Namespace/entity aliases bypassing claim removal; custom multilingual/qualified values lost; aliased metadata consumers changed; unbounded parser/copy expansion or delayed cancellation; metadata resurrected after redaction or another user's document selected as owner.

### Task1: Bounded metadata filtering
Files: new src-tauri/src/document_metadata.rs and document_metadata_xml.rs, tests modules; lib.rs; Cargo.toml/lock.
- [x] Add core preservation/filter/alias/encoding/malformed/limit tests, run stub and observe actual behavior RED.
- [x] Implement preflight and namespace-aware selective filtering, bounded decoding and private stream/Info installation. Pin parser dependency and lock only required changes.
- [x] Run focused library suite serial/offline; preserve exact failures and hashes.

### Task2: Save integration and fidelity
Files: composition_catalog.rs/tests, engine.rs, complete_source_save.rs/tests; new tests/document_metadata.rs.
- [x] Add failing original structured and plain Save reproductions, including mixed metadata and overlays.
- [x] Integrate leading-source policy in structured and plain Save, keeping byte-copy no-op and visual routes distinct.
- [x] Test source order, aliases, cancellation, malformed destination preservation, repeated saves, UTF8/16/32 and explicit redaction. Correct obsolete whole-packet-drop expectations with real metadata assertions.
- [x] Run affected suites and independently inspect XMP/Info, graphics and source bytes; build fresh candidate and exercise real desktop Save/reopen. Fresh independent audit proves page pixels/text and ordinary metadata; full tagged-tree equivalence is not newly established.

### Task3: Final review/checkpoint
- [x] Freeze scoped source/evidence and request one fresh review. Review found no demonstrated actionable defects; coverage limits remain explicit.
- [x] Verify terminal evidence and fingerprints, update roadmap/ledger, retain overall goal active and existing packages unchanged. Checkpoint1fa72827 binds169hashes,211distinct native passes,8desktop steps and both independent audits.
