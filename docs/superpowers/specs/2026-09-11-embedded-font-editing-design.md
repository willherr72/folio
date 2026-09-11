# Embedded-font editing and font dialog

User authorization: improve More fonts to match Print and work on issues #12/#13. #12 is an implementation; #13 is a staged investigation requiring concrete follow-up issues, not general word-processor behavior.

## Font dialog
Match Modal/Print header, padded form area, border/control tokens, footer alignment, dark theme and responsive height. Search installed fonts, retain local import and clear unsupported reasons. Selection loads one actual-font preview; only explicit Apply commits. Cancel and switching preview release unowned fonts, preserving active documents/history and late-load cleanup. Keyboard/focus semantics remain accessible. Expose optional title/description/current selection for substitute-font use.

## Existing embedded fonts (#12)
Keep immutable page copies and original protection. Support horizontal single-line Latin runs whose embedded TrueType program and PDF encoding/Unicode/glyph mappings can be positively verified. Reuse existing resources only for representable replacement characters. Reject ambiguous mappings, ligatures, missing glyphs and unsupported layouts precisely. Prove no-op text/geometry/pixel stability and post-edit save/reopen/extraction/neighbor preservation. Do not relax preflight for unrelated unsafe page features.

Offer an explicit installed/imported substitute for otherwise valid runs when the original subset lacks requested glyphs; load and preview the chosen font using #11 resources, never silently substitute. Preserve size, color and baseline; page limits and collision checks still apply. Font resource and Unicode mappings must agree after save/reopen and recovery. Existing standard-font editing stays compatible. Add optional fontId to replacement command rather than changing saved overlay metadata.

## Complex text (#13)
Investigate shaping, bidi, glyph clusters, positionable PDF font APIs and text extraction using primary documentation and reproducible local probes. Cover combining accents, Latin ligatures, Arabic mixed direction and one Indic sample. Document viable native/font/preview integration, limitations of PDFium mappings, and stages for positioned runs, nested forms and explicit text areas. Create concrete tracked implementation issues; do not label unsupported scripts implemented.

## Validation/release
Targeted red/green tests, full frontend/native checks, real browser light/dark keyboard and preview workflow, production native dialogs, independent PDF readers. Preserve source immutability, undo/redo, recovery, bounded font ownership and all earlier safety tests. Ship verified artifacts and record exact implemented scope in central issues.
