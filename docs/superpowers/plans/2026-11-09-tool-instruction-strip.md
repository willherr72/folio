# Tool Instruction Strip Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. This is a small runtime-found CSS correction; perform the correction directly and preserve the prior final code review without dispatching extra reviewers.

**Goal:** Keep tool instructions and placement controls from covering PDF text.

**Architecture:** A relative, wrapping flex row above the document viewport; content-sized action buttons. Existing React/native behavior is unchanged.

**Tech Stack:** Existing CSS, TypeScript/Vite, native WebView2/CDP and identity-checked Windows window resizing.

**Spec:** docs/superpowers/specs/2026-11-09-tool-instruction-strip-design.md

## Global constraints

Change only src/styles.css. Preserve accepted packages, qualified candidate, original PDFs, failed runs and 100-action history. No native/dependency changes. No CSS injection in final compiled acceptance. Native minimum client width is 900px; normal width is 1280px. Serialize desktop sessions, tests and compilation.

## Review focus

- Cropped top-of-page editable text must receive actual pointer clicks.
- Wrapped instruction text and action buttons must remain visible at 900px width.
- Cancel image placement must preserve the document; Place image must create exactly one undoable image.
- Existing signature Cancel and keyboard behavior retain their handlers and accessible buttons.
- Reserved-row layout must keep source-coordinate rendering and saving unchanged.

## Task — instruction layout correction

- [x] Create a native browser regression and reproduce actual banner/text overlap on the preserved candidate.
- [x] Validate the proposed two-rule CSS in an explicitly diagnostic owned session.
- [x] Replace the absolute banner rule and fixed button width in src/styles.css; preserve all other product files byte-for-byte.
- [x] Run focused/full frontend as appropriate and TypeScript/Vite; build a qualified optimized GUI with unchanged native inputs.
- [x] Run compiled native text-click and image action tests at 1280px/900px, verify raster/text and save/reopen, reconcile cleanup and record the scoped correction. Keep the broader goal active.

## Ledger

Started 2026-11-09 from the sealed lazy-selection checkpoint 7098f04f373236c18dfce9f93d1e5acf6205a8333043c61678f58ae063abb5d4. Chosen row layout removes visual and pointer obstruction together. No product change yet.

Native red-1 proved an actual text row intercepted by the banner. layout-probe-4 passed both declared widths, text clicks/Cancel, image Cancel/Place/Undo and save/reopen; it remains explicitly CSS-injected diagnostic evidence. Two helper issues were corrected and preserved: DOM node return-by-value serialization, and comparing a normalized run locator to source-bound operand whitespace. The dialog helper title allowlist now includes only the additional native Place an image title, preserving PID/title/control verification. Production rules exactly match the positive probe. Only src/styles.css differs from the sealed candidate; full frontend is running.

Completed: all 1,489 frontend tests pass; TypeScript/Vite and optimized GUI build pass with exact two-rule qualification and 1,290 unchanged native inputs. Compiled desktop-1 passes all 12 row-layout checks at 1280px and 900px client widths without injected CSS, real text-click/Cancel, image Cancel/Place/Undo, crop restoration and save/reopen. The independent reader confirms exact glyph data and full RGB at 72/144 dpi on both saved pages. Six corrupt-evidence controls and five identity controls pass. All six observed process roots and milestone private WebView profiles are absent. No additional reviewer was dispatched for this routine CSS correction. Accepted 0.34 and the launcher remain unchanged; goal active.
