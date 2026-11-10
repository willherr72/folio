# Source-language scopes implementation plan

> Use superpowers:executing-plans. Existing autonomous execution authorization applies; preserve old evidence and unrelated work. No commits or release.

**Goal:** Prepare a bounded, byte-preserving page-content language scope for structured composition. This is a prerequisite, not closure of the combined-document language finding.

**Architecture:** Reuse lopdf's opt-in resource accounting for strict content decoding, inspect complete page-content scopes, then produce only a prefix/suffix. Never re-encode source operations. Integration must separately preserve annotation/document text strings, structure inheritance and reusable-form language context.

**Spec:** Parent `2026-10-30-structured-source-assembly-design.md`, with PDF 1.7 sections 10.5 and 10.8.1 (pp. 850–854, 937–941) and text-string language escapes on p.159. The source reference is retained in `artifacts/combined-editing-save-20261030/references/`.

**Decision:** A catalog-only change is insufficient for partly tagged sources. Rewriting every content operator would needlessly alter original numeric lexemes. An outer Span can preserve default page-content language while retaining inner overrides. It cannot alone preserve comments or document information; do not connect it to Save until those semantics are handled.

## Constraints and review focus

- Source streams remain byte-identical; this helper returns framing bytes only.
- Inspect at most 16 MiB input, 32 MiB copied parser payload and 200,000 parser attempts per call; caller carries remaining budgets across calls. These are logical limits, not exact RSS bounds.
- Inline images remain outside this first controlled content decoder; refuse before legacy image parsing or integer arithmetic.
- Nested strings/arrays/dictionaries and operator-only streams must consume budgets before allocation; cancellation must unwind parser state.
- Reject unbalanced/crossing text and marked-content scopes, incomplete path objects, unknown operators, and a new marked clipping sequence containing q/Q.
- This helper applies to complete page content with the page's initial text-rendering state. It does not authorize Forms, annotation appearances, editing, or full tagged-PDF validity.

## Task 1: Controlled strict content decoding

Files: vendored lopdf `load_limits.rs`, `parser/mod.rs`; `src-tauri/src/composition_io_tests.rs`.

- [x] Add failing tests for operation-only and scalar-operand budget exhaustion, string payload limits, aggregate allowances, cancellation/unwind, malformed input, and refused inline images.
- [x] Implement `Content<Vec<Operation>>::decode_strict_with_resource_control(bytes, limits, cancelled)` using the existing RAII scope. Charge operator strings and each operation/operand attempt; retain ordinary decoding behavior.
- [x] Run the focused composition I/O tests; bind test/source hashes and retain RED/GREEN logs.

## Task 2: Page-content language envelope

Files: `composition_language_scope.rs` and tests; private module registration.

- [x] Add failing tests for partly tagged text, nested language overrides, exact numeric/string bytes, scope crossing, clipping-only q/Q, malformed/oversized/cancelled input and adversarial language bytes.
- [x] Implement an inspected prefix/suffix using hexadecimal language bytes and balanced scopes. Return remaining parser allowances; keep Save unchanged pending complete integration.
- [x] Run focused and library regression. Request one final scoped review, correct demonstrated defects, and record the remaining integration obligations in the continuation checkpoint.

Verified evidence: `artifacts/source-language-scopes-20261030/checkpoint-evidence.json`.
The single review's two P2 findings were corrected by the coordinator with RED/GREEN evidence. A standalone authored Word probe additionally passed independent object, raw-glyph/link and exact 1x/2x raster comparisons. This does not close the parent integration task.

## Follow-on integration (not claimed by this plan)

Define and verify effective-language preservation for reusable Form invocations and all retained text-string fields (including Info, comment Contents/title/subject and unstructured alternate text). Language escapes support only specific language/country encodings and may require reader compatibility work; do not infer a general solution. Add an independent semantic oracle, then wire the adapter into Save and repeat the real combined save/re-edit workflow.
