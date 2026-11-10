# Explicit Automation Paragraph Fonts Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Autonomous
> execution is authorized; one final scoped review follows acceptance.

**Goal:** Add reproducible explicit paragraph font choices to CLI and MCP.
**Architecture:** Bounded immutable font inputs feed the existing native style
overrides. CLI supplies files; MCP retains launch-authorized bytes and gives each
worker private copies of only referenced fonts. Native proof remains mandatory.
**Tech Stack:** Rust, existing FontAsset/parser, CLI/MCP JSON, Python acceptance.
**Spec:** `docs/superpowers/specs/2026-10-28-automation-paragraph-fonts-design.md`

## Global constraints

16 paths,16MiB each,64MiB aggregate including duplicates;64 unique used source
style overrides;512 UTF-8 bytes/name;32KiB edit and64KiB manifest existing limits.
No implicit substitution, arbitrary MCP tool paths or native proof relaxation.
One Cargo, jobs1, locked/offline, serial tests; shared target directory. Preserve
originals/packages/failed evidence. No commit, cleanup, release or parity claim.

## Review focus

1. Explicit unchanged-text substitution must not take byte-identity shortcut (T1).
2. Registered-but-not-authorized font IDs must fail, including reused engines (T1).
3. Font file mutation after MCP launch must not change effective identity (T2).
4. Failed/cancelled workers must release only owned temporary resources (T2).
5. Saved substituted paragraphs must re-edit without launch font files (T3).

### Task 1: CLI font catalog and native paragraph overrides

**Files:** Create `src-tauri/src/automation/font_inputs.rs`; modify
`automation.rs`, `automation/paragraph_preview.rs`, `cli.rs`; create
`tests/automation_fonts.rs`; extend `tests/paragraph_automation.rs` and
`docs/automation.md`.

**Interfaces:** `FontInputs::load(&[PathBuf])->EngineResult<Self>`, `catalog()->Value`,
`get(&str)->EngineResult<Arc<FontAsset>>`, `install(&Arc<FontRegistry>)->EngineResult<FontInstall>`.
FontInstall rolls back only its new registration tokens on Drop. `execute_with_fonts`
is crate-internal and accepts `&FontInputs`; existing execute delegates with empty
inputs. `FontOverride {style_id:usize,font_id:String}` uses camelCase, denies unknown
fields. ParagraphEdit and ParagraphMetadata add defaulted `font_overrides`; metadata
also records used catalog entries. CLI adds a separate fonts action, and invocation
font paths only for preview-paragraph.

- [x] Add CLI black-box RED tests for fonts catalog/hash and successful explicit
  override preview/save; unchanged text must produce changed font/output bytes.
  Existing CLI should refuse the new command/option. Preserve RED logs.
- [x] Implement bounded FontInputs with existing FontAsset parsing, compact typed
  catalog and registration guard. Test invalid/empty/directory/oversized/count/
  aggregate/duplicate-byte charging, mutation immunity and rollback ownership.
- [x] Add strict override validation and resolve every style/font against source
  and explicit inputs before fit/preview. Set source_style_id=style_id; retain
  size/color. Include overrides/used font metadata, validate it on bundle reading,
  and bypass no-op identity only when overrides are empty.
- [x] Add fonts action and repeated --font parsing with native OsString paths;
  preserve normal command behavior and exit/status JSON. Fonts catalog needs no
  engine. Document catalog/preview commands and unsupported cases.
- [x] Run library and automation_fonts/paragraph_automation/text_automation/CLI
  tests serially. Expected GREEN, source/test fingerprints unchanged. Verify
  unknown/non-used style, duplicate override, wrong font, null/unknown fields,
  registry-without-authorization and protected reader refusal controls.

### Task 2: Immutable authorized MCP font inputs

**Files:** `mcp/config.rs`, `protocol.rs`, `session.rs`, `worker.rs`,
`tests/mcp.rs`, `tests/automation_fonts.rs`, `docs/automation.md`.
**Interfaces:** Config owns FontInputs. `Call::ListFonts` replies without a worker.
Worker owns Vec<NamedTempFile> until reaped; uses config font bytes to create
private files and passes repeated --font only for referenced overrides.

- [x] Add failing modern/legacy catalog and explicit paragraph preview tests.
- [x] Parse bounded repeated --font at launch, load immutable assets; expose a
  path-free font catalog. Add matching tool schema and strict shared validation.
- [x] Validate selected font authorization before spawning; create private font
  snapshots via existing config snapshot helper, flush, keep handles in Worker.
  Worker Drop must reap before font handles are dropped. Add mutation/deletion,
  unknown-font, cancellation/forced-exit and ownership tests.
- [x] Verify modern and legacy Preview/Apply/Reinspect/Save with selected fonts,
  followed by no-override re-edit; old clients without fonts remain unchanged.
  Run affected library/CLI/MCP/paragraph/nested/form suites with fingerprints.

### Task 3: Fresh output acceptance and final review

**Files:** artifact scripts/results under `artifacts/automation-paragraph-fonts-20261028`,
`docs/product-quality-loop.md`, this plan and its execution ledger.
**Interfaces:** Tasks1/2 supply catalog IDs, preview metadata and authorized workers.

- [x] Build fresh isolated CLI/desktop binaries from the terminal affected gate.
- [x] Exercise synthetic Word missing-glyph substitution, nested/page style cases,
  protected refusal, source-file deletion, Save/Apply/reinspect/re-edit and original
  no-op identity. Independently inspect full expected text, unselected glyph/pixels,
  other-page content and Word clipping/marked scope; retain negative controls.
- [x] Obtain one final scoped review; resolve Important/Critical findings inline
  through RED/GREEN. Keep declined judgments and exact evidence limits explicit.
- [x] Record local checkpoint, evidence and next gap; preserve branch/worktree and
  accepted binaries. Goal remains active; no release or commercial-parity claim.

Self-review: bounds, ownership and no-op semantics map toTask1; launch authorization
and worker lifetime toTask2; saved fidelity and independent evidence toTask3.
