# Legacy paragraph replay fixture

`legacy-highlight-v2.pdf` is an authored Folio test page, copied from the verified
2026-10-24 highlight-publication checkpoint before native-ink measurement mode was
introduced. It contains test words, a yellow rectangle, Courier and an embedded
DejaVu Serif font. It contains no private/user document content.

The embedded font's license is in `../corpus/fonts/LICENSE_DEJAVU.txt`. This binary
must remain a version2 golden fixture; do not regenerate it with newer writers.
Regression tests prove that the original layout and exact rendered pixels survive
no-op re-edit without switching measurement policy.

`native-ink-v3.pdf` is an authored Windows export from the verified paragraph-fit
checkpoint, before isolating individual native metric probes onto scratch pages.
It contains AA, Café marked words, CC and Neighbor, with an embedded DejaVu Serif
font and yellow highlight. It is copied from
`artifacts/paragraph-fit-corrected-20261024/desktop-accepted/Edited-0.pdf`.
SHA256: `aaa7da68fa6103ec148e4446b0dca5fc7eb3a16510cf3df4a11a8764cdd1959b`.
The same DejaVu license above applies. Keep this version3 golden unchanged to
verify existing native-ink saved paragraphs retain their exact rendering.
