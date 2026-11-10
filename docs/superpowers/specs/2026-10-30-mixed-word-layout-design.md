# Mixed-format Word paragraph layout acceptance

The product-quality roadmap still lacks a combined real-producer check for mixed
styles, highlights and layout-sensitive paragraph narrowing. Exercise the verified
0.26 package first; choose a product correction only from a reproduced failure.
This is an acceptance task within the user's standing autonomous-work instruction.
The fixture and execution choices below are the implementer's choices.

## Approach

Reusing the existing Word status line covers bold/italic/color but lacks highlights
and substantial wrapping. The existing synthetic recovery fixture covers highlights
and canvas enlargement, but cannot establish original Word export compatibility.
Create a fresh authored DOCX and use a newly owned hidden Word instance to export
tagged and untagged PDFs. Keep those exported bytes immutable. This combines the
missing cases without rewriting a producer PDF to fit Folio's implementation.

Use Letter pages with 54 pt margins, Arial 11 pt body, a bold phrase, a blue italic
phrase and a yellow-highlighted phrase in one wrapped paragraph. Include a heading,
header/footer, external link, generous space below the selected paragraph, and an
untouched second page. Add `this` inside the highlighted instruction; the original
highlight already contains those letters. Preserve all other text and source styles.

## Acceptance

1. Both original exports have two pages, expected source text, embedded fonts,
   independently visible mixed styling/highlight paint and a preserved external
   link. The tagged export has a nonempty structure tree. Record producer and
   helper identities. Stop only the newly owned Word process.
2. Inspect and preview with the packaged CLI to establish actual source-bound
   selections, meaningful narrower geometry and one genuinely too-small refusal.
   Preserve refusals as evidence; do not weaken eligibility or normalize the input.
3. In the desktop, edit only the highlighted phrase. Exercise visible canvas
   resizing, Preview/Cancel, insufficient-area draft retention, Apply, Undo/Redo,
   save/reopen and unchanged re-edit. Run both exports sequentially against the
   same package. Use normal controls and owned native file dialogs.
4. Independently audit saved text/order, per-character fonts/sizes/colors, baseline
   and wrapping, highlight ownership/paint, font programs, untouched glyphs/pages,
   links, bookmarks and nonempty tagged structure where present. Pixel exclusions
   come only from source/request geometry, never from changed output paint.
   Negative controls must reject wrong style, baseline, highlight and outside ink.

The original quiz is not an input. No new dependencies, product changes, builds,
commits, releases or source-file replacements are part of an initially successful
acceptance pass. A demonstrated product defect requires a focused failing regression
before implementation and appropriate affected gates afterwards. Earlier memory
acceptance remains open and is not retested by this milestone. Unsupported arbitrary
PDFs, physical printing and commercial parity are not implied by these bounded cases.
