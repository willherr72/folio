# Area redaction drawing planner implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Build a bounded native drawing selection planner for future structural area redaction, without exposing an incomplete area-removal command.

**Architecture:** Pure typed-command interpretation, conservative transformed bounds, atomic error return. Existing canonical text removal and the 0.30 package remain unchanged.

**Tech Stack:** Rust standard library; existing native module registry. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-01-area-redaction-design.md`

## Global constraints

1..64 marks, 100,000 commands, 64 saved states, finite absolute numbers <= 1,000,000, normalized determinant magnitude >= 1e-12. Refuse clipping, hairlines, unfinished paths and state changes inside paths. Selection includes boundary contact. Preserve inherited dirty work and failed evidence; no commits, pushes, cleanup or package replacement in this cycle.

## Review focus

- Stroke geometry wider than the path centerline, especially square caps and sheared miter joins.
- Matrix concatenation and state restoration must not borrow a later object's transform.
- Bounds underflow, nonfinite operands and representationally collapsed rectangles must refuse.
- Compound subpaths and nonpainted path data must not leak through a selected path.
- Resource exhaustion must return no partial decision list.

## Task 1: Typed drawing planner

**Files:** create `src-tauri/src/redaction_area_paths.rs`, `src-tauri/src/redaction_area_paths_tests.rs`; register private module in `src-tauri/src/lib.rs`.

**Interface:** `plan_paths(commands: &[PathCommand], marks: &[AreaRect]) -> Result<Vec<PathDecision>, PathError>`. `AreaRect` has left/bottom/right/top f64. Commands cover Save/Restore/Concat, line width/join/cap/miter, Move/Line/Curve/CurveV/CurveY/Rect/Close, Paint(fill/stroke), End, Clip, Unchanged and Unsupported. Decisions bind inclusive first/last indices and bounds to removal/painted/extends-beyond flags. The function has no side effects.

- [x] Write tests for whole-path selection, retained neighbors, compound paths, curves, transformed strokes, malformed state and numeric/budget limits; run a deliberately unimplemented function and observe the positive assertions fail.
- [x] Implement bounded geometry and command interpretation. Run actual module tests with `rustc --edition=2021 --test src-tauri/src/redaction_area_paths.rs -o <cycle>/planner-tests.exe`, then execute serially. Require every test to pass.
- [x] Add independent sampled containment tests and verify multiple negative mutations are rejected by the suite. No mutation may touch the live source; compile isolated copies under the cycle artifact root.

## Task 2: Integration boundary and review

- [x] Register the private module, compile it in a crate-like privacy context as well as directly, and verify no existing product file besides `lib.rs` changed from the accepted product fingerprint.
- [x] Freeze scoped source/tests/spec/plan/evidence hashes; request one fresh final scoped code review. Correct demonstrated issues in one coordinator pass with regressions. Do not create a new review for small corrections.
- [x] Record exact terminal evidence and remaining PDF-adapter/publication/image/UI acceptance work in the ledger and roadmap. Keep the goal active and package 0.30 intact.

## Plan self-review

This implements only the explicitly scoped foundation milestone in the complete design. Every review-focus class has tests in Task 1. No PDF publication or desktop completion is claimed. The standard-library-only module permits direct exact-source testing without repeating the multi-hour native regression for an uncalled private planner; later integration must run the affected native engine suites.
