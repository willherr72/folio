# Protected workflow implementation plan

> Inline execution: superpowers:executing-plans. User autonomy authorizes design and
> implementation decisions; no commits, publication or recurring approval requests.

Spec: ../specs/2026-10-22-protected-workflow-design.md
Ledger: artifacts/protected-workflow-progress.md (retain across continuation turns).
Baseline: branch t3/resume-pdf-editor, HEAD ac27b5a, extensive authorized uncommitted
prior cycles. Native286 verified, frontend395 unchanged, cycle8b desktop acceptance.

## Task 1: memory composition

Interfaces: bounded writer/shared PDF load-save helper; AcroForm preserve(Document),
portable finalize(Document), bookmarks write_document(Document). Worker compose_pdf
returns final plaintext Vec before publication. prepare_print creates a raw-annotation
source directly from bytes; composed preview does not export/reopen a file.

- [x] RED: print preparation's source identity must not be an existing PDF file;
      preserve current order/rotation/annotation pixel checks. Bounded writer tests
      exercise exact boundary, overflow, sticky refusal and no returned partial bytes.
- [x] Replace path intermediates with shared Document composition; cap both PDFium
      and lopdf writes before growing buffers past the limit. Publication remains atomic.
- [x] GREEN: printing unit pipeline and focused form/bookmark/persistence/preview suites.
      Expected: all pass, including existing field/action and annotation fidelity.
- [x] Add/verify mixed-composition regression and source hash/independent-reader checks.

## Task 2: authenticated state and native policy

Interfaces: opaque private context serialization, source-origin/provenance and nonsecret
rights DTO; typed password errors and pending-open challenge; native authorization.

- [x] RED/GREEN context round-trip under DPAPI, invalid/absent context, revisions2–6,
      permissions and both retained credentials. Update pinned vendor provenance if needed.
- [x] RED/GREEN password/empty-password opening, source replacement inheritance and
      recovery binding. No password persists; no frontend key serialization.
- [x] RED/GREEN copy/edit/annotation/form/assembly/print policy, forged export plans,
      compatible/incompatible source combinations and original overwrite protection.
- [x] Route final protected output through encryption before any temporary publication.
      Verify export failure leaves destinations unchanged and no plaintext staging.

## Task 3: password and permission UX

- [x] Add masked challenge/retry/cancel and owner-unlock UI using native opaque identity.
- [x] Wire permission-aware controls and explain restrictions; preserve ordinary flows.
- [x] Test stale results, cancellation/closing, source cleanup, undo/recovery and passwords
      absent from persistence. Expected: frontend suite and production build pass.

## Task 4: final lifecycle acceptance

- [x] Independent pypdf/MuPDF authentication/rights/text/pixel verification across formats.
- [x] Full native/frontend and embedded desktop builds; isolated Windows protected open,
      wrong-password/cancel, edit, encrypted save/reopen, crash/recover and print checks.
- [x] One fresh review of this plan/spec/diff and evidence, most capable available model;
      implement one material fix pass with reproducing RED/GREEN tests. No second review.
- [x] Update README, quality ledger, limitations and next parity priorities; keep broad
      goal active unless the actual full parity checklist is satisfied.

Review focus: overlooked empty-password inputs, absent/corrupt recovery contexts,
source replacement/undo privilege escalation, hand-crafted export/preview plans,
page assembly versus content/annotation rights, copy/search transport, print quality,
plaintext intermediate files, incompatible context combination, owner/user credentials,
custom fonts/annotations/forms/bookmarks fidelity and bounded failure publication.
