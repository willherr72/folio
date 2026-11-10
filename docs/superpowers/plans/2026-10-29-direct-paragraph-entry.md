# Direct Paragraph Entry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Open the verified paragraph workspace directly from page text through an explicit toolbar action.

**Architecture:** Keep the existing edit tool with a text/paragraph entry preference. Share paragraph preparation between direct and single-text-dialog entry; preserve source ownership and focus restoration.

**Tech Stack:** React/TypeScript, Vitest, existing Tauri/PDF engine.

**Spec:** docs/superpowers/specs/2026-10-29-direct-paragraph-entry-design.md

## Global constraints

No backend, dependencies, PDF formats, package overwrite or commits. Existing fidelity checks and memory budgets stay unchanged. Preserve all inherited working-tree changes.

## Review focus

- Nested preparation rejection leaves source unchanged and shows an error.
- Protected/unsupported adapters cannot enter unavailable tools.
- Apply/Cancel returns focus to the initiating toolbar action.
- Asynchronous results cannot target a different tab, adapter or source generation.
- Added toolbar action remains reachable at narrow windows.

## Task 1: Direct entry and shared preparation

Files: src/App.tsx; tests/occurrence-paragraph-app.test.tsx; tests/form-paragraph-app.test.tsx; tests/password-app.test.tsx.

- [x] Add failing app tests for direct paragraph entry without a modal, Cancel and focus, direct Apply/Undo/Redo, explicit return to Edit text, annotated capability refusal, nested preparation/rejection and protected tool availability.
- [x] Run the focused tests and preserve their missing-tool failure.
- [x] Add entry preference and toolbar control, sharing `beginParagraphRequest(request)` with existing paragraph action. Route errors to the originating dialog or app failure banner. Check active tab, source generation and adapter after asynchronous preparation.
- [x] Run the affected app/paragraph tests; make ambiguous old paragraph-button selectors exact.

## Task 2: Integration and evidence

- [x] Run the full frontend suite and production build; bind source fingerprints and test inputs.
- [x] Build a fresh desktop candidate. Exercise direct Word highlighted entry, cancel/refusal, keyboard, Apply/history, save/reopen and narrow controls; audit saved outputs independently.
- [x] Measure direct entry with a persistent controller and unchanged resource budgets if the fresh optimized candidate is ready; otherwise leave this resource claim explicitly pending.
- [x] Request one final scoped review; correct concrete findings locally, record exact gates and limitations in the execution ledger and quality roadmap. Keep the broader goal active.
