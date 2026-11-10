# Paragraph inspection implementation plan

Spec: docs/superpowers/specs/2026-10-27-paragraph-inspection-design.md
Execution: inline under continuing authorization; one fresh final scoped review.
Global constraints: existing dirty linked worktree, no commits/push/cleanup, one
Cargo process, locked/offline shared target, preserve originals and old evidence.

## Task 1: Native-bound CLI inspection

Add actual subprocess tests in tests/paragraph_inspection.rs. First observe unknown
command failure. Resolve a seed with existing source/page hashing, permit native
paragraph eligibility independently of single-run supported, call the complete
source styled suggestion, bound JSON and hash paragraph metadata. Add strict CLI
verb and Request dispatch. Verify stability, exact mixed native output, page/source
binding, permissions, unsupported cases and no filesystem mutation.
Expected: new tests fail before implementation, pass after.

## Task 2: MCP integration

Add a subprocess regression to tests/mcp.rs for discovery, schema, immutable source
resolution and source data. Observe failure before protocol/worker changes. Add
strict parser/tool and private disposable worker verb. Reuse lifecycle handling.
Expected: new regression plus existing MCP suite pass with eleven tools.

## Task 3: Review and packaged evidence

Update docs/automation.md. Run full lib, CLI, MCP, text inspection/automation and
paragraph inspection tests. Snapshot changed scope with hashes against baseline;
one independent final review, correct important findings with RED/GREEN evidence.
Build portable CLI with matching fingerprints. Official SDK legacy/modern and
standalone CLI inspect generated Word PDFs outside checkout, no explicit renderer,
source hashes and cleanup checked. Preserve all logs; checkpoint/next step records
paragraph preview remains next. No release or commercial parity claim.

Review focus: byte cap including escaping/envelope, stable but source-bound IDs,
mixed style preservation, unsupported run vs paragraph admission, privacy/paths,
strict CLI/MCP schema, worker cleanup, protected sources, no accidental weakening
of single-run replacement. Scope excludes new native layout/publication logic.
