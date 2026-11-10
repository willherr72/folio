# Existing Form Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Remove a logical form field and its linked appearances without altering other content, with Undo/Redo and save/reopen.

**Architecture:** A bounded native tree edit produces an immutable complete-source replacement. A dedicated inspection/confirmation dialog uses the existing active-tab source transaction.

**Tech Stack:** Rust, lopdf, PDFium, React, TypeScript, Vitest, Tauri.

**Spec:** docs/superpowers/specs/2026-10-26-form-removal-design.md

## Global Constraints

Preserve dirty worktree and original files. No commits, publishing, cleanup, dependencies, renderer changes or budget increases. One Cargo/build process at a time. Inline implementation with one final scoped review; corrections inline without another review round.

## Review Focus

1. Linked radio widgets must not be described as removal of just one option.
2. Shared indirect arrays must not corrupt unrelated annotations or field branches.
3. Actions targeting removed nodes/names must fail safely; global actions keep their meaning.
4. Empty ancestors and calculation-order entries must not become dangling fields.
5. Late native results must not replace another tab or source revision.

## Task 1: Native removal

Files: new `src-tauri/src/form_removal.rs`, `form_removal_engine.rs`, `src-tauri/tests/form_removal.rs`; integrate `lib.rs`, `engine.rs`, `app.rs`; minimal visibility changes in `form_properties.rs`.

Interfaces: `inspect_form_removal(source_id, page_index, annotation_index) -> FormRemovalInfo`; `remove_form_field(source_id, RemoveFormFieldRequest {page_index, annotation_index, expected_key}) -> DocumentInfo`.

- [x] Write regression fixtures/assertions for linked widgets, preserved siblings/content, nested ancestors, indirect arrays, CO and safe action refusal; observe missing API failure.
- [x] Implement bounded inspection/mutation and immutable engine publication with permission and stale-key checks.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --test form_removal -- --test-threads=1`; expand save/reopen, protection and orphan-reference tests, and run affected form-property/export checks.

## Task 2: Desktop removal

Files: new `src/editor/form-removal.ts`, `src/components/FormRemovalDialog.tsx`, focused tests; integrate adapter, App and FormFillEditor.

- [x] Test confirmation scope, loading/refusal/error/cancellation, duplicate submits and late candidate ownership before implementation.
- [x] Implement dedicated Remove field dialog; route source replacement through existing form commit and clear stale field selection.
- [x] Test app Undo/Redo, copied source pages, draft/permission gating and other-tab preservation; run focused tests and frontend build.

## Task 3: Acceptance

- [x] Freeze hashes, request one independent scoped review, address substantive findings with regression checks.
- [x] Build isolated executable and test remove/cancel/Undo/Redo/save/reopen in Windows; inspect saved PDF independently.
- [x] Record exact checks, limitations and artifact links in the ledger and product roadmap. Keep overarching parity goal active.
