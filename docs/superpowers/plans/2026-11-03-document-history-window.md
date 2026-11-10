# Document history window implementation

Spec: `docs/superpowers/specs/2026-11-03-document-history-window-design.md`.

1. Add failing history rollover, branch, no-op, dirty-baseline and source-retention regressions. Bound normal commits to 100 actions and route both completed-drag paths through that same commit. Expected: new regressions fail before and pass after the change; retained documents stay exact.
2. Verify App gesture completion and tab-switch completion at capacity. Run the full frontend suite with test-input hashes and build a new development candidate with unchanged native inputs verified. Expected: green tests/build and stable source fingerprints.
3. Run 105 real paragraph edits, Undo to the fifth candidate, Redo to the last, save and independently inspect the PDF, and prove the first five native sources close before tab close. Review the integrated delta once, correct Important findings with failing regressions, and record evidence and limitations.

Interfaces: model commit owns retention; App gestures use commit; workspace source cleanup consumes retained history plus recovery leases. No byte-size accounting or new native command.

Review focus: off-by-one endpoints, no-op Redo preservation, saved-digest independence, both drag completion paths, tab/recovery source ownership, stale asynchronous cleanup, and desktop oracle provenance. Preserve inherited dirty changes and all historical evidence; no commits or release publication in this milestone.
