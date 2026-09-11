# Shaped text: native layout and PDF serialization

User direction: continue the roadmap after v0.9.0. Issue #14 explicitly requires a native/API serialization gate before supported characters are widened. Implement that gate first, then integrate the proven representation into explicitly shaped added text. #15 positioned existing runs and #16 wrapping remain separate.

## Design
Use maintained HarfRust, pinned with Unicode bidi/script/segmentation data, to shape exact immutable validated font bytes. Retain logical Unicode unchanged and explicit UTF-8/UTF-16 cluster ranges. Resolve bidi into visual runs and script-itemize each run without reversing the input string. Return bounded glyph IDs, advances, offsets, ink geometry, grapheme boundaries and optional outline paths in one versioned result. Missing glyphs, unsupported formats/control policies and resource excesses are explicit failures.

Allocate PDF CIDs independently of Unicode and map them to shaped glyph IDs. Preserve logical text with semantically appropriate ToUnicode/ActualText spans; never assign the whole cluster to every glyph without replacement semantics. Exercise actual PDFium create/set/save/reopen APIs or document a measured reason to emit explicit PDF operators. The exported native result must render like the shaped layout and extract logical Unicode through pinned PDFium and independent MuPDF. Record pypdf limitations explicitly, including RTL/marks. No production character range is widened merely because an experimental PDF renders.

Preserve legacy annotations unchanged. Production integration, once serialization succeeds, needs a versioned opt-in layout setting, same native outlines/positions for the browser, logical selection/search mapped to clusters, async font ownership, IME, undo/recovery and validated portable annotation reconstruction. Do not advertise complete script support before these checks pass.

## Acceptance for the serialization gate
Reproducible native shape/PDF evidence for ligatures on/off, composed/decomposed accents, two-axis combining marks, Arabic joining/mixed-direction digits, a redistributable Indic font, supplementary characters, missing glyphs, and zero-width/control handling. Save/reopen at all quarter turns, compare exact logical text in two readers, verify glyph positions and raster preservation, bounded hostile text/font/cluster input, pinned dependency/license provenance. Report any compatibility blockers before building UI on the representation.
