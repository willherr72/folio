# Strict image predictor compatibility

Extend the private image-area publication path to Flate images with TIFF Predictor 2 or PNG Predictors 10–15. Keep 8-bit DeviceGray/RGB, the existing dimensions, sample budgets, occurrence isolation, and fresh publication rules. No UI or package change.

Use a small image-specific predictor decoder after the unchanged strict zlib decoder. A general permissive PDF decoder would lose checksum, full-input-consumption and exact output-size guarantees. Rasterizing the page would lose editable text and exact outside samples. The image-specific route preserves those guarantees.

Direct DecodeParms dictionaries may contain only Predictor, Colors, BitsPerComponent and Columns. Values must be direct integers. Defaults are 1, 1, 8 and 1. For prediction, require Colors/Columns to equal the actual image components/width and BitsPerComponent 8; refuse predictor use without Flate. Identity parameters retain previous behavior; supplied dimensions must be valid but do not affect identity decoding. Unknown keys, arrays and indirect parameters remain refused.

TIFF uses row-local, per-component horizontal differencing modulo 256. All PNG modes use each row's explicit tag 0–4, including when the declared predictor differs. Decode with exact row lengths, zero missing neighbors, wide Average/Paeth arithmetic and specified Paeth tie order. Invalid tags and truncated/extra bytes refuse. References: PDF 32000-1 section 7.4.4.4 (https://www.pdftron.com/downloads/pdfref.pdf), W3C filter rules (https://www.w3.org/TR/PNG-Filters.html), and local PDFium flate decoder.

Validate dimensions and remaining aggregate decoded sample budget before decompressing. The exact inflated bound is sample bytes plus one byte per PNG row, at most 48,000,000 + 32,768 bytes under existing image limits. Decode in place and compact PNG tags forward, without allocating a second full image. Existing encoded input and aggregate retained sample budgets remain 64 MiB. This bounds this helper's buffers, not total application RSS.

Acceptance: observed failing admission tests; fixed byte vectors and generated varied filters; bad parameters, lengths, row tags, zlib checksum/truncation/trailing data; exact native saved outputs and a second edit; independently generated predicted source PDFs decoded and rendered with independent readers; unchanged sources/package; one scoped fresh review. JPEG, masks, profiles, alternate bit depths, indirect parameter forms and broad performance/Windows acceptance remain open.
