# Native shaping dependency decision

This is the issue #14 native/API serialization gate. It adds no supported characters to legacy overlays and makes no production UI support claim.

The entire gate is behind the nondefault `shaped-text` Cargo feature. Default application builds omit its modules, public APIs and optional shaping dependencies. Run native evidence with `cargo test --features shaped-text --test shaping --test shaping_unicode_conformance --test shaped_pdf --test shaped_pdfium_api`; regenerate reader fixtures with `cargo run --features shaped-text --example shaped-text-probe`. `FontAsset::parse_for_shaping` is available only with that feature.

## Pinned dependencies

Registry versions and local crate contents were verified on 2026-09-11. Exact direct versions are in `src-tauri/Cargo.toml`; transitive versions and registry checksums are retained in `src-tauri/Cargo.lock`.

| Component | Version | Role | License | Unicode data |
| --- | --- | --- | --- | --- |
| HarfRust | 0.13.3 | OpenType glyph substitution and positioning | MIT | Upstream HarfBuzz-compatible generated tables |
| unicode-bidi | 0.3.18 | UAX #9 paragraph levels and visual run order | MIT OR Apache-2.0 | 16.0.0 |
| unicode-script | 0.5.8 | Script itemization | MIT OR Apache-2.0 | 17.0.0 |
| unicode-segmentation | 1.13.3 | Extended grapheme boundaries | MIT OR Apache-2.0 | 17.0.0 |

[HarfRust upstream](https://github.com/harfbuzz/harfrust) tracks HarfBuzz 14.3.1 in this release and shapes in units per em. Folio scales once into PDF points. HarfRust uses `read-fonts` for font parsing and retains a safe Rust public API. Its documented conformance differences remain relevant; a successful sample is not evidence of complete script support.

[RustyBuzz upstream](https://github.com/harfbuzz/rustybuzz) was archived on July 26, 2026 and recommends HarfRust. A new implementation should not adopt the archived predecessor. The Unicode crates' primary repositories are [unicode-bidi](https://github.com/servo/unicode-bidi), [unicode-script](https://github.com/unicode-rs/unicode-script), and [unicode-segmentation](https://github.com/unicode-rs/unicode-segmentation). Data versions above come from each pinned crate's `UNICODE_VERSION` constant, not the latest Unicode publication. Conformance fixtures must match those individual versions.

The lockfile adds `harfrust 0.13.3`, `read-fonts 0.43.3`, `font-types 0.12.5`, `bytemuck_derive 1.12.0`, and `unicode-script 0.5.8`. `read-fonts` and `font-types` are MIT OR Apache-2.0; `bytemuck_derive` is Zlib OR Apache-2.0 OR MIT. Existing pinned `ttf-parser 0.25.1` remains the legacy/font-permission validator and supplies static outline bounds. No system font fallback or platform shaper is used.

## Behavior and bounds

`shape_text` validates the exact font bytes against their SHA-256 ID and existing static monochrome TrueType/embedding-permission checks. `FontAsset::parse_for_shaping` admits a validated font even if it has no legacy-range glyphs; its `FontInfo.coverage` and `validate_text` retain legacy semantics. The shaping entry never calls legacy text validation.

Input is a nonempty single line, at most 4,096 Unicode scalars / 16,384 UTF-8 bytes, at most 64 scalars per grapheme, 256 visual/script runs, and 16,384 output glyphs. Font size is 1–1,000 points and absolute output coordinates are limited to one million points. Font bytes retain the existing 16 MiB cap. Logical Unicode is never normalized or reversed in the returned text. Glyph clusters and grapheme boundaries retain both UTF-8 byte and UTF-16 code-unit offsets. Glyph origins include offsets, use baseline zero, and have a positive-up y axis.

Native dependency checks consume all 766 rows of Unicode 17.0.0 `GraphemeBreakTest.txt` and a deterministic 286-row sample of Unicode 16.0.0 `BidiCharacterTest.txt`, checking paragraph levels, per-scalar levels, X9 removal and visual order. The bidi selection covers first/last rows and evenly spaced rows from the complete 91,707-row source; it is sampled coverage, not full UAX #9 conformance. Versioned URLs, exact source/sample SHA-256 values and the Unicode license are in `tests/fixtures/shaped-text/unicode/provenance.json`. These tests exercise dependencies directly, including controls that Folio deliberately refuses.

For a full development run, set `FOLIO_FULL_BIDI_TEST` to a downloaded Unicode 16.0.0 `BidiCharacterTest.txt` and run `cargo test --features shaped-text --test shaping_unicode_conformance -- --nocapture`. The runner checks its exact SHA-256 before use and requires all 91,707 rows. On 2026-09-11, that full character-level corpus and all 766 grapheme rows passed (1.63 seconds of test execution). This covers the complete named files; it does not claim the separate class-based `BidiTest.txt`, paragraph-splitting rule P1, or rendering rules L3/L4, which the source character corpus explicitly excludes.

ZWJ and ZWNJ are admitted as intentional shaping input. Explicit bidi controls, line breaks, invisible separators, soft hyphen, variation selectors and tag characters are refused pending an explicit UI/selection policy. Missing shaped glyphs fail rather than selecting another font. The ligature option controls optional `liga`, `clig`, `dlig`, and `hlig`; required shaping features such as Arabic `rlig` remain enabled.

HarfRust 0.13.3 bounds buffer expansion internally to `max(input_scalars * 256, 65536)` and operation budget to `max(input_scalars * 4096, 65536)` (`src/hb/buffer.rs`). Its public `GlyphBuffer` does not expose the internal `successful` flag or a typed budget-exhaustion error. Folio checks returned glyph counts, character-boundary clusters, IDs, positions and geometry, but those checks cannot prove that an internal budget interruption completed all requested OpenType lookups. This is a production hardening limitation of this gate, to resolve before exposing arbitrary imported-font shaping as supported UI behavior.

## Redistribution

The pinned crate packages include full notices: HarfRust `LICENSE`; Unicode crates/read-fonts/font-types `LICENSE-MIT` and `LICENSE-APACHE`; bytemuck_derive additionally `LICENSE-ZLIB`. HarfRust's MIT notice credits HarfBuzz developers and Yevhenii Reizner. Existing release license collection must include these newly locked packages before distribution; this gate does not create a release. Test-font licenses and exact provenance live beside `tests/fixtures/shaped-text` and the existing DejaVu corpus.
