# Folio 0.26 development portable integration

Intent: integrate reviewed desktop/CLI/MCP improvements and new text removal into an optimized, reproducible development package; preserve document fidelity and prior packages. The verified original-quiz desktop checkpoint is artifacts/quiz-removal-desktop-20261030/checkpoint.json. Overall Adobe/Foxit goal remains active.

Coordinator choice under standing autonomous authorization: use a new0.26.0-dev.1 version, existing build-portable.ps1 and exact-source acceptance. Overwriting0.25 would destroy evidence; claiming the debug candidate is a portable package would not exercise release behavior. Preserve all prior artifacts and original documents. No external publish, commit/push or user app restart.

Freeze source/version/renderer and native test inputs before broad gates. Run full default Cargo suite serial/no-fail-fast, explicit originalWord/producer cases, full frontend suite, TypeScript/Vite and offline asset checks. Record ignored tests separately; no timeout relaxation or failed-evidence deletion. One Cargo process, one observable live handle, no restart solely on observation timeout.

Existing packaging installs only app/CLI, bounded resource/renderer/OCR assets, notices and manifests. Verify matching versions, source fingerprints, binary/resource hashes, ZIP entry safety/completeness, extracted-package execution, CLI/MCP and desktop saved-output fidelity, font/producer/OCR/recovery workflows and private-source preservation. Private quiz and derivatives never enter package/public evidence. Use prior acceptance scripts as templates, not authority to bypass guards tied to older source.

The outstanding sustained real Word Preview/Cancel memory-growth failure remains an open release-quality gate (natural-preview-loading-20261029/checkpoint.json). A new development package is not a resolution, commercial-parity claim or final release approval. Do not relax its original128MiB20-cycle budget or use forcedGC/blocked tracing/artificial replacement workloads. Resource diagnosis/correction remains necessary.

No feature expansion during fixed-source validation. Any discovered product failure is diagnosed first, reproduced narrowly, corrected with affected regression, and followed by a new source-bound gate; prior failures remain. Final scoped packaging/evidence review before calling package integration complete. Clear checkpoint/continuation files own process identities and every pending gate.
