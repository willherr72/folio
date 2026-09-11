# Version-matched Unicode test data

These files are development test inputs, not fonts or application resources.
`LICENSE_UNICODE.txt` is the complete Unicode License V3. Source URLs, complete
source SHA256 and sizes, output SHA256 and sizes, and the exact bidi selection
algorithm/line numbers are recorded in `provenance.json`.

- `BidiCharacterTest-16.0.0-sample.txt`: 286 unmodified test rows selected from
  the 91,707-row Unicode16 file for `unicode-bidi0.3.18`. Keep the source header.
  Select the first16 and last16 rows plus256 evenly spaced zero-based test-row
  indices `floor(i * (N - 1) / 255)`, then deduplicate and sort. This is sampled
  BidiCharacterTest coverage, not full UAX9 conformance or BidiTest coverage.
- `GraphemeBreakTest-17.0.0.txt`: complete unmodified Unicode17 file for
  `unicode-segmentation1.13.3`. The dependency uses newer Unicode data than the
  current bidi crate; do not claim the two have identical data versions.

Run `python scripts/fetch-shaped-text-fixtures.py` to verify all fixture bytes.
`--fetch` restores missing inputs and reproducibly rebuilds the bidi subset,
checking the complete source hash before selecting rows. The larger full bidi
source is not vendored. Test execution and any excluded rows must be reported
separately from fixture preparation.
