# Folio's strict JPEG entropy patch

Base: the crates.io jpeg-decoder0.3.2 archive, SHA256
`00810f1d8b74be64b13dbf3db89ac67740615d6c891f0e7b6179326533011a07`.
Upstream MIT and Apache-2.0 licenses are retained. This is a local modification,
not an upstream release or endorsement. Upstream version/dependencies are unchanged;
Folio disables default features for this dependency.

The image-asset importer must reject truncated image data rather than retain invented
pixels. Both image0.25.10's JPEG backend (including direct zune strict mode) and this
upstream decoder accept the regression: a valid80x40 JPEG header, one entropy byte,
then an early EOI marker.

The patch in src/huffman.rs tracks actual entropy bits separately from padded
lookahead bits. Lookup tables may peek at padded bits, as before, but consuming them
as a Huffman code or coefficient returns an error. All four consumption paths
propagate the error; restart reset clears both counts. Actual byte stuffing and
scan padding remain available. This does not add a whole-decoder memory limit or
claim all malformed JPEG structures are detected.

Folio's image_assets integration tests cover the failing JPEG, several truncation
lengths, missing EOI, all EXIF orientations, and fifteen original Pillow-generated
RGB/grayscale/CMYK, baseline/progressive, tiny/odd-sized and restart-marker fixtures.
Decoded samples are compared with independent Pillow/libjpeg reference images.
Tests and fixture generator are outside the vendor directory.

Verify source provenance using `python -B scripts/verify-jpeg-vendor.py` from the
Folio root. The helper checks the cached pinned archive, every original file, and
the recorded exact patch and source hashes. After an intentional patch change use
`--refresh`; it never edits library source. Root .gitattributes disables newline
conversion for vendored files. Before replacing this fork, rerun the malformed and
valid-image regressions against the proposed decoder.
The local gitignore also retains the published Cargo.lock for complete archive
verification and excludes Cargo's extraction marker; no dependency versions change.
