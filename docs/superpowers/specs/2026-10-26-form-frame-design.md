# Existing form field borders and backgrounds

## Intent and authorization

Give an existing text field, checkbox, radio widget, dropdown or list a useful border/background editor. The user's standing instruction is autonomous implementation and verification. The choices below are engineering decisions, not newly elicited preferences. Preserve answers, captions, custom artwork and linked-field behavior.

The prerequisite is complete: `artifacts/form-fill-fidelity-20261026/checkpoint.json` binds the corrected form-fill implementation, 594 passing native tests, build and desktop/audit evidence. The older `artifacts/form-frame-design-20261026/investigation.md` and its trusted-only geometry spike remain historical evidence, not production code or approval.

## Approach

Change the selected widget's verified frame paint in its own appearance streams. Preserve original non-frame instructions and their state effects, appearance dictionaries, resources, bounding boxes and matrices. Update the corresponding local `/MK` and `/BS` properties. Never mutate an appearance shared by other widgets in place.

Whole native appearance regeneration was rejected because it loses artwork; page overlays were rejected because they do not change the field's own appearance or later filling behavior. Paint replacement requires more admission work, but allows an honest refusal when the frame cannot be separated from content.

## User behavior

The existing field actions gain **Border and background**. Show inspected width in physical points, border style, border color and background. Each color has Original, Transparent and custom RGB modes. Preserve Gray/RGB/CMYK operators exactly when unchanged. Width accepts finite 0–32 pt, including fractional widths; zero means no border, never PDF hairline. Reject widths that consume the entire verified shape. Default controls leave settings untouched, and Preview requires a change.

Support solid, dashed and underline rectangular frames and solid/dashed circular radio frames. Beveled/inset rectangles are part of this milestone, with explicit multi-edge geometry proof and generation. Circular bevel/inset/underline and ambiguous custom shapes are not silently converted: report the unsupported style. Pressed/rollover states must all pass admission. Preserve an established pressed-state shade relationship only when proved from its original palette; otherwise refuse color changes involving ambiguous state palettes.

Preview renders an immutable candidate on the selected page. Apply uses the existing complete-source page replacement transaction and Undo/Redo/recovery. Cancel, stale requests, tab changes, unmount and superseded candidates release exactly the candidate's owning adapter. Never release a borrowed source. Disable Apply until the current candidate has rendered successfully.

The edit belongs to the selected widget and page occurrence. Other widgets of a shared field retain their frames; their logical answer remains shared. Later filling, save/reopen, copying pages and recovery must preserve the edited frame and original artwork.

## Native admission and preservation

- Load through bounded `acroforms::authoring_document`, retain source/output limits of 512 MiB and existing form graph limits of 4000 nodes/placements, 100000 values and depth 64.
- Limit each decoded appearance to 4 MiB and 262144 instructions. Bound lexical parsing before decoding with the existing appearance preflight. Bound aggregate retained/generated programs and graph copying to 64 MiB; charge aliases, nested dependencies and repeated states before allocating. No dependency updates.
- Validate source kind/permissions/flags, expected value/bounds and a bounded fingerprint of the original frame settings and appearance. Revalidate after occurrence isolation. Respect source edit-content and annotation permissions.
- `/BS` overrides legacy `/Border`; reject malformed widths, arrays, styles and unsupported rounded corners rather than guessing. Unchanged dash arrays and color spaces retain original values. Physical units use page UserUnit and the verified appearance-to-widget scale; retain matrices and rotations.
- For an explicit border setting change, publish a complete canonical `/BS` and remove the validated obsolete `/Border`. Inspection uses `/BS` precedence but still validates a present legacy entry. This avoids carrying contradictory settings into later native filling; background-only edits leave the existing border metadata intact. Upstream PDFium has differed between [generation](https://pdfium.googlesource.com/pdfium/+/f96e865f15b1e8ea01bdc759aafc034b3ffed191/core/fpdfdoc/cpdf_generateap.cpp) and [interactive annotation access](https://pdfium.googlesource.com/pdfium/+/b3466cc54c3de0cdd61618d6326ceb7f95249c75/fpdfsdk/cpdfsdk_baannot.cpp); these references guide the regression, not proof about the pinned binary. Saved-appearance and later-fill tests remain required.
- Walk graphics state and path lifetime correctly: PDF paths are not saved by q/Q. Require a unique frame candidate in the expected shape and painting order, finite geometry, known relevant colors, line/dash state, transforms, and clipping. Unknown ExtGState, pattern colors, transformed/clipped ambiguous frame paths or overlapping candidates refuse. Text and decorative paint cannot become a frame merely because their color matches.
- Rectangular candidates are full-bounds backgrounds, matching uniform rectangle rings, inset rectangle strokes, or verified underline/bevel geometry. Radio candidates are a unique canonical circular stroke before answer/caption paint; infer its actual geometry, including labeled radio circles that occupy only part of the widget. Do not use the whole labeled-widget rectangle as a circle.
- Support absence only when established: a missing background can be inserted behind the verified frame; absent borders need a proved rectangular widget geometry and unambiguous metadata. Missing appearances or global NeedAppearances require a separately verified baseline or an explicit refusal. Do not clear global regeneration flags opportunistically.
- Replace only classified painting operators with `n q <new paint> Q`; leave their prior state-setting operators intact. Account for pending clips: do not replace a clip-bearing paint with geometry that changes the clip. New background is behind the existing frame/answer. Unchanged color/style/geometry remains exact where practical, and an unchanged request returns original bytes after validation.
- All N/R/D stream/state dictionaries are processed atomically with copy-on-write ownership. Do not import unrelated native document graphs. Do not trust a Folio marker as evidence of geometry or ownership.
- Keep original field names, values/defaults, options, actions, flags, placement and resource bindings. Bounds outside the widget and non-frame foreground paint remain invariant. Stage all mutations before registration; refusal leaves originals untouched.

## Later edits

Frame width edits preserve the current answer's placement. A later actual answer edit may use the new padding. A same-value fill remains a no-op. Test subsequent text appearance changes and resizing: if existing unchanged-regeneration proofs cannot support a changed frame, report that limitation explicitly rather than weakening artwork protection. Transparent border/background must remain transparent through later fills.

## Evidence and completion

Unit tests prove lexical budgets, state/path admission, metadata precedence, exact retained operations and styles. Public native tests prove selected occurrence and linked-widget isolation, permissions/stale identity, all states, actual retained raster regions, fractional widths, four rotations and UserUnit 1.7, fill/clear/refill, save/reopen and refusal identity. UI tests prove draft validation and candidate ownership. One final scoped independent source review follows implementation; fixes get reproductions and regression coverage.

Completion additionally requires the relevant native suites, full frontend suite/build, fresh Windows executable and desktop Preview/Apply/Cancel/Undo/Redo/save/reopen/recovery on text, choice, checkbox and labeled radio cases. Independent pypdf/MuPDF checks verify values, frame geometry/colors and invariant artwork. Preserve old executables, source PDFs, dirty changes and failed logs. No commits, pushes, release or parity claim.
