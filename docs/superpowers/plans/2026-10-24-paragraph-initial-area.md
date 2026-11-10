# Measured initial styled paragraph areas

Execute inline using systematic debugging, TDD and verification. User authorizes
autonomous project work; preserve existing worktree and prior checkpoints.

Goal: an ordinary supported paragraph gets a useful initial area measured with
the same fonts/layout as its preview. Never alter saved verified paragraph bounds,
silently shrink/change fonts, move the paragraph, or bypass collision checks.

Observed: authored Mixed-0.pdf, Helvetica12 plus Helvetica-Bold18, source text
One two, suggested raw width56.196 and height22.5. Unchanged native preview fails;
explicit width250/height100 succeeds. Determine exact mismatch with native probes.

Plan:
- [x] Reproduce unchanged preview failure with native fixtures; compare layout
  at original bounds, wider bounds and taller bounds. Include crop/rotations.
- [x] Reuse real measurement to propose a compact useful extension from the
  existing top/left, bounded by page and neighboring text/graphics. Preserve the
  current bounds if already fitting or if no safe automatic extension is proven.
  Inspection must remain available when a font needs explicit substitution.
- [x] Cover successful initial preview/publication, nearby obstacles, insufficient
  page room, stored bounds, original immutability and bounded measurement work.
- [x] Run affected native suites, review, build and exercise packaged desktop
  preview with unchanged suggested bounds. Record hashes and remaining limits.

Highlight source attribution remains staged. This sizing correction must not
grant source highlights or arbitrary graphics a new overlap exemption.
