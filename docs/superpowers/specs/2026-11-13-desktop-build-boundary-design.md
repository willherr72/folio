# Desktop asset build boundary

Folio's normal optimized GUI library currently depends on 17 generated frontend
and OCR assets because `tauri::generate_context!()` is expanded inside `app.rs`.
Rebuilding the large PDF engine for frontend iterations delays useful work.

The chosen small refactor moves that macro into the GUI binary entry point and
passes its `tauri::Context<tauri::Wry>` to `folio_engine::run`. The builder,
commands, setup, error dialog, resources, version, feature flags and renderer
remain unchanged. This is an internal startup interface; the only caller is
`src-tauri/src/main.rs`. No dependency or crate split is needed.

Alternatives considered: changing optimization would weaken normal-build
comparability; splitting the engine into a separate package adds unnecessary
dependency and packaging work. Leaving the boundary intact preserves long
frontend rebuilds. Moving context construction is the smallest testable option.

Success requires actual Cargo evidence: frontend assets belong to the GUI binary
dependency list, disappear from the engine library dependency list, and a
controlled generated-CSS change rebuilds the binary while retaining the exact
library bytes and modification time. Compare elapsed build time only under the
recorded same-host warm-cache conditions. Never infer application speed from this.

Use normal optimized custom-protocol builds and one Cargo job. Preserve original
generated CSS bytes and restore them before final candidate qualification. Capture
every generated asset hash and source/resource fingerprint. Verify actual GUI
startup, native PDF editing/save/reopen and offline OCR using existing public QA
fixtures; default CLI must still compile and open the saved result.

The selected 0.49 portable, launcher, earlier packages, private quiz, original
failures and dirty worktree remain untouched. Do not commit, delete evidence,
publish or claim commercial parity. User's standing autonomous instruction
authorizes this implementation choice without another approval round.
