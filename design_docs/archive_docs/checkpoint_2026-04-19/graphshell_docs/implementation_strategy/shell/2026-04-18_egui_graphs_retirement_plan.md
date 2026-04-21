<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# egui_graphs Retirement Plan (2026-04-18)

**Status**: **Archived 2026-04-19** — scope complete. `egui_graphs` is
retired from the live path; `graph-canvas` is the sole graph scene /
interaction / camera authority. Live-path follow-ons moved to their own
plans:

- Steps 2–6 layout portfolio → [../../../../graphshell_docs/implementation_strategy/graph/2026-04-19_step5_spatial_pattern_layouts_plan.md](../../../../graphshell_docs/implementation_strategy/graph/2026-04-19_step5_spatial_pattern_layouts_plan.md)
- Persistent rapier adapter → [../../../../checkpoint_2026-04-20/graphshell_docs/implementation_strategy/graph/2026-04-19_persistent_rapier_adapter_plan.md](../../../../checkpoint_2026-04-20/graphshell_docs/implementation_strategy/graph/2026-04-19_persistent_rapier_adapter_plan.md) (archived 2026-04-20)
- Pluggable-mods registry → [../../../../graphshell_docs/implementation_strategy/graph/2026-04-19_layouts_as_pluggable_mods_plan.md](../../../../graphshell_docs/implementation_strategy/graph/2026-04-19_layouts_as_pluggable_mods_plan.md)
- §5.6 plumbing re-lands (background pan / wheel pan / pinch / inertia /
  Fit / overlays) → [../../../../checkpoint_2026-04-20/graphshell_docs/implementation_strategy/shell/2026-04-19_graph_canvas_overlays_and_camera_relands_plan.md](../../../../checkpoint_2026-04-20/graphshell_docs/implementation_strategy/shell/2026-04-19_graph_canvas_overlays_and_camera_relands_plan.md) (archived 2026-04-20)
- Flaky test hygiene → [../../../../checkpoint_2026-04-20/graphshell_docs/implementation_strategy/testing/2026-04-19_flaky_test_hygiene_plan.md](../../../../checkpoint_2026-04-20/graphshell_docs/implementation_strategy/testing/2026-04-19_flaky_test_hygiene_plan.md) (archived 2026-04-20)
- Step 7 WASM layout adapter → [../../../../graphshell_docs/implementation_strategy/graph/2026-04-03_wasm_layout_runtime_plan.md](../../../../graphshell_docs/implementation_strategy/graph/2026-04-03_wasm_layout_runtime_plan.md)
- Graph-canvas input / accessibility follow-on → [../../../../graphshell_docs/implementation_strategy/shell/2026-04-19_graph_canvas_input_accessibility_followon_plan.md](../../../../graphshell_docs/implementation_strategy/shell/2026-04-19_graph_canvas_input_accessibility_followon_plan.md)

**Original scope**: Complete M2 of the iced host migration by retiring
`egui_graphs` as a live dependency, leaving a clean baseline in which
`graph-canvas` is the sole graph scene/interaction/camera authority.

**Parent plan**: [../../../../graphshell_docs/implementation_strategy/shell/2026-04-14_iced_host_migration_execution_plan.md](../../../../graphshell_docs/implementation_strategy/shell/2026-04-14_iced_host_migration_execution_plan.md)

**Driving directive**: "ensure redundancy doesn't carry over and that we don't
have any weird legacy issues or compensations. let's just get a clean, good
baseline 'this should work' out of retiring egui_graphs" — user, 2026-04-18.

---

## 1. Revised Scope (after verification)

Earlier drafts assumed three interlocked slices (MetadataFrame, overlays,
physics). Verification showed most of that surface is already dead on the
live path. The revised baseline scope is:

### A. Physics off `egui_graphs::Graph` (required for clean baseline)

- Rework the `LayoutCalculator` trait to operate on petgraph (`crate::graph::Graph`)
  or a position snapshot instead of `egui_graphs::Graph<...>`
- Migrate `graph/layouts/graphshell_force_directed.rs`,
  `barnes_hut_force_directed.rs`, `active.rs`
- Replace `graph/physics.rs` re-exports of `egui_graphs::FruchtermanReingold*`
  with internal-owned equivalents
- Delete `render::sync_graph_positions_from_layout` (physics writes directly
  to petgraph; no copy step needed)
- Delete `app.workspace.graph_runtime.egui_state` and `egui_state_dirty`

### B. Dead-code sweep

All of the following are unreachable from the live graph pane today (see §2):

- `render/canvas_camera.rs` — entire file retires (its dead-code public
  surface has no live callers)
- `render/canvas_overlays.rs` — all `draw_*` functions retire (none live)
- `render/canvas_input.rs` — `collect_lasso_action`, `collect_graph_actions`,
  `collect_graph_keyboard_traversal_action` retire
- `render/mod.rs` — `graph_view_metadata_id`, `graphshell_owned_navigation_settings`,
  `canvas_interaction_settings`, `canvas_style_settings`,
  `pointer_canvas_pos` (when its MetadataFrame dep is gone), and any
  surrounding dead helpers
- `model/graph/egui_adapter.rs` — whole module deletes
- `render/canvas_visuals::apply_search_node_visuals` — test-only; delete with
  its tests

### C. Re-land plumbing-only features

The dead functions above support features that, after §2 analysis, meet the
Firefox/standards/ergonomic/modular bar with only a data-source rework (i.e.,
swap `egui_state.graph` reads for petgraph + `CanvasCamera`):

1. **Background pan** (primary-drag pans the camera)
2. **Wheel pan** (plain wheel translates camera — infinite-canvas convention,
   not webpage convention)
3. **Ctrl/Cmd + wheel zoom** (also captures trackpad pinch via synthetic
   `ctrlKey`)
4. **Middle-click drag → free pan** (Figma/Miro idiom, appropriate for
   infinite canvas)
5. **Pan inertia** (kept; reinforces the spatial/physical feel of the
   force-directed space)
6. **Fit / FitSelection / FitGraphlet** (bounds computed from petgraph,
   applied to `CanvasCamera`)
7. **Frame affinity backdrops** (decorative visual layer over world space)
8. **Scene region backdrops** (Arrange/Simulate mode chrome)
9. **Highlighted edge overlay** (hovered/selected edge visual)

All routed through `CanvasCamera` as single camera authority. No MetadataFrame.

### D. Re-land plumbing-only features: host-neutral seam

The baseline re-lands should extend `canvas_bridge` rather than egui-specific
code where feasible, so the iced host later inherits them for free:

- Pan/zoom/fit: live in `CanvasCamera` + host-neutral helpers
- Overlay draws: emit extra `ProjectedScene` overlay items (which
  `canvas_egui_painter` already paints) rather than host-specific painter
  calls
- Middle-click and ctrl+wheel input translation: in
  `canvas_bridge::collect_canvas_events`

### E. Drop the dependency

- Remove `egui_graphs` from `Cargo.toml`
- Cargo.lock regen
- Run full workspace build + tests

### NOT in baseline (see follow-on plan)

- Lasso redesign (accessibility, right-drag conflict with context menu)
- Tab traversal redesign (spatial navigation, ARIA roles, announcements)
- Hover tooltips (focus-triggered variant, ARIA, keyboard dismissal)
- Keyboard zoom shortcut rebinding (`Ctrl+=` / `Ctrl+-` / `Ctrl+0`)

These require real design work and cannot be "ported as-is". Tracked in the
follow-on plan.

---

## 2. Findings

### Live graph-pane surface (verified)

- [render_graph_canvas_in_ui](../../../../render/mod.rs) (render/mod.rs:475) —
  graph-canvas only, zero egui_graphs
- [render_graph_info_in_ui](../../../../render/mod.rs) (render/mod.rs:401) →
  `graph_info::draw_graph_info` — touches `egui_state` only for dirty flag +
  one scene-gather `set_location` sync
- [sync_graph_positions_from_layout](../../../../render/mod.rs)
  (render/mod.rs:599) — reads from `egui_state.graph`, writes to petgraph
- [render_graph_pane_overlay](../../../../shell/desktop/workbench/tile_behavior.rs)
  (shell/desktop/workbench/tile_behavior.rs:1094) — chrome only (Split Graph
  button); hint text reads *"Lens, depth, fit, and physics moved to the graph
  host"* confirming original intent for migration

### Dead-on-live-path surface (verified by reverse-grep)

No live callers found for:

- `canvas_camera::handle_custom_navigation` / `apply_pending_camera_command` /
  `apply_pending_keyboard_zoom_request` / `apply_pending_wheel_zoom` /
  `apply_background_pan` / `apply_background_pan_inertia`
- `canvas_input::collect_lasso_action` / `collect_graph_actions` /
  `collect_graph_keyboard_traversal_action`
- `canvas_overlays::draw_frame_affinity_backdrops` /
  `draw_highlighted_edge_overlay` / `draw_hovered_node_tooltip` /
  `draw_hovered_edge_tooltip` / `draw_scene_runtime_backdrops` /
  `draw_scene_simulate_overlays` / `draw_scene_region_action_overlay`
- `render_graph_in_ui_collect_actions` (only mentioned in a comment)
- `graph_view_metadata_id` — called only from its own tests
- `apply_search_node_visuals` — called only from tests

The functions are `pub(super)` or `pub(crate)`, so Rust's dead-code lint
doesn't flag them. That's why the drift survived.

### Input convention for the graph canvas

The graph canvas is infinite-canvas territory, not webpage territory. The
correct conventions (stored in agent memory as
[feedback_graph_canvas_navigation_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_graph_canvas_navigation_defaults.md)):

- Plain wheel → pan (like Firefox inside infinite-canvas documents)
- Ctrl/Cmd + wheel → zoom (also trackpad pinch)
- Middle-click drag → free pan (Figma/Miro idiom)
- Pan inertia on release → keep (reinforces spatial/physical feel)

---

## 3. Execution Order

1. Build current tree; capture baseline test count
2. Retire physics off `egui_graphs::Graph` (§1.A)
3. Dead-code sweep (§1.B)
4. Re-land plumbing-only features on `CanvasCamera` + host-neutral seam
   (§1.C, §1.D)
5. Drop `egui_graphs` dependency (§1.E)
6. Run full workspace build + tests; verify no new failures

---

## 4. Progress

### 2026-04-18

- Plan created.

### 2026-04-19

- Scope verified against live path. Revised plan: physics retirement + dead
  code sweep + plumbing-only re-lands; follow-on plan for features needing
  design work. Saved pan/zoom defaults to agent memory.

- **Dead-code sweep landed** (§1.B). Deleted: `render/canvas_camera.rs`,
  `render/canvas_overlays.rs`, `render/canvas_input.rs` (three files, all
  unreachable from the live path). From `render/mod.rs`: removed the three
  `mod` decls, their production and test imports, `graph_view_metadata_id`,
  `pointer_canvas_pos`, `apply_active_scene_region_drag`,
  `graphshell_owned_navigation_settings`, `canvas_interaction_settings`,
  `canvas_style_settings`, `use egui_graphs::{...}`, and ~20 tests that
  depended on deleted symbols. From `render/graph_info.rs`: removed the
  dead-on-live-path `requested_layout_algorithm_id` and
  `should_apply_layout_algorithm`. From `render/canvas_visuals.rs`: removed
  `apply_search_node_visuals` (test-only caller set) and its three tests.
  Kept `canvas_lasso_binding_label` (promoted to `pub(crate)`) — it's live
  via `graph_info.rs:557`.

- `cargo check --lib --tests` green. Only warnings remain (mostly deprecated
  egui methods, unused variables; none from this change).

- **Remaining `egui_graphs` surface after this sweep:**
  - `graph/physics.rs` re-exports `FruchtermanReingold*` etc.
  - `graph/layouts/{graphshell_force_directed,barnes_hut_force_directed,active}.rs`
    operate on `egui_graphs::Graph<...>`
  - `model/graph/egui_adapter.rs` defines `EguiGraphState` / `EguiGraph`
  - `app.workspace.graph_runtime.egui_state`, `egui_state_dirty` fields
  - `render/sync_graph_positions_from_layout` reads `egui_state.graph` and
    writes to petgraph
  - `graph_info.rs` scene-gather `set_location` sync + `egui_state_dirty`
    flag writes
  - `physics.rs::apply_position_deltas` still mirrors writes to `egui_state`
  - Isolated: no camera/overlay/input code still touches egui_graphs

## 5. Next Session — Physics Retirement (Option C, graph-canvas home)

The remaining work is physics migration. All `egui_graphs` dependence now
sits in the physics/layout pipeline and its carrier state (`EguiGraphState`).

**Decision (2026-04-19)**: physics moves into `graph-canvas`, not into
Graphshell-app-local `graph/layouts/`. Options considered:

- ~~**Option A — Vendor FR in `graph/layouts/`.**~~ Rejected as legacy
  compensation. Keeps physics parochial to one host; forces either a
  re-extract or a duplicate port when iced-host lands (M5).
- ~~**Option B — Use `fdg-sim` or similar.**~~ Rejected: new dep with
  different API shape; perturbs existing tuning; ownership cost without
  structural yield.
- **Option C — Rewrite in `graph-canvas`. Accepted.** Right structural
  home (mobile/WASM-clean by construction, iced inherits for free, aligns
  with existing `scene_physics` and `simulate` modules, matches the
  2026-02-24 physics plan's updated home). See
  [../graph/2026-02-24_physics_engine_extensibility_plan.md §2026-04-19](../graph/2026-02-24_physics_engine_extensibility_plan.md#2026-04-19).

### 5.1 The `Layout` trait

Lands in a new module `crates/graph-canvas/src/layout/mod.rs`:

```rust
use std::collections::HashMap;
use std::hash::Hash;
use euclid::default::Vector2D;
use serde::{Deserialize, Serialize};

use crate::scene::CanvasSceneInput;

pub trait Layout<N: Clone + Eq + Hash> {
    type State: Default + Clone + Serialize + for<'de> Deserialize<'de>;

    /// Advance one frame. Returns position deltas for the host to apply
    /// to its own position store. Does not mutate the scene input.
    fn step(
        &mut self,
        scene: &CanvasSceneInput<N>,
        state: &mut Self::State,
        dt: f32,
        extras: &LayoutExtras<N>,
    ) -> HashMap<N, Vector2D<f32>>;

    /// Whether this layout has settled (delta norm below threshold).
    /// Hosts use this to auto-pause the tick.
    fn is_converged(&self, state: &Self::State) -> bool { false }
}

/// Out-of-band inputs for layout composition.
///
/// - `pinned` — nodes whose positions must not be moved
/// - `domain_buckets` — precomputed registrable-domain groups for clustering
/// - `semantic_similarity` — precomputed pair similarities for semantic forces
/// - `regions` — frame-affinity / scene region pulls
pub struct LayoutExtras<N> {
    pub pinned: std::collections::HashSet<N>,
    pub domain_buckets: HashMap<String, Vec<N>>,
    pub semantic_similarity: HashMap<(N, N), f32>,
    pub regions: Vec<crate::scene_region::SceneRegion>,
}
```

Key shape choices:

- **Delta-returning, not mutating** — matches `scene_physics` convention.
  Hosts apply deltas to their own position lane (petgraph for graphshell,
  other carriers for future hosts).
- **`dt: f32`** — explicit timestep; no `std::time` dep; WASM-clean.
- **`extras`** — out-of-band inputs per the 2026-04-03 semantic-clustering
  follow-on plan (semantic vectors computed out-of-band, not per-frame).
- **Generic over `N`** — same NodeKey generic as the rest of the crate;
  no petgraph dependency in graph-canvas.

### 5.2 The `ActiveLayout` dispatcher

```rust
// crates/graph-canvas/src/layout/active.rs
pub enum ActiveLayout<N: Clone + Eq + Hash> {
    ForceDirected(force_directed::ForceDirected<N>),
    BarnesHut(barnes_hut::BarnesHut<N>),
    Radial(radial::Radial<N>),
    Timeline(timeline::Timeline<N>),
    Phyllotaxis(phyllotaxis::Phyllotaxis<N>),
    Grid(grid::Grid<N>),
    // future: Penrose, LSystem, SemanticEmbedding, Wasm, Rapier
}

#[derive(Serialize, Deserialize)]
pub enum ActiveLayoutState { /* per-variant state */ }

impl<N: Clone + Eq + Hash> Layout<N> for ActiveLayout<N> {
    type State = ActiveLayoutState;
    fn step(&mut self, scene, state, dt, extras) -> HashMap<N, Vector2D<f32>> {
        match self { ActiveLayout::ForceDirected(l) => l.step(...), ... }
    }
}
```

Graphshell's `graph::physics` becomes a thin re-export/shim module:

```rust
// graph/physics.rs (post-migration)
pub use graph_canvas::layout::{ActiveLayout, ActiveLayoutState, Layout, LayoutExtras};
pub use graph_canvas::layout::force_directed::{
    ForceDirected as GraphPhysicsLayout,
    ForceDirectedState as GraphPhysicsState,
};
// Graphshell-owned policy stays
pub struct GraphPhysicsTuning { ... }
pub struct GraphPhysicsExtensionConfig { ... }
```

### 5.3 Staged landing order

Each step is a clean stopping point. Baseline-safe iteration.

**Step 1 — Trait + FR (the MVP).** Land:

- `graph-canvas::layout::Layout` trait + `LayoutExtras`
- `graph-canvas::layout::force_directed::{ForceDirected, ForceDirectedState}` —
  vendored from `egui_graphs::FruchtermanReingoldWithCenterGravity` (MIT),
  rewritten to return deltas instead of mutating. ~300 LOC.
- Bridge: `graph::physics` re-exports; frame tick in graphshell reads
  `CanvasSceneInput`, calls `step()`, applies deltas to petgraph.
- Delete `sync_graph_positions_from_layout`, `egui_state`,
  `egui_state_dirty`, `egui_adapter.rs`.
- Drop `egui_graphs` from `Cargo.toml`.

At this point: egui_graphs is **gone**, parity is preserved, iced host
adapter will inherit physics cleanly. Roughly the 1-session budget
described in the previous plan.

**Step 2 — Barnes-Hut.** Vendor `FruchtermanReingoldWithExtras` +
quadtree. ~250 LOC. Behind a `barnes-hut` feature flag if we want to
keep the core `graph-canvas` lean.

**Step 3 — Physics extras as `Layout` composition passes.** Degree
repulsion, domain clustering, semantic clustering, hub pull, frame
affinity. Each is a small `Layout` impl that consumes the same scene + the
precomputed inputs in `LayoutExtras`. Composition via `CompositeLayout`
(run pass N's deltas, accumulate, then run pass N+1 against the mutated
snapshot — simple layered composition).

**Step 4 — Static positional layouts.** From the plan's catalogue, these
are pure math with no iterative state:

- `layout::radial::Radial` — BFS from a focal node; ring n at
  graph-theoretic distance n; angular spacing by degree. ~80 LOC.
- `layout::timeline::Timeline` — x by `created_at` / `last_visited`, y by
  UDC cluster or domain group. ~100 LOC.
- `layout::phyllotaxis::Phyllotaxis` — Fibonacci spiral, priority-keyed.
  Five-line placement formula + ordering state. ~60 LOC.
- `layout::grid::Grid` — rectilinear snap by node count sqrt or by
  explicit row/col. ~50 LOC.
- `layout::kanban::Kanban` — column-bucket by status tag. ~80 LOC.

All of these are `Layout` implementers that return deltas toward their
target positions (so they can compose with a damping pass for
animate-in, or return the full "position − current" delta for instant
placement).

**Step 5 — Geometric layouts (lower priority, higher value/line ratio).**

- `layout::penrose::Penrose` — recursive rhombus subdivision, P2/P3
  variants. ~150 LOC of pure geometry. No crate; golden-ratio
  transforms.
- `layout::l_system::LSystem` — Hilbert curve + Koch + dragon as built-in
  grammars; ~100 LOC. `l-system-fractals` crate if we want external
  grammars.
- `layout::semantic_embedding::SemanticEmbedding` — UMAP-style projection
  of existing UDC / semantic-vector data; reads from `LayoutExtras`. The
  projection itself is out-of-band; the `Layout` impl just reads the
  precomputed 2D coordinates and returns deltas toward them.

**Step 6 — rapier2d `Layout` adapter.** Bridge the already-landed
`RapierSceneWorld` to the `Layout` trait: each `step()` calls
`world.step()`, reads body translations, returns deltas. Makes the
scene-physics path a peer of other layouts rather than a parallel pipeline.

**Step 7 — WASM layout adapter.** Host-side `WasmLayoutAdapter`
implementing `Layout`; delegates `step()` to a guest `compute_layout`
function via extism or wasmtime. Guest ABI: msgpack-serialized scene +
state in, deltas out. Versioned as `layout-wasm-api:1`.

### 5.4 Scope for the next session

Strong recommendation: **Step 1 only** next session. That's the
egui_graphs-retirement MVP:

- One new module (`graph-canvas::layout`) with the trait, `LayoutExtras`,
  and FR vendored.
- One bridge edit in `graph::physics` + frame tick.
- Delete carriers + dep.
- Verify `cargo check --lib --tests` + `cargo test --lib` pass count.

Steps 2–7 become their own subplans (or ride on the existing follow-ons
at [2026-04-03_layout_variant_follow_on_plan.md](../graph/2026-04-03_layout_variant_follow_on_plan.md)
and [2026-04-03_wasm_layout_runtime_plan.md](../graph/2026-04-03_wasm_layout_runtime_plan.md)).

### 5.5 Detailed step 1 checklist

1. Add `crates/graph-canvas/src/layout/mod.rs` with the `Layout` trait,
   `LayoutExtras<N>`, and re-exports for submodules.
2. Add `crates/graph-canvas/src/layout/force_directed.rs` with vendored
   FR + center gravity math. State is `ForceDirectedState`
   (displacement accumulator, damping, last_avg_displacement for
   convergence). `step()` reads `CanvasSceneInput`, computes
   repulsive/attractive/gravity forces, returns per-node deltas. Pin
   respects `LayoutExtras::pinned`.
3. Add `ActiveLayout<N>` enum + `ActiveLayoutState` in
   `crates/graph-canvas/src/layout/active.rs`. Initially only
   `ForceDirected` variant.
4. `Cargo.toml` for graph-canvas: add serde + euclid (already present),
   plus any dependencies the vendored math needs. No new deps expected.
5. In `graphshell/graph/physics.rs`: replace `pub use egui_graphs::...`
   with `pub use graph_canvas::layout::{ActiveLayout, ActiveLayoutState,
   force_directed::*, ...}`. Keep `GraphPhysicsTuning` and
   `GraphPhysicsExtensionConfig` Graphshell-owned.
6. Rewrite `graph/layouts/{graphshell_force_directed,barnes_hut_force_directed,active}.rs`
   to target the new types. Keep the trait surface backward compatible
   at the Graphshell re-export point so downstream code doesn't churn.
7. Update `graph::physics::apply_position_deltas` to write *only* to
   petgraph (no `egui_state` mirror).
8. Delete `render::sync_graph_positions_from_layout` and its three call
   sites ([shell/desktop/workbench/tile_render_pass.rs:155](../../../../shell/desktop/workbench/tile_render_pass.rs#L155),
   [tile_render_pass.rs:225](../../../../shell/desktop/workbench/tile_render_pass.rs#L225),
   [node_pane_ui.rs:98](../../../../shell/desktop/workbench/tile_behavior/node_pane_ui.rs#L98)).
   The physics loop now writes to petgraph directly; the frame tick
   calls `ActiveLayout::step()` and applies deltas inline.
9. Delete `egui_state` and `egui_state_dirty` fields from
   `app.workspace.graph_runtime` ([app/workspace_state.rs](../../../../app/workspace_state.rs)).
10. Remove every `egui_state = Some(...)` initialization,
    `egui_state_dirty = true` flag write, `egui_state.as_ref()` /
    `.as_mut()` read. The compiler will enumerate them.
11. Delete `model/graph/egui_adapter.rs` + its `mod` declaration.
12. Remove `egui_graphs` from `Cargo.toml`. Regenerate `Cargo.lock`.
13. `cargo check --lib --tests` → fix fallout → `cargo test --lib`.
14. Compare test count against this session's baseline; fix regressions.

### 5.6 Plumbing-only re-lands (§1.C / §1.D)

These can happen in the same next session or a third session, whichever
the user prefers:

- Background pan (primary drag) → `CanvasCamera.pan`
- Plain wheel → pan (not zoom) — per memory
  [feedback_graph_canvas_navigation_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_graph_canvas_navigation_defaults.md)
- Ctrl/Cmd + wheel → zoom (also captures trackpad pinch)
- Middle-click drag → free pan
- Pan inertia (keep; reinforces force-directed physicality)
- Fit / FitSelection / FitGraphlet → bounds from petgraph, applied to
  `CanvasCamera`
- Frame affinity backdrops → emit as `ProjectedScene` overlay items
- Scene region backdrops (Arrange/Simulate) → same
- Highlighted edge overlay → same

All routed through `canvas_bridge` so iced inherits them for free.

## 6. File State at End of This Session

Files deleted:

- `render/canvas_camera.rs`
- `render/canvas_overlays.rs`
- `render/canvas_input.rs`

Files edited:

- `render/mod.rs` — imports, helpers, tests cleaned up
- `render/graph_info.rs` — dead layout-gate functions removed
- `render/canvas_visuals.rs` — `apply_search_node_visuals` removed with tests

Compile state: `cargo check --lib --tests` passes (119 warnings, no errors).

Not yet done (physics/adapter): `graph/physics.rs`, `graph/layouts/*.rs`,
`model/graph/egui_adapter.rs`, `app/workspace_state.rs::egui_state`,
`render/mod.rs::sync_graph_positions_from_layout`, `Cargo.toml` are
untouched.

## 7. Step 1 Landed (2026-04-19)

Executed the 14-step MVP checklist in §5.5:

- Added `crates/graph-canvas/src/layout/{mod.rs,force_directed.rs}` with the
  `Layout<N>` trait, `LayoutExtras<N>`, and vendored FR + center gravity
  (delta-returning, flat state). 6 FR unit tests pass.
- `graph::physics` now re-exports `ForceDirected` / `ForceDirectedState` as
  `GraphPhysicsLayout` / `GraphPhysicsState`. Tuning function updated for
  flat-state fields (`c_repulse`, `c_attract`, `damping`, `c_gravity`).
- `render::canvas_bridge::run_graph_canvas_frame` ticks FR in-band: builds
  scene, runs `Layout::step()` when physics is running and the user isn't
  dragging, applies deltas straight to petgraph. No `sync_graph_positions_from_layout`
  anymore.
- Deleted: `graph/layouts/` subtree (active.rs, graphshell_force_directed.rs,
  barnes_hut_force_directed.rs, physics_scenarios.rs),
  `model/graph/egui_adapter.rs` (with `GraphNodeChromeTheme` moved inline
  into `shell/desktop/runtime/registries/theme.rs`),
  `render::sync_graph_positions_from_layout` and its 3 call sites,
  `GraphViewRuntimeState::egui_state` + `egui_state_dirty` fields and ~62
  flag writes across 12 files, `GraphViewState::egui_state` per-view cache.
- `egui_graphs` dropped from `Cargo.toml`.

Live-path effect: force-directed now actually ticks on the graph canvas.
Before Step 1, `LayoutAlgorithm::execute` for `ForceDirectedLayout` was a
no-op and the egui_graphs loop was never reached. Retirement both removes
the dead dependency and restores the missing motion.

`cargo test --lib`: 2144 passed / 0 failed / 3 ignored (serial);
`cargo test -p graph-canvas --lib`: 139 passed.

## 8. Steps 2, 3, 4, 6 Landed (2026-04-19)

Following the staged order in §5.3:

### Step 2 — Barnes-Hut

`crates/graph-canvas/src/layout/barnes_hut.rs`. O(n log n) quadtree-based
repulsion with `θ = 0.5` default; same attraction + center-gravity as FR;
shares `ForceDirectedState` for drop-in swappability at scale.

### Step 3 — Extras as `Layout` impls

`crates/graph-canvas/src/layout/extras.rs` adds `DegreeRepulsion`,
`DomainClustering`, `SemanticClustering`, `HubPull`, `FrameAffinity`. Each is
a `Layout<N>` that reads the scene plus slots on `LayoutExtras`:

- `pinned: HashSet<N>` — nodes that do not receive deltas.
- `domain_by_node: HashMap<N, String>` — precomputed registrable-domain
  groupings for domain clustering.
- `semantic_similarity: HashMap<(N, N), f32>` — precomputed pairwise
  similarity scores for semantic clustering.
- `frame_regions: Vec<FrameRegion<N>>` — anchor + members per frame; the
  layout computes the member centroid each step.

Graphshell-side call sites (`graph::physics::apply_*_forces`,
`graph::frame_affinity::apply_frame_affinity_forces`) keep their external
signatures but delegate to the graph-canvas impls. `semantic_pair_similarity`
and `registrable_domain_key` helpers stay in `graph::physics` for the bridge
to call when building `LayoutExtras` slots each pass.

### Step 4 — Static positional layouts (partial)

`crates/graph-canvas/src/layout/static_layouts.rs`:

- `Grid` — row-major with `ceil(sqrt(n))` columns, configurable gap/origin.
- `Radial` — BFS rings from a focal node; ring `n` at radius `n ×
  ring_spacing`; unreachable nodes on an outer ring.
- `Phyllotaxis` — Fibonacci spiral (golden angle) with inward/outward
  orientations for priority-queue vs recency-ring semantics.

All three share `StaticLayoutState` with a `damping` field — `1.0` snaps
instantly, `0.2` eases in over ~20 frames.

**Deferred**: Timeline and Kanban — they need host-specific metadata
(time coordinates, status tags) which would expand `LayoutExtras` with a
currently-unused slot. Adding them is mechanical once a consumer asks.

### Step 6 — rapier2d `Layout` adapter

`crates/graph-canvas/src/layout/rapier_adapter.rs` behind the `simulate`
feature. `RapierLayout` builds a fresh `RapierSceneWorld` each step from the
current scene (ball collider per node, spring joints per edge, static bodies
for pinned nodes), steps once with configurable gravity, reads back deltas.

**Known limitation**: rebuild-per-step loses cross-frame momentum. A
persistent variant that reuses one world and syncs positions in/out each
step is a deferred optimization; the current behavior matches the pre-M2
scene-physics runtime which was also per-frame snapshot-based.

### Tests

- `cargo test -p graph-canvas --lib`: **154 passed / 0 failed** (149 with
  default features + 5 static-layout tests; +4 Barnes-Hut, +6 extras in
  prior runs counted in this total).
- `cargo test -p graph-canvas --features simulate --lib`:
  **178 passed / 0 failed**.
- `cargo test --lib -- --test-threads=1` (graphshell workspace):
  **2144 passed / 0 failed / 3 ignored**.
- Two tests flake under parallel execution in ways unrelated to this
  session's changes (corridor graphlet mask and radial sector probe).
  Flagged for a future test-hygiene pass; not regressions.

### Still deferred (follow-ons)

- **Step 5**: Penrose aperiodic tiling, L-system fractal path, UMAP-style
  semantic embedding — design-heavy; trait-implementer work once decisions
  on grammar choice and similarity space land.
- **Step 7**: WASM `Layout` adapter — needs versioned guest ABI; tracked in
  [../graph/2026-04-03_wasm_layout_runtime_plan.md](../graph/2026-04-03_wasm_layout_runtime_plan.md).
- **Timeline / Kanban** static layouts — pending a `LayoutExtras` slot for
  time/tag metadata.
- **Persistent rapier adapter** — reuse one `RapierSceneWorld` across
  frames for real momentum.
- **Plumbing-only feature re-lands** from §5.6 (background pan, wheel pan,
  pinch zoom, middle-click pan, inertia, Fit commands, overlays).
