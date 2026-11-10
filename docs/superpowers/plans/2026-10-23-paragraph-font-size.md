# Paragraph font-size implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Let people resize uniform existing paragraphs while retaining the original font, color, editable text, and surrounding PDF content.

**Architecture:** Add an optional effective font size to native paragraph preview. Keep the existing Rust entry point as an original-size wrapper; forward the optional size through the Tauri request. Native glyph probes, wrapping, baselines, output Tf and reopened validation all use the chosen size. A dialog number control defaults to the inspected size; size changes invalidate candidates just like text changes.

**Tech stack:** Rust/PDFium/lopdf, React/TypeScript, Vitest, Windows WebView acceptance.

**Spec:** Uniform paragraphs only. Explicit size1–300pt; finite values required. Omitted size retains original behavior. Font program and color remain unchanged. Existing area must contain all ink, with no overlap or surrounding changes; overflow rejects preview and retains the draft. No automatic shrinking or substitution. Reset font size restores original. Adjustment cancellation retains typography; a newly checked selection resets it. Apply commits exactly the preview, with one-step undo and recovery as before.

## Review focus

- Effective size under scaled text matrices and intrinsic page rotation.
- Embedded font encodings/metrics and missing glyph refusals.
- Invalid sizes and overflow must not mutate source bytes or create published candidates.
- Late preview results after size edits must release both source and URL.
- Untouched text, graphics, native annotations and protection must retain existing fidelity gates.

## Task

- [x] Write and observe failing native and UI regressions.
- [x] Implement optional native size and dialog control/reset/validation.
- [x] Pass native paragraph/comment fidelity suites, full frontend tests, and build.
- [x] Obtain one scoped review and address actionable findings (no correctness findings; both suggested coverage cases added).
- [x] Verify Windows edit/preview/apply/undo/save/reopen and independent PDF typography/pixels.
- [x] Record source fingerprints, copied executable, evidence and limitations.

Evidence: artifacts/paragraph-size-verification.json.709frontend/102files,46native/3binaries,TypeScript/Vite/debugbuild pass. Windows18ptedit then savedintrinsic90 reedit10pt pass; independentfont/color/size/neighbor/note/pixel checks pass. Initial native testarea and independentMuPDFicon-geometry assumptions corrected without production changes; failures preserved. No parity claim.
