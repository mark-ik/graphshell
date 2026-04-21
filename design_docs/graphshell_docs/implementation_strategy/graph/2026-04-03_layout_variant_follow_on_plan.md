<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Layout Variant Follow-On Plan (2026-04-03)

**Status**: Active follow-on plan. Updated 2026-04-19 — the `Layout<N>` trait
home moved from `graph::layouts/` to `graph_canvas::layout`; Radial,
Phyllotaxis, and rapier2d have landed; Timeline and Kanban are the active
next-wave targets. Penrose and L-system move to a Step-5 design pass (see
[2026-02-24_physics_engine_extensibility_plan.md](2026-02-24_physics_engine_extensibility_plan.md)).
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

## Feature Target 2: First Analytic Variant Wave — partly landed

### Target 2 Context

The first new variants should maximize conceptual difference from FR while minimizing dependency
surface.

### First Wave — 2026-04-19 status

1. `graph_layout:radial` — **landed** as `graph_canvas::layout::Radial` with
   `RadialConfig` (focus, center, ring_spacing) and BFS-based ring assignment.
2. `graph_layout:phyllotaxis` — **landed** as `graph_canvas::layout::Phyllotaxis`
   with inward/outward `SpiralOrientation`.
3. `graph_layout:timeline` — **pending** (needs metadata slot on
   `LayoutExtras`; see Target 2.1 below).

### Target 2 Tasks (revised)

1. ~~Add one module per variant under `graph/layouts/`.~~ Variants live in
   `crates/graph-canvas/src/layout/static_layouts.rs` under the portable
   `Layout<N>` trait; they implement delta-to-target semantics with a
   `StaticLayoutState.damping` for instant-or-eased application.
2. Define deterministic input ordering rules for each variant.
3. Add stable variant IDs and compatibility declarations consistent with
   `layout_algorithm_portfolio_spec.md`.
4. Route those variants through the `LayoutAlgorithm` registry at
   `app::graph_layout`, which is what graphshell's host already uses for
   one-shot apply (`Grid` and `Tree` already live there; Radial, Timeline,
   Phyllotaxis need registry entries that delegate to the graph-canvas impls).
5. Define how transitions between variants use `2026-04-03_layout_transition_and_history_plan.md`
   instead of ad hoc one-off animation behavior.

### Target 2.1 — Timeline metadata slot

Timeline needs a per-node time coordinate that isn't in the current
`CanvasSceneInput` / `LayoutExtras`. Design question:

- **Option A**: add `time_by_node: HashMap<N, f64>` to `LayoutExtras`.
  Narrow, explicit, matches Kanban's analog.
- **Option B**: generalize to `axis_value_by_node: HashMap<N, AxisValue>`
  where `AxisValue` is `enum { Numeric(f64), Categorical(String) }`.
  Shared slot for Timeline's x-axis time + Kanban's column bucket tag +
  future axial layouts (UDC-depth, topic-frequency, etc.).

Option B is marginally more work but aligns the Timeline/Kanban/future
analog into one slot and one mental model. Recommended.

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

## Feature Target 4: Second Wave and Admission Bar

### Target 4 Context

Once the first wave lands, Graphshell can admit more specialized variants without turning the
portfolio into an uncurated idea dump.

### Second Wave — 2026-04-19 status

- **Kanban / column projection** — **pending** as Target 4.1 below. Reads
  per-node categorical tags, buckets into columns. Blocks on the same
  `LayoutExtras` slot decision as Timeline (see Target 2.1). Small
  implementation (~80 LOC) once the slot exists.
- **rapier-backed scene layout** — **landed** as
  `graph_canvas::layout::RapierLayout` (feature-gated behind `simulate`).
  Current revision rebuilds the world per step; a persistent variant that
  carries momentum across frames is tracked in
  [../../../archive_docs/checkpoint_2026-04-20/graphshell_docs/implementation_strategy/graph/2026-04-19_persistent_rapier_adapter_plan.md](../../../archive_docs/checkpoint_2026-04-20/graphshell_docs/implementation_strategy/graph/2026-04-19_persistent_rapier_adapter_plan.md) (archived 2026-04-20; momentum-preserving drag-release landed).
- **Penrose / aperiodic tiling** — **pending design pass** as part of
  Step 5 in [2026-02-24_physics_engine_extensibility_plan.md](2026-02-24_physics_engine_extensibility_plan.md).
  Open questions: P2 kite-dart vs P3 rhombus, handling of node counts that
  don't fit a subdivision level cleanly.
- **L-system path layouts** — **pending design pass** as part of Step 5.
  Open questions: built-in grammars only (Hilbert / Koch / dragon) or a
  runtime grammar registry; if registry, grammar syntax and sandboxing.

### Target 4.1 — Kanban

Column bucket by categorical node value. Depends on the `LayoutExtras`
metadata slot decision in Target 2.1.

```rust
pub struct KanbanConfig {
    pub origin: Point2D<f32>,
    pub column_gap: f32,
    pub row_gap: f32,
    /// Canonical order of columns left-to-right. Nodes whose tag is absent
    /// go in an "other" column at the end.
    pub column_order: Vec<String>,
}
```

Within each column, nodes are stacked top-down in stable index order.
Columns with no members are omitted.

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
