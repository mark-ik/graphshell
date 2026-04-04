<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Layout Variant Follow-On Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the built-in layout-variant lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan focused on new native layout families beyond FR/Barnes-Hut.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-04-03_layout_backend_state_ownership_plan.md`
- `2026-04-03_wasm_layout_runtime_plan.md`
- `2026-04-03_layout_transition_and_history_plan.md`
- `layout_algorithm_portfolio_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `multi_view_pane_spec.md`
- `view_dimension_spec.md`

---

## Context

The physics extensibility seam is landed far enough that Graphshell no longer needs to keep all
future layout work inside one umbrella note.

Current reality:

- post-physics helpers are Graphshell-owned and useful for force-directed refinement
- `ActiveLayout` / `ActiveLayoutState` already provide a built-in layout dispatcher
- Barnes-Hut proves Graphshell can ship new layout variants without taking full ownership of the
  upstream `Layout<S>` / `LayoutState` trait contract

This lane is specifically about **new native built-in layout families**. It depends on the shared
state-carrier decisions in `2026-04-03_layout_backend_state_ownership_plan.md`, and it should stay
distinct from the runtime-loaded guest layout work tracked in
`2026-04-03_wasm_layout_runtime_plan.md`.

---

## Non-Goals

- replacing the upstream `egui_graphs` trait contract in this lane
- turning built-in native variants into a proxy for the WASM runtime lane
- rapier scene-physics as a prerequisite for the first analytic variants
- quietly expanding this plan into layout transition/history ownership

---

## Feature Target 1: Adopt the Shared State Carrier

### Target 1 Context

`ActiveLayoutState` is currently too FR-shaped for analytic or topology-driven layouts. This plan
should consume the widened carrier defined in `2026-04-03_layout_backend_state_ownership_plan.md`
rather than inventing a second persistence model.

### Target 1 Tasks

1. Replace the FR-shaped `physics`-only assumption with per-variant payload support inside the
   shared Graphshell-owned state carrier.
2. Keep the outer adapter compatible with the current upstream `Layout<S>` / `LayoutState` seam.
3. Preserve per-`GraphViewId` persistence and deterministic restore behavior.
4. Ensure the render layer continues to see one concrete `ActiveLayoutState` type.

### Target 1 Validation Tests

- ForceDirected and BarnesHut roundtrip through the widened carrier without state loss.
- Snapshot restore does not relayout unexpectedly when restoring the same variant and state.
- Unknown or unavailable variants degrade through the documented fallback path.

---

## Feature Target 2: Land the First Analytic Variant Wave

### Target 2 Context

The first new variants should maximize conceptual difference from FR while minimizing dependency
surface.

### Candidate First Wave

1. `graph_layout:radial`
2. `graph_layout:timeline`
3. `graph_layout:phyllotaxis`

### Target 2 Tasks

1. Add one module per variant under `graph/layouts/`.
2. Define deterministic input ordering rules for each variant.
3. Add stable variant IDs and compatibility declarations consistent with
   `layout_algorithm_portfolio_spec.md`.
4. Route those variants through `ActiveLayout` rather than through helper accumulation.
5. Define how transitions between variants use `2026-04-03_layout_transition_and_history_plan.md`
   instead of ad hoc one-off animation behavior.

### Target 2 Validation Tests

- Switching among ForceDirected, BarnesHut, and first-wave variants preserves graph truth.
- Variant selection/restoration is deterministic by stable external ID.
- Layout switches can opt into the shared transition/history path without changing graph truth.

---

## Feature Target 3: Portfolio and Diagnostics Integration

### Target 3 Context

New variants only pay off if they are visible to selection policy, fallback logic, and quality
diagnostics rather than living as hidden one-off modules.

### Target 3 Tasks

1. Ensure the portfolio registry can discover each built-in variant by canonical ID.
2. Define compatibility/fallback rules for graphs or views that cannot satisfy a requested
   analytic layout.
3. Expose requested vs resolved variant IDs through diagnostics.
4. Ensure recommendation logic distinguishes analytic layouts from dynamic physics layouts.

### Target 3 Validation Tests

- The portfolio registry can discover each new built-in variant by canonical ID.
- Unknown or incompatible requests fall back deterministically without mutating graph truth.
- Diagnostics identify both the requested and resolved variant IDs.

---

## Feature Target 4: Set the Admission Bar for Later Variants

### Target 4 Context

Once the first wave lands, Graphshell can admit more specialized variants without turning the
portfolio into an uncurated idea dump.

### Candidate Second Wave

- Penrose / aperiodic tiling
- L-system path layouts
- Kanban / column projection
- rapier-backed scene layout

### Target 4 Tasks

1. Admit a new built-in variant only if it needs native compile-time ownership and does not fit
   better as a runtime-loaded WASM guest.
2. Require a stable external ID, deterministic input ordering, and a fallback story before adding
   any new variant.
3. Keep scene-backed and projection-backed variants aligned with their own authorities instead of
   collapsing everything into one layout bucket.

### Target 4 Validation Tests

- Candidate variants can be rejected or deferred without ambiguity.
- The admission bar cleanly separates built-in native variants from runtime-loaded guest layouts.
- Variant growth does not force immediate replacement of the upstream trait seam.

---

## Exit Condition

This plan is complete when Graphshell can ship at least one non-FR analytic layout family beyond
Barnes-Hut through the same built-in dispatcher, shared persisted state carrier, and diagnostics
path used by the existing force-directed variants.
