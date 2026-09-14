# Text-area layout implementation plan

> Execute with superpowers:subagent-driven-development. Independent native, evidence, and review tasks; root owns reader/preview harness and final integration.

**Goal:** deterministic bounded native Latin wrapping and honest preview/PDF compatibility evidence.
**Architecture:** new pure native layout module; existing shaped output used only by a development example; no desktop feature enabled.
**Tech:** Rust/HarfRust, PDFium, Python MuPDF/pypdf, browser SVG.
**Spec:** ../specs/2026-09-14-text-area-layout-design.md

## Tasks
- [x] Native owner: write failing tests then implement text_area.rs, lib exports, focused tests and Cargo test registration. Agree exact output names with root/example owner. Bound shape work before each call; preserve ranges, overflow and atomic tokens. Run focused tests with sole Cargo slot.
- [x] Evidence owner: implement examples/text-area-probe.rs using request/layout API and existing shaped overlay preparation/export. Write fixed specimens/native layout+preview manifests and PDFium raw text, original/resaved PDFs. Own example only, no Cargo until native handoff.
- [x] Root: strict Python reader/geometry/raster inspector and developer preview artifact, tests for inspector false positives. Record failed logical-copy gates honestly.
- [x] Independent reviewer: verify source range/overflow/work limits and evidence no false passes; resolve findings before completion.
- [x] Root: full native regressions and feature-disabled build check; docs and issue progress prepared for commit/ff/push under standing authorization. No release/version bump for a development-only API checkpoint.

## Rules
Use existing isolated positioned-text worktree on text-area-layout. One Cargo process, -j1, primary cached target. No dependency updates. Existing user authorization covers implementation/issue progress; no repeated approval. Preserve unsupported bidi/IME/real-reader gates. Do not add a desktop tool or claim #16 complete.

Ruling: MuPDF generates Type3 font labels containing object references. Native import renumbers them; the inspector removes only that exact generated label from geometry comparison and still records raw-dictionary inequality. Every coordinate and text comparison remains exact. The initial failing report is retained in local artifacts. No exact-copy gate was relaxed.
