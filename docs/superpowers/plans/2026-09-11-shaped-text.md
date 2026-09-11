# Shaped Text Implementation Plan

> **For agentic workers:** Use subagent-driven-development or executing-plans with independent review. Native layout, font fixtures/interop and serialization are separate owned units; coordinate shared contracts before integration.

**Goal:** Complete #14's required native/API serialization gate and integrate only a representation that passes its reader and geometry checks.
**Architecture:** Native logical text → bidi/script runs → HarfRust glyph layout → shared preview geometry and allocated-CID PDF output. Legacy unshaped content retains its existing path.
**Tech Stack:** Rust, pinned HarfRust/Unicode crates, existing PDFium/lopdf/ttf-parser, Tauri/React only after the native gate.
**Spec:** ../specs/2026-09-11-shaped-text-design.md

- [x] Native experimental layout: serde types, resource limits, exact-font shaping, bidi/script segmentation, cluster/grapheme maps and missing-glyph errors. Pinned optional dependencies. The shaper's unexposed budget-exhaustion status remains a documented production limitation.
- [x] Serialization experiment: exercised actual bundled PDFium CID/position/marked-content/create/save APIs and explicit positioned PDF operations. Native glyph layout survives export, but portable logical Unicode and selection geometry **fail** the independent-reader gate. Negative controls retain those failures explicitly.
- [x] Fixtures/interoperability: redistributable Arabic/Indic/supplementary fonts with full notices and provenance, representative cases, measured reader compatibility and version-matched bidi/grapheme corpora.
- [x] Gate review: native result, positions, font bytes, Unicode and bounds measured. **Decision: gate failed; no production integration.** See `docs/shaped-text-interop.md`.
- [ ] Deferred until the gate passes: explicit versioned shaped-layout options, same native preview/geometry, cluster selection/search and logical IME input, validated annotation save/reopen, undo/recovery and resource lifetime tests.
- [x] Full regressions and independent review: frontend 209, default native 92, experimental native 105 passed; full named bidi/grapheme files passed. Findings resolved and durable verification/reader results recorded for #14. No user-facing release: integrated behavior remains deferred until the gate passes.
