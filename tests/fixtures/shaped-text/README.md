# Shaped-text development fonts

These unmodified static TrueType files are redistributable test inputs, not an
application font fallback set. `provenance.json` records immutable source URLs,
SHA256 digests, byte counts, name-table versions/copyrights, and licenses.
Run `python scripts/fetch-shaped-text-fixtures.py` to verify every downloaded
font and license; add `--fetch` to restore missing files without overwriting.

| Font | Sample purpose | Source and license |
| --- | --- | --- |
| Noto Sans Arabic Regular | Arabic joining and lam-alef in `سلام` | [Official Noto archive](https://github.com/notofonts/noto-fonts), commit `ffebf8c1ee449e544955a7e813c54f9b73848eac`; full OFL1.1 in `LICENSE_NOTO.txt` |
| Noto Sans Devanagari Regular | Pre-base reordering and clusters in `किताब` | Same official archive and license |
| DejaVu Sans 2.35 | Single-font mixed `ABC سلام 123 DEF`, including spaces/digits | [Matplotlib's bundled distribution](https://github.com/matplotlib/matplotlib/tree/b39c8c1cc1481a0b00101d34b74b552e63717740/lib/matplotlib/mpl-data/fonts/ttf), full license in `LICENSE_DEJAVU.txt` |

Noto's archived revision is intentional: it supplies small immutable static
TrueType inputs. It is not a claim about the latest Noto distribution. Both
Noto fonts cover ASCII digits but do not cover Latin `ABC`; a mixed sample must
use DejaVu Sans explicitly. No implicit font substitution is allowed in gates.
All three files have `glyf`, lack `fvar`, and report OS/2 `fsType = 0`.

Reuse `../corpus/fonts/DejaVuSerif.ttf` with its adjacent full license for
`office` ligatures, composed/decomposed accents, and `A\U0001D434B`
supplementary coverage. DejaVu Sans supplies the positive two-axis
`q\u0307\u0323` mark-positioning test. Serif's SHA256 is
`107244956e9962b9e96faccdc551825e0ae0898ae13737133e1b921a2fd35ffa`.
U+1D434 is MATHEMATICAL ITALIC CAPITAL A. This avoids an extra fixture font.

The full licenses retain upstream notices and attribution. No font has been
subset, renamed, modified, or obtained from Windows system font files.
