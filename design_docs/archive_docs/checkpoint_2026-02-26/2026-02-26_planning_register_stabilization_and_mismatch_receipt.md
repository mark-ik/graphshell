# Planning Register Stabilization + Mismatch Receipt (2026-02-26)

**Window**: 2026-02-26
**Scope**: Control-plane refinement of active lane guidance (no runtime code changes)

## What changed

1. Expanded `lane:stabilization` with explicit rendering/input regression inventory:
   - focus-ring z-order regression over document view,
   - multi-view camera command ownership mismatch,
   - lasso metadata ID targeting drift,
   - graph/node-pane input consumption and focus ownership edge cases.

2. Added `lane:control-ui-settings` parity checklist items:
   - command palette keyboard + pointer/global trigger parity,
   - global/contextual command semantics convergence,
   - settings IA convergence from legacy bridge booleans,
   - settings tool pane placeholder graduation target.

3. Added active Spec/Code Mismatch register rows:
   - `viewer:settings` selection vs non-embedded placeholder behavior,
   - browser viewer-table claims vs runtime embedded viewers,
   - Wry strategy/spec vs feature/dependency/runtime wiring status.

## Lane-order impact

- No lane-order reorder was introduced.
- Existing canonical order remains unchanged, with the update focused on improving issue seeding clarity and done-gate visibility for top lanes.

## Hotspot assumptions

- Stabilization slices remain constrained to hotspot files (`render/mod.rs`, `app.rs`, `shell/desktop/ui/gui.rs`, `input/mod.rs`, workbench compositor paths).
- Control UI/settings slices remain separated from runtime-followon signal-routing work to reduce merge overlap.

## Closure/update criteria

- Keep these entries active until each item is either:
  1) landed with tests/diagnostics proof and issue receipt, or
  2) explicitly re-scoped/deferred with rationale in a newer dated receipt.
- Any lane sequencing change should create a new dated receipt rather than rewriting prior receipt intent.
