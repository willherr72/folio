# Dependent text re-edit implementation plan

Use executing-plans inline; user opted out of routine confirmations and per-task
agents. One final scoped review. Spec: ../specs/2026-10-28-dependent-text-reedit-design.md.

1. [x] Preserve baseline and reproduce saved no-font growth failure through native
   complete-source editing with tagged context, Tc/Tw and dependent following text.
2. [x] Add original-font private plan using verified advances; bind it in isolated
   and outer source publication. Retain existing route selection and all guards.
3. [x] Extend controls for spacing/quotes, growth/shrink/repeated saves, positions,
   source-font/resource retention and refusals. Run focused and affected regression.
4. [x] Build a fresh local candidate; independent saved-output and desktop/automation
   acceptance as appropriate; one final frozen review, resolve findings, checkpoint.

Only one Cargo process. Locked/offline, jobs1, serial tests, shared configured target.
Never mutate product/test inputs during gates. Preserve all failed logs and hashes.

Current evidence: full-native-1 completed before the final spacing correction with
1,566 passes, zero failures, 14 ignored. Corrected-regression-2 completed on final
source with 671 passes, zero failures, six ignored. Explicit original Word tests
passed separately (two tests, inputs preserved). Large combined double-quote
spacing retains its confirmed pre-existing context limit; see quote-limit-probe-2.
Fresh candidate build, desktop/CLI/MCP acceptance, independent saved-output audits,
and 12 unchanged-file controls passed. The sole final review approved the scoped
implementation without actionable defects. Coverage limits remain recorded in
artifacts/dependent-text-reedit-20261028/review/disposition.json; the verified
checkpoint is artifacts/dependent-text-reedit-20261028/checkpoint-final.json.
