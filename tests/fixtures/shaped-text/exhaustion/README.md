# Hostile shaping fixtures

These intentionally adversarial GSUB and composite-outline programs are for native tests only. They derive from the repository's DejaVu Serif fixture, reduced to A, B and space; full redistribution notices are adjacent in `LICENSE_DEJAVU.txt`. `manifest.json` records source/output hashes. Regenerate with `python scripts/generate-shaping-exhaustion-fixtures.py` using fontTools 4.62.1.

- `recursive.ttf`: one contextual lookup invokes itself until HarfRust's recursion limit. Unpatched 0.13.3 returns an apparently ordinary A glyph without exposing failure.
- `budget.ttf`: each A invokes 8,191 identity substitutions followed by A→B. With sixteen input As, the unpatched operation budget interrupts after only four become B; the returned mixed B/A output still satisfies ordinary geometry/count checks.
- `expansion.ttf`: nine doubling substitutions followed by nine pair contractions. Final output is small even though intermediate glyph count grows by 512. The patched caller limit must reject intermediate excess, including when the input buffer already has spare reserved capacity.
- `composite.ttf`: fourteen levels of repeated binary components expand one A into 16,384 copies of the original A outline. The graph is acyclic and fits ttf-parser's depth limit, but exceeds Folio's aggregate outline-work budget before computing ink bounds.

Tests verify explicit rejection and also retain positive controls: unconstrained expansion/contraction of one A completes, and the ordinary Latin/Arabic/Indic corpus remains unchanged.
