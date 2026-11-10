# Radio-group authoring implementation plan

> Use superpowers:executing-plans inline. Steps use checkbox tracking; the user's
> standing authorization removes routine approval questions. One final reviewer.

**Goal:** Create labeled, mutually exclusive PDF radio groups with the existing
placement/preview/Undo workflow and faithful saved output.
**Architecture:** Extend the field request union and native immutable authoring;
return all new widgets internally for bounded validation. Reuse the public candidate
transaction and current Fill forms editor.
**Tech stack:** Rust/lopdf/PDFium, React/TypeScript, existing Windows CDP harness.
**Spec:** `docs/superpowers/specs/2026-10-25-radio-authoring-design.md`.

## Global constraints

2–32 unique nonempty choices; 256 scalars/1024 bytes per label, 32768 bytes total.
Helvetica WinAnsi labels; Unicode group names/accessibility labels. Fixed 10 pt
circle, 18 pt label start and 2 pt right inset. Row minimum max(16,fontSize*1.2+4).
No truncation/substitution; all intrinsic/workspace quarter turns and UserUnit.
Preserve original bytes and unrelated work. No commits/releases. One Cargo at a time.

## Review focus

1. Numeric radio on-state names and Opt/control order must preserve exclusive
   selection and export labels across filling/save/reopen, including non-ASCII labels.
2. Source/workspace rotations, nonzero crop origins and UserUnit must preserve row
   order, visible physical sizes and click rectangles.
3. Multi-widget native validation must not enumerate unrelated oversized options,
   leak candidates, or amplify shared resources beyond existing graph budgets.
4. Existing/shared/duplicated fields and root resource maps must remain isolated.
5. Long labels, small frames and changing option counts must refuse overflow;
   stale previews and borrowed source IDs cannot authorize Apply.

## 1. Native authoring and regression

Files: `src-tauri/src/form_author.rs`, new `src-tauri/src/form_radio_author.rs`,
`src-tauri/src/form_author_engine.rs`,
`src-tauri/tests/form_radio_authoring.rs`. Reuse engine persistence text metrics.
Consumes current immutable create API; produces NewFormField::Radio {options}
and internal CreatedWidget {annotation_index,bounds,option_index} list.

- [x] Add JSON-request-based tests for creation, Opt labels, exclusive switching,
  save/reopen and unchanged sources; run `cargo test --test form_radio_authoring`
  and observe unknown-radio failure before implementation.
- [x] Implement parent/children, off/on circle-and-label appearances, physical
  row layout and exact new-widget validation. Reuse bounded root/resource copies.
- [x] Add 48 rotation/unit cases, duplicate isolation, invalid count/repertoire,
  measured overflow, existing field/protected-source and unrelated-option cases.
  Expected: all native cases pass with exact values and physical geometry.

## 2. Creation UI

Files: `src/editor/form-authoring.ts`, `src/components/FormAuthorDialog.tsx`,
`tests/form-radio-authoring.test.tsx`. Public adapter method remains unchanged.

- [x] Add a failing radio-type/choices/geometry/preview test, then implement the
  union, validation, type choice and initial group sizing. Explain vertical rows
  and that only one option can be selected. Candidate ownership stays unchanged.
- [x] Run focused authoring and form-filling tests, then full frontend/build gates.
  Expected: valid labeled group reaches native preview; invalid or changed drafts
  cannot Apply; old field types and candidate disposal remain green.

## 3. Review and desktop acceptance

- [x] Freeze source delta against this cycle's pre-change baseline, not Git HEAD;
  dispatch one scoped reviewer with the spec, plan, ledger and focus above.
- [x] Parent fixes demonstrated findings red-to-green; no rereview. Run native
  library/authoring/filling/preservation/protection regression and build candidate.
- [x] Windows create/select/switch/Undo/Redo/cancel, duplicate, save/reopen and
  narrow controls. Independently verify parent value, all child states/labels,
  untouched fields and pixels; inspect rotated output. Preserve failures/inputs.
- [x] Bind executable/source/PDFium hashes, stop owned apps, update README and
  quality ledger. Keep broad goal active and list genuine remaining gaps.
