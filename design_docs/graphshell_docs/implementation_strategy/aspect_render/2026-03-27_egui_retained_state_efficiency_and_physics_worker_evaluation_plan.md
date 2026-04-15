# egui Retained-State Efficiency, Per-View Isolation, and Physics Worker Evaluation Plan

**Date**: 2026-03-27
**Status**: Design / planning
**Primary hotspot**: `render/mod.rs`
**Related**:

- `2026-03-27_physics_spike_metrics.md`
- `2026-03-08_render_mod_decomposition_plan.md`
- `frame_assembly_and_compositor_spec.md`
- `../graph/multi_view_pane_spec.md`
- `../graph/GRAPH.md`
- `../workbench/workbench_layout_policy_spec.md`
- `../shell/SHELL.md`
- `../../technical_architecture/unified_view_model.md`
- `../../../archive_docs/checkpoint_2026-03-05/2026-03-05_camera_navigation_fix_postmortem.md`

---

## 1. Problem Statement

The current graph render path is paying the highest cost in the wrong place.

The visible simulation jitter and velocity-loss bugs are not primarily evidence that the
physics step must move off-thread. They are primarily evidence that Graphshell rebuilds
retained `egui_graphs` state too often for changes that are only visual projections.

Today:

- structural graph mutations and visual-only state changes both flow through
  `egui_state_dirty`
- full `EguiGraphState::from_graph_with_memberships_projection(...)` rebuilds reset
  retained FR velocity
- workbench-hosted graph panes are already multi-view in architecture, but the retained
  runtime state this path mutates is still largely singleton state

This plan corrects those boundaries before any worker implementation is considered.

---

## 2. Architecture Correction

### 2.1 Graphshell remains authoritative; `egui_graphs` is a projection target

`egui_graphs` has two roles in this codebase:

1. projection target for node/edge/selection/badge/crash/theme state
2. event source emitting `GraphAction` back to Graphshell

Graphshell owns semantic truth. `egui_graphs` must not become authoritative over
selection, lifecycle, or long-lived physics ownership.

### 2.2 The under-used retained path already exists

Two existing seams show the right direction:

- `model/graph/egui_adapter.rs::sync_from_delta(...)` handles some structural changes
  incrementally
- `render/canvas_visuals.rs` already uses `node_mut()` / `edge_mut()` for visual updates

The problem is inconsistent use: selection, badges, crash flags, and theme changes still
fall through to full rebuilds.

### 2.3 `GraphViewId` is the runtime boundary

The key architectural correction is that the runtime boundary here is not the workspace
singleton. It is `GraphViewId`.

The surrounding codebase and specs already treat these as per-view concerns:

- selection
- camera
- layout/physics state
- multi-pane graph hosting in Workbench

But the fields driving the retained egui graph path are still singleton-oriented:

- `egui_state`
- `egui_state_dirty`
- `last_culled_node_keys`
- layout/metadata slot use via `set_layout_state(..., None)`,
  `get_layout_state(..., None)`, and `graph_view_metadata_id(None)`

If this plan only splits dirty flags without first moving retained runtime ownership to
`GraphViewId`, it will optimize the wrong boundary and violate the multi-view/workbench
contract.

Therefore:

- Graph owns durable truth
- each `GraphViewId` owns ephemeral retained graph runtime state
- Shell and Workbench chrome state must not piggyback on graph rebuild flags

### External pattern note (2026-04-01): RustGrapher / WasmGrapher

Reviews of RustGrapher and WasmGrapher support the ordering in this plan. The useful lesson is not "move physics off-thread first"; it is "separate simulation ownership from widget-retained state first". Both projects validate force-directed simulation as a reusable plain-data engine and keep acceleration structures such as Barnes-Hut or quadtree indexing as engine concerns rather than UI-state concerns.

For Graphshell this reinforces the existing sequencing:

- per-`GraphViewId` retained-state isolation first,
- single-owner physics second,
- worker evaluation third.

A worker remains conditional. Acceleration remains a legitimate follow-on once `EguiGraph` is no longer the cross-frame authority for velocity and positions.

---

## 3. Target Runtime Shape

Replace the single workspace-global rebuild flag with a per-view runtime carrier.

Illustrative shape:

```rust
pub(crate) struct GraphViewRenderRuntime {
    pub egui_state: Option<EguiGraphState>,
    pub structural_dirty: bool,
    pub visual_projection_dirty: VisualProjectionFlags,
    pub subgraph_dirty: bool,
    pub last_culled_node_keys: Option<HashSet<NodeKey>>,
    pub metadata_custom_id: Option<String>,
}
```

Dirty-classification rules:

- `structural_dirty`
  Full projection shape changed for that view and requires
  `from_graph_with_memberships_projection(...)`.
  Examples: structural delta fallback, persistence restore, lifecycle restore/tombstone,
  edge-projection changes that alter the rendered edge set.

- `visual_projection_dirty`
  Rendered attributes changed in place and should be applied via `node_mut()` /
  `edge_mut()` without rebuild.
  Examples: selection, badges, crash, theme, and any future pure highlight styling.

- `subgraph_dirty`
  The rendered slice changed without a durable topology mutation.
  Examples: filter mode, facet filtering, viewport culling-set changes, graphlet/frame
  scoping changes.

Explicit non-bucket:

- shell/workbench chrome state is not a graph projection dirty class and must not set any
  of the three flags above

---

## 4. Call-Site Audit Rule

This work must not proceed by converting only the obvious selection sites and then stopping.

Every current `egui_state_dirty = true` writer should be classified into one of:

1. `structural_dirty`
2. `visual_projection_dirty`
3. `subgraph_dirty`
4. not a graph projection concern at all

Known hotspots to classify explicitly:

- `app/focus_selection.rs`
- `app/graph_views.rs`
- graph search UI/orchestration in `shell/desktop/ui/gui/*`
- `render/graph_info.rs`
- workbench routing/focus transitions that currently bounce through graph rebuilds

This audit is part of the plan, not optional follow-up cleanup.

---

## 5. Ordered Phases

### Phase 0. Per-`GraphViewId` runtime ownership and dirty audit

**Problem**

Multi-view graph panes are supported, selection and camera are already per view, but the
retained egui graph runtime is still singleton state.

This creates two risks:

- optimization lands on the wrong boundary
- later per-view isolation work has to undo Phase A/B/C field placement

The 2026-03-05 camera postmortem is the key receipt: once custom metadata/layout ids are
used, every participating egui_graphs consumer must use the exact same id or the layout
state either restarts or writes to dead slots.

**Change**

Add a per-view render-runtime carrier keyed by `GraphViewId` and move into it:

- `egui_state`
- dirty flags
- `last_culled_node_keys`
- metadata/layout slot identity

Interim compatibility rule:

- if physics ownership remains workspace-global until Phase C, document that explicitly as
  temporary
- view-local projection/rebuild state still moves in Phase 0

**Acceptance criteria**

- two graph panes may be open without sharing rebuild flags
- culling cache is tracked per `GraphViewId`
- `GraphView`, camera metadata lookup, lasso state, `set_layout_state`, and
  `get_layout_state` all use the same per-view slot identity
- later phases refer to per-view runtime storage rather than workspace-global fields

### Phase A. Stop rebuilding egui state for selection-only changes

**Problem**

`app/focus_selection.rs` currently dirties the entire retained egui graph path at eight
high-frequency call sites. Selection changes are visual-only and should not discard FR
velocity.

**Change**

Add:

```rust
bitflags::bitflags! {
    pub(crate) struct VisualProjectionFlags: u8 {
        const SELECTION = 0b0001;
        const BADGES    = 0b0010;
        const CRASH     = 0b0100;
        const THEME     = 0b1000;
    }
}
```

Convert selection updates from workspace-global rebuilds to per-view:

- `visual_projection_dirty |= SELECTION`
- no structural rebuild flag

In the render path, add an incremental projection pass after any full rebuild:

```rust
if graph_view_runtime
    .visual_projection_dirty
    .contains(VisualProjectionFlags::SELECTION)
{
    if let Some(state) = graph_view_runtime.egui_state.as_mut() {
        project_selection_incremental(state, &view_selection);
    }
    graph_view_runtime.visual_projection_dirty
        .remove(VisualProjectionFlags::SELECTION);
}
```

Important detail:

- focused-view swaps must reproject selection for both the old and new view

**Acceptance criteria**

- selecting/deselecting does not call
  `from_graph_with_memberships_projection(...)`
- FR velocity is preserved across selection changes
- selection styling matches `SelectionState` in the same frame
- focused-view swaps do not force workspace-global rebuilds

### Phase B. Incremental projection for badges, crash, and theme

**Problem**

Other visual-only changes still trigger full rebuilds:

- crash state
- semantic badges
- theme application

**Change**

Use the same `VisualProjectionFlags` path for:

- `CRASH`
- `BADGES`
- `THEME`

Implement:

- `project_crash_incremental(...)`
- `project_badges_incremental(...)`
- theme application outside the full-rebuild path

Also separate pure shell/workbench chrome state from graph projection:

- graph-search pin/unpin should not dirty graph projection unless it changes filter/highlight
- shell overview visibility and diagnostics-surface visibility must never trigger graph rebuild

**Acceptance criteria**

- crash changes do not reset other nodes' FR velocity
- badge changes do not reset FR velocity
- theme switches do not reset FR velocity
- no full rebuild occurs for these paths when `structural_dirty == false`

### Phase C. Single-owner physics state

**Problem**

Physics state is currently dual-owned:

1. app-owned copy
2. egui_graphs-owned retained copy in egui memory

Each frame clones app state into egui, runs the widget-driven step, then copies it back
out and applies extension forces after the widget step. That is a dual-write lifecycle.

**Change**

Make app-owned runtime the single owner of physics parameters and velocity state, per
`GraphViewId`. `egui_graphs` becomes a render target for final positions.

Direction:

- move velocity into app-owned per-view runtime storage
- compute the FR step in app-owned state
- apply extension forces before writing positions into `EguiGraph`
- call `GraphView::add()` with layout stepping disabled

This is the prerequisite for any future worker because it removes `EguiGraph` from the
simulation ownership path.

**Acceptance criteria**

- physics parameters have one write path per view runtime
- FR velocity has one write path per view runtime
- `set_layout_state` / `get_layout_state` are no longer called each frame
- extension forces apply before writing positions into egui state
- advancing pane A does not mutate pane B's layout state

### Phase D. Baseline benchmarks

Run after Phase A, again after Phase B, and again after Phase C.

Measurement protocol is defined in:

- `2026-03-27_physics_spike_metrics.md`

Metrics must carry at least:

- `GraphViewId`
- reason
- workbench/surface context where applicable

Metric families:

- rebuild rate
- velocity-loss rate
- physics-step frame budget

Decision gate:

- if p99 physics step is below 1 ms at N=500 after Phases A-C, a worker is not justified

### Phase E. Physics worker, conditional only

Only proceed if the Phase D gate says the synchronous app-owned step is still expensive
enough to justify the complexity.

If built:

- use `spawn_blocking`
- copy only `Send`-safe per-view position/velocity data
- stage results back before any structural rebuild consumes them
- key scheduling and staging by `GraphViewId`

**Acceptance criteria**

- synchronous vs worker position divergence stays below epsilon
- worker path never resets FR velocity
- hidden/occluded graph panes may defer work without corrupting visible panes

---

## 6. Cross-Stack Integration Requirements

### 6.1 Workbench integration

- retained graph runtime state must follow `GraphViewId`, not pane position
- moving/splitting/reordering a graph pane must not reset its layout state
- viewport culling and subgraph invalidation must respect visible navigation geometry from
  Workbench layout policy
- hidden or fully occluded graph panes may coalesce updates, but their per-view dirty
  state must be preserved

### 6.2 Shell integration

- rebuild, subgraph invalidation, and velocity-reset diagnostics should surface both in
  Diagnostics Inspector and Shell overview attention surfaces
- pure shell chrome changes must not reuse graph rebuild flags
- diagnostic payloads should be rich enough for shell attention summaries:
  `GraphViewId`, reason, and hosting context

### 6.3 Focus and accessibility integration

- `focused_view` transitions should dirty only the affected views
- UxTree / Graph Reader / accessibility projection must observe the same selected/focused
  target in the same frame as the visual update
- the optimized path must not reintroduce focus-repair-click behavior for pane-hosted
  graph views

---

## 7. Key Ownership Receipts

| Question | Answer |
| --- | --- |
| Who owns node positions between frames today? | `egui_graphs::Graph` retained in egui state; `Node::projected_position()` is authoritative only at rebuild and drag-end writeback. |
| Who owns physics state between frames today? | Dual ownership: app runtime + egui memory layout state. |
| When do extension forces apply today? | After the FR step in the same frame, writing directly into `EguiGraph` node positions. |
| When do positions flow back to durable node position? | At drag-end only. |

These receipts justify the ordering of the plan: retained-state correctness first, single
physics ownership second, worker evaluation last.

---

## 8. Key Files

| File | Role |
| --- | --- |
| `app/focus_selection.rs` | selection dirtying and focused-view transitions |
| `app/workspace_state.rs` | current singleton graph runtime fields |
| `app/graph_views.rs` | view-scoped state and `GraphViewId` alignment |
| `render/mod.rs` | rebuild path, culling, layout state round-trip, physics step |
| `render/canvas_visuals.rs` | existing incremental visual update pattern |
| `render/graph_info.rs` | shell/chrome state currently leaking into graph rebuilds |
| `model/graph/egui_adapter.rs` | retained-state projection and incremental delta sync |
| `graph/layouts/graphshell_force_directed.rs` | FR step separability receipt |
| `../graph/multi_view_pane_spec.md` | per-view runtime authority receipt |
| `../workbench/workbench_layout_policy_spec.md` | visible navigation geometry contract |
| `../shell/shell_overview_surface_spec.md` | shell attention/diagnostics surfacing |

---

## 9. Verification

```sh
cargo test
# Phase 0: open two graph panes and verify retained runtime state is isolated per GraphViewId
# Phase A: selection change preserves FR velocity without rebuild
# Phase D: gather release-mode diagnostics for rebuild rate and physics-step timing
```

---

## 10. Cross-Stack Integration Requirements

*These integrations are explicit done gates, not incidental cleanup.*

### Workbench Integration

- Retained egui runtime state must follow `GraphViewId`, so moving/splitting/reordering a
  graph pane in Workbench does not reset its layout state.
- Viewport culling and subgraph invalidation must respect **visible navigation geometry**
  from Workbench layout policy, not a single global canvas rect.
- Hidden or fully occluded graph panes may coalesce physics/visual updates until visible
  again, but their dirty flags and per-view runtime state must be preserved.

### Shell Integration

- Diagnostics for rebuilds, subgraph invalidations, and velocity resets should surface in
  both the Diagnostics Inspector and Shell overview attention surfaces.
- Pure shell chrome mutations (search pinning, overview visibility, command-surface
  toggles) must not reuse graph rebuild flags.
- Any new diagnostic event should carry enough context for Shell to present an actionable
  summary: `GraphViewId`, reason, and whether the view is graph-first or pane-hosted.

### Focus / Accessibility Integration

- `focused_view` transitions should dirty only the affected views' selection projection,
  not force a workspace-global rebuild.
- UxTree / Graph Reader / accessibility projection must observe the same selected/focused
  graph target in the same frame the visual projection updates.
- The optimized path must not reintroduce "focus repair click" behavior for pane-hosted
  graph views.

---

## 11. Ownership Receipts (Spike Stage 2, 2026-03-27)

| Question | Answer |
| -------- | ------ |
| Who owns node positions between frames? | `egui_graphs::Graph` (egui persistent storage, keyed by widget ID). Canonical `Node::projected_position()` is authoritative only at rebuild (`EguiGraphState::from_graph()`) and at drag-end (`GraphAction::DragEnd` → `ViewAction::SetNodePosition`, `render/mod.rs:926–928`). |
| Who owns physics state between frames? | Dual ownership. App: `app.workspace.graph_runtime.physics` (`app/workspace_state.rs:226`). egui_graphs: FR state in egui memory via `set_layout_state` (`render/mod.rs:561–568`). Each frame: clone app → lens-modify → write into egui; after widget step, read back via `get_layout_state` (`render/mod.rs:659–661`) and overwrite app copy. |
| When do extension forces apply? | After the FR step, same frame — `apply_graph_physics_extensions` at `render/mod.rs:670`. Writes into `EguiGraph` node positions directly. |
| When does physics pause? | On `GraphIntent::SetInteracting{true}` (`GraphAction::DragStart`, `render/mod.rs:923–924`), handled in `graph_app.rs:1514–1519`. Also when `dynamic_layout` is false (`render/mod.rs:550–551`). |
| When do positions flow back to `Node::position`? | Only at drag-end: `GraphAction::DragEnd` → `ViewAction::SetNodePosition` (`render/mod.rs:926–928`). Not continuously. |
