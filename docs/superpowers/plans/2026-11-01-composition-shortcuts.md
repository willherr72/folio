# Composition-safe shortcuts implementation plan

> For agentic workers: Use superpowers:executing-plans to implement inline.

**Goal:** Keep input-method keystrokes from discarding drafts or navigating search.
**Architecture:** Reuse the existing native-event composition/229 guard at the two
shared shortcut boundaries; preserve propagation isolation and normal behavior.
**Tech stack:** React, TypeScript, Vitest, Playwright/Edge.
**Spec:** docs/superpowers/specs/2026-11-01-composition-shortcuts-design.md

## Constraints and review focus
No new dependencies, native changes or portable rebuild. Preserve inherited work.
Test both composition representations; preserve default handling, shortcut
isolation, post-composition normal shortcuts, document draft and focus ownership.
Browser harness must use current product components and actual keyboard input for
narrow traversal. Synthetic composition evidence must be labeled explicitly.

## Task 1: Reproduce and correct shortcuts
- [ ] Add tests/composition-shortcuts.test.tsx exercising real Modal,
  DocumentPropertiesDialog and SearchBar; both isComposing and keyCode 229.
- [ ] Run `npm test -- tests/composition-shortcuts.test.tsx` and retain failure.
- [ ] Add guard to src/components/Modal.tsx and src/components/SearchBar.tsx.
- [ ] Run focused dialogs/search/properties regressions then full `npm test`.

## Task 2: Integration and review
- [ ] Run `npm run build`.
- [ ] Bundle actual components into a local file harness (no server); launch an
  owned headless Edge instance, exercise 840x640 keyboard editing/Apply/Cancel,
  synthetic composition dispatch and search normal keys, then close it.
- [ ] Fresh scoped review; correct any demonstrated important defects.
- [ ] Record tested hashes, results and explicit limits in a checkpoint and roadmap.

No commit or deletion of inherited artifacts is part of this cycle.
