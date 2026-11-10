# Mixed-format paragraph editing

The user asked to start with uniform formatting, then authorized continued work
toward full everyday PDF usefulness. Uniform support is not the final scope.
Execute inline in this worktree; no routine approval gates.

## Evidence and design

Current text_paragraph.rs prepare requires identical font/program/proof/color,
matrix scale and font size, then one font-resource object identity. Layout,
output checks and paragraph_memory.rs also assume one style. Removing only the
admission guard would flatten formatting or reject output later. Highlighting
and graphics collision refusal is separate and must not be bypassed.

Preserve attributed text as ordered {text, styleId} runs. Native inspection will
provide a bounded palette tied to verified source font resources/programs, sizes
and colors. IDs apply only to the current checked selection. Expected source
text/selection validation remains mandatory; the backend must validate IDs and
font/geometry claims rather than trusting frontend metadata. Newlines are authored
hard breaks; wrapping never rewrites the logical draft.

Chosen approach: explicit range edits and style spans, then per-run measurement
and publication. Whole-string heuristic diffs are ambiguous for repeated words;
uniform replacement loses formatting. Independent text boxes per style would not
reflow a paragraph. Retain the current uniform path until the mixed path has
equivalent verification.

Insertion at a caret inherits the preceding character's style (the first style
at offset zero); replacement inherits the first selected character's style. An
explicit admitted style may override this. Unchanged prefix/suffix retain their
style IDs. Offsets use DOM UTF-16 and must fall on grapheme boundaries. No Unicode
normalization or HTML ingestion. Paste normalizes CRLF/CR to LF. Deleting everything
retains the typing style in an empty run. Adjacent equal styles coalesce. Bound
text at 4096 UTF-8 bytes and the source palette at 64 styles. The draft model may
represent Unicode that native font admission later rejects; it does not add shaping.

Model integration contract: reconstruct deserialized snapshots through the factory.
An edit may join previously separate graphemes; incompatible retained style IDs
cause atomic rejection, leaving the old snapshot intact. Do not restyle neighbors
silently. After successful edits, snap the arithmetic caret with snapStyledCaret
before style queries or subsequent edits. Tests cover accents, ZWJ sequences and
regional indicators. Raw paste work is bounded at 8192 UTF-16 units; normalized
content and the complete draft remain limited to 4096 UTF-8 bytes.

## Milestones

1. [x] Reproduce font/size/color rejection on authored PDFs. Inventory the private
   quiz's styles/graphics without copying its text into fixtures.
2. [x] Implement src/editor/styled-paragraph.ts and tests for range edits, selection
   formatting, Unicode boundaries, limits and immutable undo snapshots. Scoped
   review. This is an internal foundation, not user-visible mixed-format support.
3. [x] Native read-only inspection: verified per-span resources/styles and logical
   separators; retain source-copy raster and source-byte invariance checks.
   Mixed no-op emission/glyph/save verification belongs to publication below.
4. [x] Native layout/publication: mixed-style words/lines, variable ascent/descent,
   spacing, alignment/justification and explicit overrides. Prove each emitted
   run's encoding/font/size/color/position and unchanged surrounding pixels.
5. [x] Faithful save/re-edit: versioned hints independently reproduced from actual
   operators/glyph geometry; malformed, forged and overlapping hints ignored.
6. [ ] UI: styled editor with range edits, composition, keyboard/undo, mixed-state
   controls, stale-preview cleanup and font leases. Retain drafts on failures.
7. [ ] Acceptance: standard/embedded fonts, sizes/colors, hard breaks, longer/
   shorter edits, all rotations, save/reopen/re-edit, comments, resource lifetime,
   collisions, independent reader text/style/pixels, full suites, review and Windows.

Milestone 2 may finish separately while native and UI work remain pending. Do not
claim mixed-format support or publish a replacement executable for unconnected
code. The overall goal remains active.

Milestone 3 evidence: `artifacts/mixed-paragraph-inspection-20261023/verification.json`.
Full native suite: 509 passing/four ignored before the final mixed-only row-size
correction; final corrected source: 44 focused paragraph tests, including ten mixed
inspection tests. Independent MuPDF font/size/color inspection comparisons pass for
three authored PDFs. Review's incoming-row admission finding reproduced and fixed;
64/65-piece and 4096/4097-byte boundaries tested. Source-copy raster equality does
not establish mixed no-op encoding or saved-output equivalence. No UI command yet.

## Native integration map (next milestone)

The read-only layout portion of milestone 4 is now verified separately in
`2026-10-23-mixed-paragraph-layout.md`. It handles source-bound metrics, words
across style boundaries, spaces/hard breaks, measured line boxes and all four
alignments. Publication evidence and remaining milestone 4 work are recorded below.

Source-font publication is now verified in `2026-10-23-mixed-paragraph-publication.md`:
immutable derived output, reopened source font/style/visible-glyph checks,
neighbor/graphics refusals, outside-area pixels, native comments and source policy,
save/export checks and 48 independent actual-output cases. Explicit per-style
overrides/substitution complete milestone 4 in `2026-10-23-mixed-paragraph-formatting.md`:
160 native tests (three ignored), 32 independent actual exports, reviewed source
and saved-output fingerprints. New imported fonts use matching integer CID widths
for cross-reader glyph positions and distinct resource identities from legacy fonts.
Native faithful mixed re-edit is verified separately in
`2026-10-24-mixed-paragraph-memory.md`: 203 selected native tests, 32 independent
first-publication cases, 128 saved exports and four 4096-fragment stress exports.
Versioned hints are independently replayed, large selections require complete
verified memory, and exact no-op appearance is checked. No GUI acceptance follows
from those native results. Maximum-fragment debug cycles remain about 31 seconds.
The next implementation plan is `2026-10-24-styled-paragraph-editor.md`.

- `prepare`: preserve per-member verified resource name/object identity and run
  proof rather than only `font_resource`. Build logical styled runs alongside
  existing separator/trim rules, assigning inserted separators explicitly. Only
  deduplicate styles whose complete resource/encoding/size/color identity matches.
- Keep mixed inspection separate from legacy uniform inspection until preview is
  ready. Do not return a mixed selection to a caller that submits plain text and
  one format. New DTOs should carry a palette plus run strings (no cross-language
  offset conversion); selected IDs/expected text remain bound to immutable source.
- `paragraph_baseline`: use each run's effective font size for extents. Uniform
  alignment/line-spacing inference and memory restoration cannot be reused without
  generalization. Variable-sized lines require measured ascent/descent and line
  boxes rather than first-font size multiplied by a line count.
- `paragraph_layout`: wrap words across style boundaries using cumulative advances
  and ink extents; a style change inside a word is not a break opportunity. Preserve
  explicit whitespace and hard breaks. Position justified gaps once, including
  gaps that cross run boundaries. Check negative bearings and actual visible glyphs.
- Publisher: emit each fragment with its verified font resource, size, color and
  origin; map expected fragments to reopened objects and verify actual glyphs.
  Keep cursor-only removals, nonselected object/glyph comparisons, comments and
  outside-area raster checks. Font import/substitution must retain resource leases.
- `paragraph_memory`: versioned mixed records need independent layout/operator
  reproduction, not style IDs or glyph coordinates treated as authority. Rebuild
  palette IDs from the reopened source. Old uniform version 1 stays readable.
- Existing selection/record limits are 64 pieces although output can contain many
  more style fragments. Do not create paragraphs that cannot be re-edited: derive
  bounded limits for source pieces/output fragments and measure worst-case runtime
  before exposing the path. Keep logical text at 4096 bytes and style palette at64.
- Source graphics/highlighting remains a separate fidelity problem. The quiz has
  six styles, two mixed MuPDF blocks and one vector drawing. Mixed text support
  alone is not proof that its highlighted instruction block can be reflowed.
