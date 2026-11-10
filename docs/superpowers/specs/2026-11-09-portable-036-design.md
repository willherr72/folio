# Folio 0.36 local development integration

Integrate the qualified typed-text-to-paragraph handoff into the combined desktop/CLI/MCP Windows portable. User standing autonomy applies: implement and qualify without routine permission requests. Preserve accepted 0.35, all originals, failed attempts, sealed evidence and the current launcher until qualification is complete.

## Approach

Use the existing portable builder and the accepted 0.35 extraction/runtime workflows. A versioned 0.36.0-dev.1 preserves accepted outputs and gives the launcher a concrete identity. An unversioned replacement would weaken provenance; a separate GUI-only candidate would not deliver the automation/OCR resources users need.

Change only the five existing version metadata files from 0.35.0-dev.1 to 0.36.0-dev.1: package.json, package-lock.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock and src-tauri/tauri.conf.json. Preserve LF Cargo metadata and all dependency/renderer bytes. Update release documentation before capturing distribution inputs. No new product behavior, dependency, PDFium patch or security-policy change belongs in this packaging milestone.

## Source authority and qualification

The implementation baseline is artifacts/text-paragraph-draft-20261109, checkpoint SHA256 aa6ecf32cccbc8f621ae2d6b67a7a8d95eb33a6e68c9964dc4c9badc6eb63d86 and final source fingerprint 0106cae20efd08520d52ee98f1465f3b45bec47e3d5893e4fa421adedc027a05. Its full frozen source snapshot, final frontend gate and Windows original-quiz/shared-form/font results are preserved authorities.

Native authority is that milestone's fresh native-full-1: 1,159 optimized integration/binary passes plus 1,496 opt-level=0 library correctness passes, 32 skips. That run binds fingerprint 39d3da0de9227dce7b04a5d5b5a74bda199f6e8008e787d2774401c348ef6b2f; native-reuse-1 proves the later two UI-only review corrections leading to the final baseline. Reuse only under exact Rust/dependency/renderer/native/shared-fixture qualification plus exact five-file version substitutions. Do not relabel these native results as fresh package results or substitute accepted 0.35's older 2,650-pass authority.

Preserve helper provenance from accepted 0.35 checkpoint 3c46aeb8fc5fda962b153aad50fc48b319fdcb212c2eee32621492e878ae64f2. Copy helpers and immutable authored fixtures, never successful pass receipts. Adopt its corrected paragraph readiness and strict native Save helper. Qualifier controls must reject nonversion metadata changes, renderer/native changes, changed membership/fixtures and unbound evidence. Historical checks read frozen artifacts and remain independent of future worktree edits.

## Required acceptance

- Fresh complete frontend tests with two workers and the default 5000ms timeout; TypeScript/Vite, optimized GUI/CLI builds and all pre/post source/input guards.
- Complete ZIP/extracted membership, source/renderer provenance, all offline OCR assets and dependency-license records. Extract outside the checkout and use those extracted executables/resources for acceptance.
- Established 37-step CLI/MCP workflows, desktop text/paragraph/OCR, editable saved-output audits, combined removal and missing-original recovery. Bind every required saved output to its actual independent audit.
- Existing sustained Preview/Cancel ownership and 105-edit/100-Undo/100-Redo resource/history workflows. Keep observed resource scopes and original baseline identities explicit; do not claim new peak/low-memory or broad performance certification.
- Re-run the handoff cases on the extracted package: original uploaded quiz (private supplement), generated shared form and explicit chosen-font transfer. Preserve local and workspace Undo/Redo, Preview/Apply, save/reopen, original sources, unaffected glyphs/pixels and selected-piece font containment. Keep private inputs/results outside the ZIP and packaging review.
- Identity-bound native/browser cleanup includes failed attempts. Preserve failure evidence and correct demonstrated helper assumptions without relaxing source-authorized masks or formatting checks.

Serialize compilers, suites, desktop sessions and resource observers. Use the existing isolated worktree and D:/Folio-build-cache/t3-8962a327 with one build job. Do not repeat unchanged successful checks unless a new failure, source change or unresolved concern justifies it.

One builder check precedes the plan; one scoped packaging review follows concrete extracted acceptance. Fix consequential findings inline, freeze the package and evidence, and promote the checkout launcher only after qualification. No automatic app launch, commit, push, public release or deletion of prior packages/originals. The broader goal remains active. Larger-document native editing latency is the next investigation after integration; printing, accessibility, signatures, broad producer/script compatibility and low-memory hardware remain open.
