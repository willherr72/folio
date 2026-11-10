# Content Number Correspondence Implementation Plan

> Execute inline using superpowers:executing-plans under the existing phase3
> review boundary. Preserve the dirty tree, evidence and accepted package.

**Goal:** Prevent private redaction from changing the meaning of content numbers.

**Architecture:** A bounded numeric proof helper checks original and emitted
tokens. The existing preflight applies it before decoding and after final
content serialization. No public JPEG admission changes.

**Tech stack:** Rust/lopdf, independent Python rational/native-reader audits.

- [x] Add red tests and independent numeric proof module, register in lib and
  the standalone harness; integrate raw-token preflight and final publication.
- [x] Export source/canonical token decisions and verify actual PDFium and
  MuPDF interpretation, including refusal controls and power-table proof.
- [x] Run serial native redaction regression, compare existing saved artifacts,
  and checkpoint coverage, conservative refusals and remaining placement work.
