# Choice appearance text isolation

Use executing-plans inline and test-driven-development. This is a private helper
within the existing choice-editor design, not a publication endpoint. Preserve
all earlier evidence and source documents. No concurrent Cargo jobs or commits.

## Design decisions

Whole-widget regeneration erases artwork. Instead, parse a bounded saved appearance
and replace one top-level `/Tx BMC ... EMC` region. Require the prefix, text region
and suffix to be independently balanced graphics islands: all state changes occur
inside q/Q, text objects close locally, and no current path or pending clip crosses
an island boundary. Reject enclosing transforms, nested marked content, unknown
operators and incomplete state rather than guessing their effect. This narrow
first path admits the existing native combo/list probes and saved artwork around
them. More complex appearances remain available for ordinary viewing/filling.

Use strict lopdf content parsing. Keep the source prefix/suffix operators exactly;
serialization may normalize whitespace/numeric spelling. No-op raster comparison
is still required at the later native integration layer. BBox/Matrix, resource
graphs, font identity/coverage and original NeedAppearances remain caller-owned
proofs. The structural helper is not authority to change a PDF.

Generated text may reference fonts only through Tf, with a supplied name map.
Every used generated font must map to a nonempty original resource name; never
copy generated resources or drawing outside Tx. Font graph equality is a separate
prerequisite of the future caller. Empty text regions remain valid.

Bound raw input and conservative encoded output to256KiB, operations to16384,
state nesting to32. Validate operands, finite numbers and text/path lifetimes.
Charge combined replacement size before cloning operations or building output.

## Task1 — structural helper

Files: new src-tauri/src/form_choice_text.rs and form_choice_text_tests.rs;
modify lib.rs with private module/test wiring.
Interface: ChoiceTextBlock::parse(bytes), font_names(), transplant(generated,map).
All interfaces crate-private; no PDF byte publication or candidate registration.

- [x] Archive current lib baseline. Write focused failing tests for exact retained
  artwork operations, list highlight drawing, empty text, font remapping, missing
  mappings, escaped marker literals, ambiguous marks, leaked graphics/text/path
  state, malformed operands and bounded inputs/output.
- [x] Implement the parser/state admission and precharged replacement. Observe
  focused green, fix actual failures without weakening test intent.
- [x] Verify native generated combo/list probe admission and independent-reader
  no-op/changed text artwork preservation using synthetic PDFs. This confirms the
  helper's structural path only, not a complete choice edit or font proof.
- [x] Run library and existing choice/property regressions; record source/evidence
  hashes and outstanding native generator/publication/UI gates.

Review focus: q/Q does not save a current path or text-object nesting. A balanced
stack alone is insufficient. A clip pending until a path-ending instruction must
not cross boundaries. Unknown operators can change state. Font name remapping
must not touch original artwork, and mapped-name growth must be bounded before
constructing output. Whole-feature review remains one final review against the
recovery checkpoint, after all choice-editor layers are implemented.
