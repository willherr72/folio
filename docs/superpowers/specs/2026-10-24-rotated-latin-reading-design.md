# Rotated Latin reading, copying and search

Everyday reading must preserve words split across PDF text-show objects, including
accented Latin and single-letter pieces. The previous grouped-editing milestone
exposed that the shared reader guard still requires two ASCII characters per
object. Its fallback can retain PDFium's rotated object order and generated gaps.

Coordinator choice under standing autonomy: extend the bounded private-page
rotation correction, using source-axis native origins and a conservative Latin
scalar subset. All objects must still have finite positive axis-aligned text
matrices and positive sizes, and each object's origins must be finite, on one
baseline and nondecreasing. A one-character object gains direction evidence from
other text on the same exact baseline: their original object-order origin spans
must be nondecreasing with positive overall advance. Isolated single-character
baselines retain fallback for now. Multi-character objects retain their positive
internal advance requirement. This preserves prior ambiguous negative/vertical/
counterrotated/non-Latin/combining-script fallback. Object/character bounds stay
64/4096; forms and unknown objects still use the raw native reader.

Keep original PDF bytes, rendering, raw extract_text, crop/rotation mapping,
permissions, search limits, OCR transport and frontend contracts unchanged.
Reading correction remains transient; no source mutation or exported rewrite is
needed. Use a small private reader_order module for eligibility, with engine.rs
retaining copy lifetime and character transport. No renderer dependency change.

Approaches considered: widening Unicode alone leaves single-letter splits broken.
Unconditionally stripping rotation regresses known counterrotation and negative
advance controls. General layout reconstruction spans scripts, columns and forms
and needs its own evidence. The selected approach addresses ordinary Latin words
using current native geometry while retaining explicit ambiguous-layout fallback.

Acceptance: reproduce accented/singleton copy and search failures; verify seven
embedded split boundaries at four rotations, ASCII singletons, multiple lines,
nonzero/inherited crop/rotation, character boxes, exact before/after source pixels
and bytes, save/reopen, no-copy permissions with location-only search. Exercise
negative/stationary/reversed singleton sequences, unsupported scripts and mixed
orientation fallback. Check actual Windows mouse selection/Control+C and Find on
rotated fixtures, including a single-letter cut. One fresh scoped final review,
relevant regressions and a runnable fingerprinted checkpoint. No parity claim.
