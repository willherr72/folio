# Font-resource boundary experiments

The next #14 integration gate was tested on 2026-09-11. **Neither candidate
passes.** The supported experimental `create_semantic_pdf` path retains its
255-distinct-definition limit. The desktop editor and v0.9.1 are unchanged.

## Candidates and results

The font-bank candidate allocates up to 255 exact Unicode/advance/bounds
definitions per Type3 resource. It preserves the original text matrix and TJ
position adjustment when switching resources inside one BT/ET text object.
Returning to a previously used bank reuses the original code and metrics.
All existing scalar, outline, geometry and output-byte limits remain in force.

Three inputs contain 255, 256 or 511 covered alphabetic scalars from the pinned
DejaVu Serif fixture, followed by `AB`, the final alphabet scalar, and `CD`.
The suffix exercises resource switches back to earlier definitions. The prefix
counts are not promised definition counts: exact local metric differences can
require another definition for a repeated character. Each input is generated at
0/90/180/270 degrees and saved/reopened through the actual PDFium engine.

| Check | Font banks | Banks with whole-line ActualText |
| --- | --- | --- |
| PDFium exact original and saved Unicode | 9/12 | 12/12 |
| MuPDF and pypdf exact original and saved Unicode | 12/12 | 12/12 |
| Usable original character placement | Fails at 180 degrees | Fails despite exact copying |

PDFium reorders bank segments and inserts spaces in all three 180-degree
font-bank cases. The 255-prefix sample begins with `ƉCD B ABC...` instead of
`ABC...`. MuPDF and pypdf retain the source order. PDFium and MuPDF raster hashes
are unchanged by native save/reopen in all 12 cases; stable rendering does not
establish correct copying. The independent per-character geometry check also
fails those same three PDFium cases.

Wrapping the line in ActualText repairs extraction in all three readers but
does not repair selection. For the 511-prefix sample, PDFium places the first
and last character origins only 48.65% of the line width apart at 0/90/270
degrees, and 0.46% apart at 180 degrees. For the shorter prefixes the 180-degree
span is similarly collapsed (1.40% and 0.92%). Whole-line logical text cannot
replace accurate per-character geometry.

Raw source and extracted strings, source/font/output hashes, original/saved
results and measurements are retained in
[font-bank evidence](semantic-font-banks-negative.json) and
[ActualText evidence](semantic-font-banks-actualtext-negative.json).
Readers: bundled PDFium chromium/8044, PyMuPDF 1.27.1, pypdf 6.14.2.
No Foxit or Acrobat manual result is inferred from these checks.

## Safety boundary and regression checks

`create_semantic_font_banks_probe` is explicitly a negative research control,
available only with the nondefault `shaped-text` Cargo feature. The ordinary
semantic writer hardcodes both probe options off and rejects overflow before
constructing the PDF. No engine/editor/export call site invokes the probe.

Native regressions retain the original 255/256 refusal boundary, prove banked
copy failures at 180 degrees, and demonstrate ActualText geometry collapse.
The strict independent inspector now follows font switches and checks each
bank's mappings and actual code uses. It rejects unknown resources, missing
occurrences and malformed or redundant text operations. Its 12 unit tests pass;
the historical 32-case short and 28-case long matrices still pass every existing
text/geometry/save/raster invariant.
The current guarded serializer's regenerated 32-case short and 28-case long
matrices also pass all of those independent-reader checks. The complete native suite with
`shaped-text` enabled passes **120 tests**, with three existing opt-in tests
ignored; all 12 inspector tests and Rust formatting checks pass. Compilation
uses `-j 1` because the initial parallel build exceeded Windows' paging-file
capacity. Frontend and packaged-desktop tests were not rerun for this
feature-gated research change.

## Reproduction

Use new output directories; the generator refuses to overwrite evidence:

```powershell
cargo run -j 1 --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --example shaped-text-probe -- --semantic-banks artifacts/shaped-text/banks-new
cargo run -j 1 --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --example shaped-text-probe -- --semantic-banks-actualtext artifacts/shaped-text/banks-actualtext-new
cargo test -j 1 --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --test semantic_pdf
python -B -m unittest discover -s tests -p semantic_native_inspector_test.py
python -B -X utf8 scripts/inspect-semantic-native.py --input artifacts/shaped-text/banks-new --output artifacts/shaped-text/banks-inspected.json --summary artifacts/shaped-text/banks-summary.json --check-invariants
```

The last command intentionally fails. These bank probes are not the inspector's
accepted short/long matrix and have no mixed-direction refusal cases, so its
matrix/refusal gates stay false as well as the three actual reader failures.
The retained summary preserves those flags rather than relabeling a partial
matrix as a passing gate. ActualText is also outside the inspector's accepted
text grammar; its separate report records raw reader results and native geometry.

The next writer candidate must preserve ordering across font resources **and**
per-character selection before the limit can be lifted. Correct extraction alone
is insufficient. RTL word ordering, mixed-direction support and editor input/
preview/recovery integration remain separate open gates.
