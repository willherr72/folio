# Portable 0.24 integration

Produce a local Windows desktop/CLI development package, version **0.24.0-dev.1**,
containing the accepted standard-font Latin and embedded CMap improvements. Reuse
the existing combined packaging workflow, pinned renderer, offline OCR assets,
licenses and guides. Preserve 0.23, all originals, prior artifacts and user apps.
This is local packaging, not publication or commercial-parity certification.

The only executable-input delta from the accepted embedded-CMap checkpoint is
the five root version manifests. Preserve dependency versions and LF Cargo.toml.
Capture exact pre-edit bytes and hashes; prove normalized version-only equivalence
and unchanged native test inputs. README and changelog describe supported Latin
editing and the remaining preservation limits without promising general coverage.

Reuse the completed default native gate (1,613 passed, 14 ignored) and explicit
original Word gate (2 passed) as evidence on the pre-version functional source.
Do not describe these as tests on 0.24 metadata. Run fresh frontend, package and
launcher checks, optimized desktop/CLI builds, archive verification, and runtime
acceptance on the verified extracted package outside the checkout. A functional
change or failure requires diagnosis and appropriate new native verification.

Acceptance covers default renderer discovery, CLI/MCP text/paragraph/form and
nested editing, explicit fonts, consecutive saves, protected/ordinary unchanged
saves, source-bound Latin/embedded editing, equivalent historical/current font
resources, desktop text/paragraph and offline OCR. Independently inspect saved
text, fonts and pixels. Repeat the private quiz's two insertions in CLI and desktop
without distributing its source, outputs or harness; publish sanitized evidence.
Retain the inherited ReportLab unused-Helvetica resource loss as a limitation,
not a passing strict-resource audit. Preserve failed attempts.

Require final package/archive/extracted hashes, refusal to overwrite an existing
package, original and predecessor integrity, one frozen scoped review and a final
checkpoint. No commits, pushes, cleanup, publication or user-app restarts. One
Cargo at a time, locked/offline, jobs1; never mutate build inputs during gates.
