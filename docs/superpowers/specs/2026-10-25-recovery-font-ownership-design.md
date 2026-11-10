# Recovery font ownership correction

The complete native audit exposed a word-spacing refusal test receiving an
unexpected error, followed by a poisoned test mutex. The isolated primary test
passes. Source tracing found that RecoveryStore::restore and legacy migration
remove every font added after a global registry snapshot, including unrelated
concurrent registrations. Confirm this with deterministic restore observers before
changing product behavior; do not weaken the spacing expectation or serialize
otherwise independent tests to hide the race.

## Required behavior

Recovery may clean up only fonts created by that recovery transaction and still
owned by it. Unrelated new fonts, existing fonts, and later claims of a font must
survive success, failure, fallback and legacy migration. Successfully restored
workspace fonts stay available. Unused transaction fonts are released, including
repeated imports of the same font through multiple sources. Existing count/byte
limits, source/image cleanup, protected recovery, migration ordering and retryable
checkpoint files remain unchanged.

## Approach

Use an explicit Arc-owned font import scope with a unique marker and a bounded
created-ID set. FontRegistry registers assets under its existing mutex and updates
the scope under a fixed registry-then-scope lock order. First creation records the
ID and scope marker. Reuse by the same scope preserves its marker. A registration
outside the scope, or a scope borrowing an externally owned asset, protects that
asset from earlier rollback. A scope never takes ownership of a preexisting font.
Cleanup checks both created-ID membership and current marker identity.

The scope crosses the internal OpenBytes worker request for legacy annotation font
imports. Normal open_bytes callers keep their existing interface through a wrapper;
the scoped implementation centralizes font ownership in the recovery scope. A lost
worker reply still discards its source/images; the final scope owner releases fonts.
Avoid holding any registry mutex while waiting on the native worker.

Restore returns a pending workspace plus its scope. Normal load adopts referenced
fonts and releases unreferenced ones only after successful workspace validation.
Legacy migration keeps the pending scope through save, closes temporary sources,
then releases only that scope. Failed generations drop their scope before fallback.
No before/after registry-ID sweeping remains in recovery or migration.

Alternatives rejected: locking the entire recovery operation risks deadlock with
queued native imports; serializing tests conceals the product race; removing cleanup
entirely retains unused recovery fonts. Explicit ownership preserves both safety
and bounded resource cleanup.

## Verification

A test-only thread-local restore observer creates deterministic interleavings after
font/source import and before final workspace validation. Cover unrelated font
registration on success/failure, same-font adoption before failure, legacy migration,
repeated scope acquisition, rollback and reference adoption. Existing recovery-font,
image, protected and annotation-migration tests stay intact. Repeat the previously
failing parallel text-state suite with the original semantic assertion and improved
error diagnostics. Run appropriate broad gates and one final scoped review, followed
by a fresh Windows build and actual recovery/edit/save verification if product changes.
The original broad run finishes without source mutation; retain all failures.
