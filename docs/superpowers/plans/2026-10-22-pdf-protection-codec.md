# PDF protection codec implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Preserve supported existing PDF credentials and permissions across an
in-memory plaintext editing representation, as a prerequisite for protected workflows.

**Architecture:** A focused protection module returns plaintext plus opaque context
and non-secret capabilities. Re-encryption applies that context to a transformed PDF
without storing passwords or writing files. Engine/UI integration follows separately.

**Tech Stack:** Rust, pinned lopdf0.45, existing PDFium; Python pypdf/MuPDF verification.

**Spec:** ../specs/2026-10-22-pdf-protection-design.md

## Global constraints

- No custom crypto, dependency upgrades, exposed credentials or disk writes in codec.
- Bounds:512MiB input/output,100,000 objects,64MiB stream decode,1,024-byte password.
- Keep the broader goal active and retain uncommitted work; no publishing/committing.

## Review focus

- Empty user passwords must not accidentally give owner capabilities.
- A changed trailer ID/object numbering must not invalidate the original passwords.
- Unsupported crypt filters must fail rather than silently leave content unprotected.
- Missing or invalid encryption metadata must not look like an ordinary unprotected PDF.
- Key/password bytes must not appear in error or Debug strings.

## Task: native codec and independent fixture gate

Files: new src-tauri/src/protection.rs; module export in lib.rs; new
src-tauri/tests/pdf_protection.rs; synthetic fixtures/generator under
tests/fixtures/protection; artifacts/protection-codec-progress.md evidence ledger.

Interface: `unlock_pdf(bytes:&[u8],password:Option<&str>)->Result<UnlockedPdf,ProtectionError>`;
`UnlockedPdf { bytes:Vec<u8>, protection:Option<PdfProtection> }`;
`PdfProtection::permissions()->DocumentPermissions`, `is_owner()->bool`,
`revision()->i64`, `protect(bytes:&[u8])->Result<Vec<u8>,ProtectionError>`.
ProtectionError has PasswordRequired, IncorrectPassword, Unsupported, Invalid, Limit
variants; messages never include passwords or raw encryption state.

- [x] Generate reproducible synthetic fixtures with provenance and no personal PDFs.
- [x] Add failing tests for passwords/rights and re-encryption after transforming PDF.
- [x] Implement codec, explicit handler/filter validation, bounded serialization and
      redacted Debug; verify focused tests reach GREEN.
- [x] Add negative and edge regressions tied to review focus; preserve plain input bytes.
- [x] Run one fresh review; fix material findings once without starting another round.
- [x] Verify outputs with independent readers/raster comparison, relevant native tests
      and production build; record completed prerequisite and remaining integration.
