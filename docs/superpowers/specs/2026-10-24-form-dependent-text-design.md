# Consecutive text pieces inside reusable PDF forms

## Intended behavior

Continue the user's authorized autonomous editing-quality work. Valid forms often
draw several pieces with consecutive Tj/TJ operators after one initial line
position. Folio currently rejects the entire form because TextFlow marks the
cursor dependent after the first show. Enable editing an individual piece while
preserving later pieces, shared occurrences, font/spacing state and original files.
Keep the existing collision checks; a longer replacement must fit available space.

## Coordinator's implementation choice

Use the existing source-bound form stream rewrite, not a page raster replacement
or wholesale native regeneration. Admit consecutive shows after a known initial
position. When a later show depends on the selected show's advance, retain the
old end cursor with a bounded numeric-only TJ adjustment following the replacement.
Line-matrix operations and quote operators still retain their own semantics.
Numeric-only TJ creates no selectable text object, so public occurrence paths
must remain stable. Coalesce adjacent cursor-only adjustments on repeated edits.

Compute the adjustment from verified encoded font widths, raw font size, Tc/Tw,
and every explicit TJ adjustment. Horizontal scale is shared by the show and its
correction. A substituted font must use its actual installed PDF width policy,
not an assumed outline advance. Test CID fractional-width behavior independently;
if PDFium/spec discrepancies prevent faithful publication, diagnose that boundary
instead of relaxing surrounding-object/pixel checks.

Preserve leading/trailing ordinary spaces in nonblank pieces where encoding/native
mapping can be proved. A whitespace-only operator and arbitrary unsupported scripts
remain separate compatibility questions, never silently removed or exposed as a
verified empty replacement. Malformed operators, font ambiguity, clipping,
transparency, collisions, singular transforms and resource limits remain guarded.

## Acceptance

- Reproduce current rejection with an actual nested/shared form fixture.
- Edit first/middle/last pieces; include Tj/TJ, numeric cursor moves, local font or
  spacing changes, following T*/Td/TD/quote lines, and surrounding ordinary spaces.
- Standard, embedded simple and Identity-H fonts; transformed/rotated forms.
- Prove unselected occurrence text/style/geometry and unaffected page pixels;
  save/reopen/re-edit, undo/recovery/protected-document behavior through existing APIs.
- Font substitution must preserve subsequent source font and cursor; repeated
  edits must not accumulate unbounded compensation operations.
- Check exported files with independent MuPDF/pypdf readers, not only native tests.
- Run relevant existing native regressions, one fresh scoped review and one
  correction pass; build and exercise a self-contained Windows checkpoint.

No commits, external publication, original-file overwrite, or parity/release claim.
Keep the accepted renderer-fix checkpoint available. User waived routine permission
and design questions; execute inline in the existing isolated worktree.

## Required renderer prerequisite, established by the failing integration

The first implementation passes standard-font edit/save/reopen/restore but fails
the unchanged-neighbor guard for embedded simple fonts. A direct-DLL probe, with
no Folio editing code, now proves truncation of fractional `/Widths` and CID `/W`:
implicit versus explicitly positioned text differs by 0.050003 points in PDFium,
while MuPDF matches. Integer-width controls match in both readers. Source uses
GetIntegerAt and integer caches for both paths. Do not compensate with truncated
metrics: that would preserve the wrong positions in independent readers.

Build a separate renderer candidate exposing a precise horizontal advance getter
for explicitly defined bounded fractional widths. Keep legacy integer glyph/hint
interfaces and vertical metrics unchanged. Route text-object positioning and the
public floating glyph-width API through the precise getter. Preserve source-width
fallback behavior for malformed/unavailable entries; never use NaN/overflow as a
cursor. Retain the prior DLL, source recipe and accepted executable until candidate
direct/native/independent acceptance completes. Review the resulting source patch
alongside the Rust feature. Any further changed behavior must be demonstrated by
tests before adoption; broad renderer regressions are required again.
