# Preserved widget font context

Existing-text edits and page replacement must preserve self-contained form
widgets even when AcroForm /DR is absent. The text-export desktop fixture from
PyMuPDF reproduces an unnecessary refusal before any page content is replaced.

Approaches: manufacturing global default fonts risks conflicting aliases and
changes unrelated fields; accepting every missing resource risks changing later
field rendering. Chosen implementation: validate the original independent font
contexts (field-inherited /DR, AcroForm /DR, and explicit normal appearance
resources) and retain their objects through the existing bounded appearance copy.
Do not synthesize fonts, rename fields, rewrite DA, or alter other-page resources.
Button/signature widgets do not require a text default font. Missing, malformed,
or page-dependent text font bindings remain refused until separately preserved.
This is an implementation choice under the user's autonomous-work instruction.

Native tests must prove replacement and occurrence text editing preserve field
pixels, field values, later filling, saved/reopened editability, unrelated pages,
and original source bytes. Exercise direct/indirect dictionaries, parent field
resources, conflicting page and local font names, malformed/missing bindings,
and repeated edits. The original PyMuPDF failure must pass desktop text edit,
save/reopen, and fill checks with independent rendered output verification.

Preserve existing limits and dirty worktree. No commits or release/parity claims.
Reference: PDFium chromium/8044 core/fpdfdoc/cpdf_bafontmap.cpp and
cpdf_formcontrol.cpp; validate behavior against the pinned local DLL.

Review correction: checking field-inherited DR is necessary but not sufficient.
When a normal AP lacks Resources, freezing the original page resources into it
must not change the BAFontMap lookup (normal AP first, then AcroForm DR). A newly
bound DA alias must match the original global font; an unspecified platform
fallback is not treated as proven equivalent. Independent inherited contexts
whose DA alias is absent from the page remain supported. Reject the conflicting
case without synthesizing fonts or changing DA. Validate recognized font kinds,
required composite descendants/encoding and Type3 structural geometry; this is
bounded structural validation, not a full embedded-program or glyph audit.
