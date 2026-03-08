# Test Guide

**Last Updated**: 2026-03-03  
**Status**: Active  
**Purpose**: Canonical testing entry guide for Graphshell.

**Related**:
- `../implementation_strategy/subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md`
- `../../OPERATOR_GUIDE.md`

---

## 1) Test Layers

1. **Unit tests** (`cargo test`)  
   Validate reducer logic, routing helpers, and deterministic behavior.

2. **Scenario/integration tests** (`shell/desktop/tests/*`)  
   Validate frame/tile/workbench behaviors and routing flows.

3. **Smoke scripts** (`scripts/dev/smoke-matrix.ps1|.sh quick`)  
   Baseline runtime confidence checks.

4. **Headed/manual checks**  
   Validate interaction UX, accessibility behavior, and rendering/perf regressions.

---

## 2) Baseline Commands

```powershell
cargo check
cargo test
pwsh -NoProfile -File scripts/dev/smoke-matrix.ps1 quick
```

Selective validation packs (run behavior-specific slices only when needed):

```powershell
pwsh -NoProfile -File scripts/dev/test-select.ps1 list
pwsh -NoProfile -File scripts/dev/test-select.ps1 show camera-lock
pwsh -NoProfile -File scripts/dev/test-select.ps1 run input-routing
pwsh -NoProfile -File scripts/dev/test-select.ps1 run navigation-history
pwsh -NoProfile -File scripts/dev/test-select.ps1 suggest
pwsh -NoProfile -File scripts/dev/test-select.ps1 run-affected
pwsh -NoProfile -File scripts/dev/test-select.ps1 suggest --base origin/main
pwsh -NoProfile -File scripts/dev/test-select.ps1 run-affected --base origin/main
pwsh -NoProfile -File scripts/dev/test-select.ps1 changed --scope staged
pwsh -NoProfile -File scripts/dev/test-select.ps1 suggest --scope worktree
pwsh -NoProfile -File scripts/dev/test-select.ps1 run-affected --scope base --base origin/main
pwsh -NoProfile -File scripts/dev/test-select.ps1 suggest --scope worktree --quiet
pwsh -NoProfile -File scripts/dev/test-select.ps1 run-affected --scope worktree --dry-run --quiet
pwsh -NoProfile -File scripts/dev/test-select.ps1 list-policy
pwsh -NoProfile -File scripts/dev/test-select.ps1 lint-policy --platform linux --base origin/main
pwsh -NoProfile -File scripts/dev/test-select.ps1 run-policy --tier pr-required --platform linux --affected --base origin/main --quiet
pwsh -NoProfile -File scripts/dev/test-select.ps1 run-policy --tier nightly --platform windows --dry-run --quiet
```

`test-select.ps1` supports `--scope all|base|worktree|staged|unstaged|untracked` on `changed`, `suggest`, and `run-affected`.
Default scope is `all` (base delta, if provided, plus working tree).
Use `--quiet` to suppress changed-file listings and show pack-focused output for CI logs.

### Policy-Driven CI Contract (canonical)

- `scripts/dev/test-contracts.json` is the canonical source of truth for routine validation packs.
- Each pack may declare `policy.tiers`, `policy.platforms`, and `policy.alwaysRun`.
- CI workflows should invoke `run-policy` instead of hard-coding pack IDs, so new packs are auto-adopted by policy metadata.
- `pr-required` tier is the blocking PR gate baseline; `pr-optional` is informative PR signal; `nightly` is full confidence sweep.
- `lint-policy` is a required guardrail in CI: it validates policy metadata and fails if any scenario file lacks nightly coverage, or if changed scenario files lack PR-tier coverage.

If diagnostics-focused checks are needed, use existing diagnostics test targets already referenced in strategy docs.

Diagnostics infrastructure conventions (Graphshell-wide):

- Use `proptest` for diagnostics invariants (event-stream and aggregate-counter properties).
- Use `insta` for diagnostics snapshot regression checks (JSON shape and deterministic traces).
- New diagnostics channel families should include at least one deterministic test in one of the two forms above.

---

## 3) Scope Rules

- **Back/Forward scope**: traversal navigation in active tile context.
- **Undo/Redo scope**: workbench structural edits (tile/frame/split/reorder/open/close).
- **Preview contract**: `Ctrl+Z` hold-preview behavior must remain test-visible per keybinding spec.

---

## 4) Minimum Acceptance Checks for UX Baseline

1. Node open first-activation renders content reliably.
2. Tile split/merge/reflow preserves deterministic focus.
3. Node open routing follows frame-context rules.
4. Render-mode behavior is policy-conformant.
5. Degradation/fallback reasons are observable.
6. Keyboard and pointer paths produce equivalent semantics.

---

## 5) Suggested Regression Buckets

- **Routing regressions**: node→tile/frame context routing, chooser/default behavior.
- **Tile tree regressions**: split orientation, grouping, reorder stability.
- **Lifecycle regressions**: `Active`/`Warm`/`Cold`/`Tombstone` mapping coherence and restore/remove parity.
- **Render regressions**: composited overlay visibility and mode dispatch.
- **Persistence regressions**: frame/tile arrangement restore parity.

---

## 6) Test Artifact Convention

When manual validation is required, capture:

- command(s) run,
- scenario fixture used,
- observed result,
- expected result,
- pass/fail,
- follow-up issue reference if failed.

Keep long-form historical validation logs in archive checkpoints; keep this file as the active testing entry point.

