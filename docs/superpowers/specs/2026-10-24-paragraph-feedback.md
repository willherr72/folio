# Visible paragraph fitting and feedback

Current Windows acceptance shows the Fit action and native errors below many
formatting fields. Preview/Apply stay visible, but the action needed after overflow
and the failure explaining it can be offscreen. Improve this workflow within the
existing styled paragraph dialog; preserve native fitting and publication gates.

Move Fit area to text to the fixed footer beside Cancel/Preview/Apply. Add concise
guidance about added text and unchanged fonts/sizes. Move fitting notices, pending
status, font/native errors and numeric/bounds guidance into a visible feedback
region above those actions. Keep role alert/status semantics and complete errors.
Long feedback can scroll in a bounded region; actions must remain reachable.
Selection adjustment does not show the fitting action/help. Existing loading and
piece-selection guidance stays with its controls.

Use wrapping flex/grid rules for narrow windows. Do not shrink the font, add
automatic fitting, classify arbitrary backend errors by text, or bypass preview.
The existing fit generation/font/history ownership remains unchanged.

Acceptance: existing dialog/adapter tests and production build; authored Windows
overflow -> visible error/Fit -> fit -> visible notice -> preview/apply/save/reopen;
footer visibility with body scrolled, narrow viewport and keyboard access; previous
fitting output comparisons remain applicable, with new Windows exports independently
verified. Scoped review and a new executable/evidence root preserve old checkpoints.

User authorized autonomous iterations without routine design approval. This is a
coordinator-chosen ergonomic improvement, not a change to PDF support claims.
