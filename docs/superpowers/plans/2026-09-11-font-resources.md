# Font Resources Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development for independent tasks and review checkpoints.

**Goal:** Complete issue #11 with installed/imported fonts that survive preview, export and recovery.

**Architecture:** Native content-addressed font registry; custom-font references in text overlays; embedded CID font appearances; exact-byte browser FontFace; recovery sidecars.

**Tech stack:** Rust, ttf-parser, PDFium/lopdf, Tauri, React/TypeScript, Vitest/Playwright.

**Spec:** ../specs/2026-09-11-font-resources-design.md

- [x] Registry/catalog: implement fonts.rs, parser/permissions/coverage/metrics and bounded immutable assets; tests for valid/invalid/restricted fonts and deduplication. Add pinned ttf-parser dependency with notices.
- [x] PDF persistence: optional TextOverlay fontId, shared registry access through PdfEngine, custom CID resources and appearance encoding for editable/flattened output, verified import and rollback. Add native roundtrip tests.
- [x] Commands/recovery: list/select/import/info/bytes/release APIs, recover font sidecars with strict checks and generation cleanup, retain old recovery compatibility. Test original-font removal and failed restoration.
- [x] Frontend: exact-byte FontFace loading and lifetime, searchable picker/import/error states, glyph validation, history ownership and export integration; targeted and browser tests.
- [x] Review, full regressions, independent reader checks and packaged desktop workflow. Document support scope, package/release, and update #11.
