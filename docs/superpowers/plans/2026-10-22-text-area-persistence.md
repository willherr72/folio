# Text-area persistence implementation plan

Required execution skill: superpowers:executing-plans (inline).

Goal: editable PDFs and recovery retain exact area source data and fonts.
Architecture: optional area settings on TextOverlay, bounded native line preparation, version 4 portable annotation records, existing generic preview rendering.
Tech stack: Rust/serde/lopdf/PDFium, TypeScript/React/Vitest.
Spec: docs/superpowers/specs/2026-10-22-text-area-persistence-design.md

## Global constraints

Preserve earlier uncommitted copy-policy work. Work in the existing isolated worktree. No dependency upgrades, publishing, PR or commits are required for this milestone. Keep legacy text behavior and strict appearance verification. Recovery saves drafts even when export would fail.

## Task 1: Native schema and preparation

Produces: optional TextAreaSettings, shared alignment enum, area-aware prepare_text_overlay.
Consumes: existing text_area layout and semantic single-line preparation.

1. Add tests for exact area schema, multiline preparation and original character text; run and observe missing support (RED).
2. Implement strict settings and bounded line composition with unique font resources, full rectangle bounds and native glyph preview.
3. Run native area tests. Expected: schema, text, geometry and overflow cases pass.

## Task 2: PDF and recovery persistence

Produces: version 4 records and recovery validation.
Consumes: Task 1 settings/preparation and existing exact-font snapshots.

1. Add editable round-trip, rotation, version/tamper and recovery tests; observe failure before implementation.
2. Extend version checks and validation; keep recovery structural rather than layout-dependent.
3. Run focused native persistence/recovery suites. Expected: exact source/settings/font restoration, stable appearance, malformed records remain native, draft recovery succeeds.

## Task 3: Frontend restoration

Produces: typed area overlays, independent history snapshots and cache entries, editing safeguards.
Consumes: Task 1 JSON settings and generic prepared previews.

1. Add tests for different area cache keys, snapshot isolation and protected shaping/font controls; observe failures.
2. Extend types, clones, preview keys and saved-area properties behavior.
3. Run focused Vitest suites. Expected: area geometry changes invalidate preparation and edits preserve requirements.

## Task 4: Verify and review

Run all native tests with one Cargo job, cargo fmt --check, no-default-features check, frontend tests with two workers, production build and Python copy-policy tests. Expected: all pass. Document actual evidence and limitations. Request one fresh-context review of the final uncommitted diff; fix material findings with regression tests and rerun affected/full checks as appropriate.

## Review focus

Check coordinate conversion across area/overlay/page rotations, per-line font resource scoping, aggregate memory limits, original text delimiter preservation, metadata downgrade/fallback behavior, recovery of invalid drafts, and frontend ways to accidentally discard area requirements. Consider inputs beyond the tests and preserve existing single-line compatibility.
