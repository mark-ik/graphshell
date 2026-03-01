# 2026-02-28 Stabilization Progress Receipt (`lane:stabilization` / `#88`)

## Scope

This receipt captures stabilization slices landed on `main` after the prior partial-progress reference in `PLANNING_REGISTER.md` and records issue-facing evidence for camera/input/focus/lasso regressions.

## Landed slices (chronological)

- `001a121` — keyboard zoom commands now requeue when graph metadata is temporarily unavailable, preventing dropped zoom intent behavior.
- `004fd13` — lasso boundary hit-testing updated to center-inclusive semantics with regression coverage.
- `18c6ae9` — node-open focus handoff hardened with timeline tracing to reduce blank-first-frame activation races.
- `874e2a6` — stale focus hints cleared during pane-close handoff.
- `441aded` — active graph pane recovery after node-pane close handoff, plus deterministic scenario/unit coverage.
- `d8983c9` — focus activation defers unmapped primary target and applies mapped fallback while preserving retry hint.
- `4708e55` — deferred focus activation diagnostics receipts added.
- `f12e0cc` — regression coverage added for deferred focus activation diagnostics emission.
- `37350e7` — background click deselect now ignores frames where graph primary-click actions were already handled.
- `4f1011f` — unmapped semantic child-webview opens are surfaced via warn-level diagnostics instead of silent drop.
- `e755f48` — unmapped semantic child-webview opens are deferred/retried across frames via runtime queueing.

## Validation evidence

- Targeted workbench/unit tests for focus activation targeting and diagnostics emission are passing.
- `cargo check` passed for each stabilization slice.
- `scripts/dev/smoke-matrix.ps1 quick` (Windows quick profile) passed after the latest stabilization test coverage commit.
- `scripts/dev/smoke-matrix.ps1 quick` (Windows quick profile) passed after each newly landed stabilization slice above.

## Register-state update intent

This lane remains `partial` in readiness terms because the stabilization bug register in `PLANNING_REGISTER.md` still includes unresolved items outside the landed focus/lasso/camera slices (notably selection deselect consistency and compositor pass-contract closure evidence).

## Issue update payload (`#88`)

Suggested issue comment body:

> Stabilization progress receipt (2026-02-28) has been landed in docs with commit evidence and validation notes:
> `design_docs/graphshell_docs/implementation_strategy/2026-02-28_stabilization_progress_receipt.md`.
>
> Newly landed commits on `main`: `001a121`, `004fd13`, `18c6ae9`, `874e2a6`, `441aded`, `d8983c9`, `4708e55`, `f12e0cc`, `37350e7`, `4f1011f`, `e755f48`.
>
> Net result: camera/zoom command reliability and lasso boundary correctness improved; pane-open/pane-close focus activation race is substantially hardened with new diagnostics receipts and regression tests. Lane remains partial pending closure of remaining bug-register items and compositor pass-contract closure evidence.
