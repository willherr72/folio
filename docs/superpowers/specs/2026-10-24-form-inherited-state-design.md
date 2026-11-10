# Nested form text-state diagnosis

The preceding editing milestone found a disagreement between PDFium and MuPDF
when a form uses leading inherited from its caller. Diagnose the bundled DLL
directly before changing document semantics or replacing the rendering dependency.

Use synthetic, original PDFs with equivalent inherited and explicitly local text
states. Compare each pair within each renderer, never pixels across renderers.
Exercise leading, rise, horizontal scale, character/word spacing, font size, color,
stroke mode, combined state, one/two form levels and q/Q restoration. Record DLL,
script and input hashes. Default probe success requires all pairs to agree; an
explicit diagnostic mode may assert the known defect matrix instead.

Pin Folio's existing refusal for inherited rise/scale as well as the already
covered leading discrepancy. Prove that local overrides and unaffected inherited
properties still permit real edits with unchanged shared occurrences and source
bytes. Do not relax source/flat equivalence or alter the source PDF to match a
renderer bug. No product binary change is part of this diagnostic milestone.

Preferred follow-up is a separately built, source-patched PDFium which copies the
three missing scalar text-state values into child form state. Copying all parser
state would also inherit text matrices/positions and is not justified. Global PDF
rewrites or changing Folio's expected geometry to match the renderer would hide
the discrepancy. A renderer change needs pinned source/toolchain/patch provenance,
the direct probe in normal mode, native and independent-reader regressions, source
preservation, and Windows editing/export/printing acceptance before adoption.

Keep the current working binary available. No external issue posting, commit,
release, or parity claim is authorized by completing this diagnosis.
