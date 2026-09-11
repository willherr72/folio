# Experimental shaping resource checks

The nondefault `shaped-text` feature uses an exact-version HarfRust 0.13.3 patch to reject interrupted shaping. This work does not enable additional production text input. PDF extraction, geometry and reader compatibility remain separate gates.

## Why a local patch is necessary

The original `GlyphBuffer` exposes glyphs and positions but no completion status. Its internal `successful` flag already records glyph growth and contextual-lookup recursion failures. Several lookup paths terminate when `max_ops <= 0` without setting that flag, and `leave()` resets the counter. Output count, valid IDs, clusters and geometry cannot distinguish successful shaping from a partially applied sequence of lookups.

Two small real GSUB fixtures demonstrate this directly. In unpatched 0.13.3, a self-recursive contextual lookup returned an ordinary A glyph. A finite contextual sequence with sixteen input As applied the final A-to-B substitution only four times before returning twelve unchanged As. Both results passed the former layout postconditions. The regression failed because partial layouts were accepted; this was not an inferred timeout.

## Exact patch and redistribution

`src-tauri/vendor/harfrust-0.13.3` contains the crates.io package from [the versioned archive](https://static.crates.io/crates/harfrust/harfrust-0.13.3.crate), SHA-256 `948d0741125ba89cd3e1c23e5642415b6ade7e1d29d67ba25fb925b533e989d6`, upstream revision `fbff7e563b4e512686c9e627d91a147ba4ee2402`. Its 85 upstream files are retained, excluding only the registry cache marker `.cargo-ok`. Only `src/hb/buffer.rs` and `src/hb/face.rs` are modified. The upstream MIT `LICENSE` remains complete.

`FOLIO_PROVENANCE.json` records every original file hash and both patched hashes. `FOLIO_PATCH.patch` contains the complete source diff; `FOLIO_VENDOR.md` explains its scope. Cargo resolves this exact version through a local `[patch.crates-io]` entry; no dependency version is upgraded. The original archive checksum lives in the provenance file because Cargo removes registry checksums from path dependencies. This is a local patch, with no claim of upstream review or acceptance.

Run `python scripts/verify-shaping-vendor.py` from the repository root to check the complete vendored inventory and all 85 recorded file hashes, including the two documented changes. This offline check verifies the retained local bytes; the separately recorded archive checksum and upstream revision identify their source.

The API adds `GlyphBuffer::is_successful()` and optional `ShapeOptions::max_glyphs` / `max_operations` setters. `None` retains the original defaults. Explicit glyph limits are checked before accepting existing reserved capacity, so a large reusable buffer cannot bypass the cap. The original failed status and an exhausted operation counter are preserved before `leave()` resets counters. Folio reads the completion flag before consuming any shaped glyphs.

## Concrete limits and scope

- Existing input limits remain 4,096 scalars, 16,384 UTF-8 bytes, 64 scalars per grapheme, 256 script/bidi runs and 16 MiB static monochrome TrueType font data.
- Each run receives the remaining portion of the 16,384 final-glyph allowance as its intermediate glyph cap. The cap bounds requested initialized slots in each backing array. Input and output segments can coexist during a lookup; it does not assert their combined logical length is below the cap.
- The operation allowance is `min(run_scalars * 1024 + 8192, 262144)`. Across at most 4,096 scalars / 256 runs, total supplied allowances are at most 6,291,456 instrumented operations. Recursion exhaustion also rejects the layout.
- Glyph info and position values are 20 bytes each on the tested target. Two initialized arrays of 16,384 slots contain 655,360 bytes of element payload. This excludes allocator capacity, shaping/font caches, input buffers, the layout and process overhead.
- A shared `OutlineBudget` validates raw `loca`/`glyf` components before ttf-parser's actual bounds or outline traversal. It permits depth indices 0–15 and charges up to 100,000 expanded component nodes plus simple points and contours across distinct output glyphs. Repeated component references are charged repeatedly because the outline parser expands them repeatedly. Bounds are calculated and cached once per distinct output glyph. A glyph declaring nonempty contours must produce an outline. Missing, descending and out-of-range glyph offsets are refused; only a valid equal offset pair represents an empty glyph. Point-matched composite arguments are refused because ttf-parser 0.25.1 does not consume those arguments, which would make preflight and actual traversal disagree.

These guards are not a hard wall-clock deadline, a process memory quota, or a guarantee that all font parsing and cache construction is constant-time. Rust allocator failures outside the guarded arrays retain normal Rust behavior. The development subprocess probe has its own ten-second deadline; that is an observation tool, not the production API's enforcement mechanism.

## Reproducible evidence

`tests/fixtures/shaped-text/exhaustion` contains four intentionally hostile, licensed DejaVu-derived fonts: recursive GSUB, finite interrupted GSUB, expansion followed by contraction, and an acyclic repeated-composite outline. The adjacent manifest records source and generated SHA-256 hashes, and the full DejaVu notice is retained. Regenerate using `python scripts/generate-shaping-exhaustion-fixtures.py` with fontTools 4.62.1.

Run `cargo test --features shaped-text --test shaping -- --nocapture` from `src-tauri`. All 13 tests passed on 2026-09-11 (0.30 seconds of debug test execution). The tests cover explicit recursion and operation failures, zero caller budgets, temporary glyph expansion despite small final output, and a pre-reserved buffer that must still obey its explicit cap. Positive controls cover ordinary Latin ligatures, Arabic, Indic, marks, supplementary Unicode and unbounded-default expansion followed by contraction. The repeated-composite regression originally accepted a single glyph after 184.7 ms of outline traversal; it expands 16,384 copies of A despite occupying only 4,604 bytes on disk. The guarded run rejected it after 6.89 ms. Separate RED-to-GREEN mutations cover point-matched arguments and a descending glyph offset that formerly turned the A in AB into blank ink. These timings describe individual development runs, not benchmarks or API deadlines.

For process observations, run `scripts/probe-shaping-resources.ps1 -TestExecutable <built-shaping-test.exe>`. The script runs the hostile GSUB test in a child process, samples working set and paged memory, records the executable hash and elapsed time, and kills only that child if its ten-second development deadline expires. The final patched debug observation on Windows completed in 358 ms with a sampled peak working set of 8,568,832 bytes and peak paged memory of 2,097,152 bytes (test executable SHA-256 `6a843da0b490e62b4f2224b00b06b891baabec494ab5639517ab1d61700229ef`). These machine-specific samples include runtime/DLL overhead and are not allocation-cap proofs.
