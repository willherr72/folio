# Original quiz editing and sustained re-edit acceptance

Exercise the actual user-provided quiz on the current verified native candidate. The user's original failure was inserting a short word; the goal also requires useful paragraph reflow and faithful saved re-editing. The previous turn completed the original-page export milestone and is classified as progress. This session must add direct evidence or fix a demonstrated defect, not repeat status or claim commercial parity.

Use the existing Rust/PDFium CLI and Python MuPDF/pypdf audits. No external fonts are supplied. The original attachment and retained input copy remain byte-identical. All private quiz content, outputs and logs stay in artifacts/quiz-growth-session-20261103 and are excluded from distribution. Accepted portable 0.31 stays unchanged.

Approaches: re-run the old two-word smoke test (cheap but weak coverage); exercise saved paragraph sessions with fixed source-derived regions and two independent readers (chosen by coordinator); build a new GUI workflow first (useful later, but introduces UI variables before native fidelity is known). Standing autonomous authorization replaces approval questions. This is a bounded acceptance milestone; product changes require a reproduced defect.

Cases: first-question word insertion and another saved edit; final-question growth into explicit available space with real wrapping and preserved font/size/color; highlighted instruction growth and saved re-edit with per-character style retention. Re-open every output through fresh inspection. Include unchanged re-edit pixel equality and an alternating add/remove sequence to measure sustained behavior.

The oracle fixes permitted page regions before seeing any output. It checks complete normalized extracted page text in MuPDF and pypdf, selected character styles and line counts, unchanged glyphs and exact pixels outside the fixed region, and source immutability. Output geometry must remain inside the preselected region. Candidate hashes must equal saved hashes. Negative controls must reject stale text, changed unselected content, changed style and changed geometry; a true unchanged control must pass its appropriate equality check. Text whitespace normalization is an explicit limit, not exact layout-order certification.

Record wall time, output bytes, object/font/resource counts and observed process memory for the sustained sequence. A finite sample is not a leak-free or commercial performance guarantee. Do not invent a passing resource threshold after seeing results; investigate unexplained unbounded growth and record limitations honestly.

No frontend changes or new package are planned. Reuse the just-completed 2,604-test native gate only if product/test fingerprints remain identical. If a product defect is found, add a minimal regression, observe RED, fix, and run the relevant tests and required wider checks. One integrated evidence/code review at the end, one correction pass, no re-review loop.
