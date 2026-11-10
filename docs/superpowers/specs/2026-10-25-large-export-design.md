# Reuse a bounded composition source parse

The measured 10,000-page synthetic source spends 6.86 s extracting five pages and 11.22 s saving all pages. Five preservation stages independently parse the same 2.9 MB source; page importing itself costs under 70 ms. These direct native results do not reproduce the earlier desktop delay over 60 s, which remains an open investigation.

Share an immutable, transaction-owned parse through SourceForm for single-source composition when input bytes are at most 8 MiB and the parsed document has at most 100,000 objects. Strict parsing and the existing 64 MiB decompression ceiling remain. The first existing form preflight performs the parse. Later resource, link, form and native-comment checks borrow it. Every preservation check and its traversal budgets still runs. Combined sources and larger sources use owned temporary parses as before. Drop the cache before final serialization/protection.

The coordinator chose this narrow scope under the user's authorization to work autonomously. Alternatives were changing PDFium page import (measured insignificant), a persistent document cache (unnecessary lifetime/memory cost), or retaining every combined source (larger memory cost). No dependencies, UI changes, output format changes, or input rewrites.

SourceForm privately owns parse state and exposes only read-only access; callers cannot pair a cached document with different bytes. Native comments reference the same SourceForm rather than accepting a second independent cache key. Existing other callers construct uncached SourceForms. Failure remains retryable; malformed parses are never cached as successful.

Validate repeated cached/uncached access, admission limits, strict rejection, reordered/duplicate/multiple sources, forms, links, annotations, crops, protected composition and resource repair. Compare same-machine timing and independent text/raster output. Check memory before expanding this fast path. A scoped correctness review and a fresh build are required. No release or parity claim.
