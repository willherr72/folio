# Document properties implementation plan

> Execute inline with superpowers:executing-plans in the existing isolated worktree; one final fresh scoped review. Standing authorization applies; no commits, cleanup or routine permission checkpoints.

**Goal:** inspect/edit document properties and explicitly remove document Info/XMP, with Undo/recovery and faithful PDF publication.
**Architecture:** bounded native inspection/XML mutation; document-level draft; final pre-encryption metadata application across PDF exports.
**Tech stack:** existing Rust/lopdf/quick-xml/roxmltree/Tauri/React/Vitest/Windows acceptance.
**Spec:** docs/superpowers/specs/2026-11-01-document-properties-design.md

## Global constraints
Preserve existing dirty work, sources and packages.16KiB UTF8 per editable field; existing4MiB XML/depth64 bounds. First output source supplies unedited metadata; explicit edits belong to the document. Remove only document Info/catalog XMP, not all hidden information. No product changes during active source-fingerprinted builds/tests.

## Review focus
- Reordering/deleting the leading source must not discard explicit edits: model/export tests.
- Qualified or multilingual RDF must retain unrelated values: native XML tests.
- Malformed metadata removal must bypass preservation parsing but retain page/object aliases: native composition tests.
- Protected Save and split/extraction must retain draft edits and enforce permissions: backend/frontend export tests.
- Redaction/recovery and stale dialog responses must not resurrect metadata or mutate another tab: frontend/native ownership tests.

### Task1: Native property model and XML/PDF operations
Create src-tauri/src/document_properties.rs, document_properties_xml.rs and respective tests; expose bounded XML decoding helper in document_metadata_xml.rs. Public types DocumentProperties and DocumentMetadataEdit(Update optional title/author/subject/keywords, Remove). Interfaces inspect(Document)->DocumentProperties and apply(Document,edit,check)->EngineResult<()>; no mutation on unsupported structures before staging.
- [x] Add failing native core tests for inspection, common RDF forms, Unicode/empty defaults, preserved custom values and malformed removal.
- [x] Implement bounded source-range updates, private Info/XMP roots and unreachable-object pruning; run exact-source harness and native unit gates.

### Task2: Backend export and recovery
Modify engine.rs with inspect worker and optional metadata on controlled export, retaining existing public wrappers; add document_properties_engine.rs. Apply edits before encryption/atomic output. Add optional metadata to SplitPdfRequest and split composition; validate recovery document metadata. Register inspect command and optional Save/export args in app.rs/pdf_save_commands.rs.
- [x] Add failing public-engine tests: complete/composed/tagged Save, independent graph/pixels, permissions, protected copy, malformed removal and split.
- [x] Implement routes and run affected native/recovery/split/security tests, retaining all failures and fingerprints.

### Task3: Editor draft and controls
Create src/editor/document-properties.ts and src/components/DocumentPropertiesDialog.tsx. Extend EditorDocument, planDigest, recovery, adapter and PdfSaveOptions. Add Document properties command, owned dialog draft, Undo/Redo commit and explicit removal; carry metadata through App save/extract/split/security options and ProtectedCopyDialog/SplitPdfDialog. Preserve redaction's new-document reset.
- [x] Add model/recovery/dialog/export propagation tests and observe failures.
- [x] Implement UI with field-conflict/multi-author disclosure and keyboard/focus handling; run focused/full frontend and build checks.

### Task4: Acceptance and checkpoint
- [x] Build a fresh candidate; desktop edit/Undo/Redo/Save/reopen/removal/recovery workflow with owned synthetic files and original preservation.
- [x] Independently inspect Info/XMP, unchanged page pixels/programs and metadata-removal bytes; negative controls.
- [x] One final scoped review; correct consequential findings once, verify evidence and update roadmap. Leave overall goal active.

Implementation clarification: Update supports clearExisting:true when adding values after removal; retain that flag through later edits to prevent resurrection. Reset to source explicitly clears the override. XMP edits target the empty-about RDF subject when present, otherwise the first subject, retaining other subjects.

Final review: I1/I2 corrected in one RED→GREEN pass; M1 (explicit Info/XMP presence labels) deferred. Corrected native84passed, unchanged frontend1427passed, fresh build and11Windows steps plus independent output acceptance passed. Scope/limitations are recorded in the ledger and checkpoint.
