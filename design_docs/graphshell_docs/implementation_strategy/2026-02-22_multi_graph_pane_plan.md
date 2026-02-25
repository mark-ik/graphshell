# Multi-Graph Pane Plan (2026-02-22)

**Status**: Active — Aligned with Registry Architecture (revised 2026-02-24)
**Supersedes**: "Layout: Advanced Physics and Algorithms Plan" (integrated here)
**See also**: `2026-02-22_registry_layer_plan.md` (authoritative on registries)

**Goal**: Enable multiple independent graph view panes in the workbench, each with its own camera and Lens, with Canonical or Divergent layout modes.

---

## Context

Graphshell currently supports a single graph view pane (`TileKind::Graph`). The camera state (`app.camera`) and visual state (`app.egui_state`) are singular in `GraphBrowserApp`. Users want to view different parts of the graph simultaneously (e.g., overview + detail, or two different clusters) and interact with the graph using different layout and physics configurations.

This plan integrates the "Advanced Physics and Algorithms" improvements into the multi-view architecture.
Registry vocabulary (`LensId`, `CanvasRegistry`, `PhysicsProfileRegistry`) is defined in `2026-02-22_registry_layer_plan.md` and used here without re-definition.

---

## Semantic Model: Graph as Workspace

Multi-pane graph views enable a workbench layout like an IDE or spatial browser:

- **Hub Pane**: A pinned graph view acting as primary navigation (analogous to a file explorer). Typically uses the `lens:file_explorer` Lens: `topology:dag`, indented-tree layout, no physics.
- **Context Pane**: A secondary view focused on the current working cluster. Typically uses `lens:research` or a custom Lens: `topology:free`, force-directed, `physics:liquid`.
- **Workspaces**: Higher-order groupings within the graph (not pane-level, graph-level).

The same underlying graph content (`app.graph`) is viewed through different lenses simultaneously. The panes are windows into one graph, not separate graphs.

---

## Architecture Changes

### 1. View Identity & Configuration

Introduce `GraphViewId` (UUID) to uniquely identify a graph viewport. Each view owns its camera and its Lens reference.

```rust
struct GraphViewState {
    id: GraphViewId,
    name: String,                    // User-editable label ("Main View", "Tree Explorer")
    camera: Camera,                  // Independent zoom/pan per view
    lens_id: LensId,                 // Points to a LensCompositor configuration
    layout_mode: ViewLayoutMode,     // Canonical or Divergent (see §2)
    local_simulation: Option<LocalSimulation>, // Some(_) only when Divergent
}

enum ViewLayoutMode {
    /// Reads node positions from app.graph (global physics). Default.
    Canonical,
    /// Owns a shadow position set + local physics step. Does not write to app.graph.
    Divergent,
}
```

`LensCompositor::resolve_lens(lens_id)` returns a composed `LensConfig` at render time: topology policy ID, layout algorithm ID, physics profile ID, theme ID, knowledge filter. These IDs are resolved against their respective registries (`CanvasRegistry`, `PhysicsProfileRegistry`, `ThemeRegistry`). The `GraphViewState` holds the ID, not the resolved config — resolution is lazy per-frame.

---

### 2. Canonical vs. Divergent Layout

The underlying graph topology (`app.graph`) is shared across all views. Spatial arrangement can differ.

**Canonical (default)**: The view reads `node.position` from `app.graph` (driven by the global physics engine). Multiple Canonical views over the same graph always show the same node positions; only camera differs.

**Divergent**: The view owns a `LocalSimulation` — a shadow copy of positions and a local physics state. The global `app.graph.positions` are cloned into the `LocalSimulation` on transition. Physics runs independently without mutating global state.

Transition semantics:

- **Canonical → Divergent**: Clone global positions into `local_simulation`. Start local physics with the new Lens's physics profile.
- **Divergent → Canonical**: Discard `local_simulation` (revert to global), or "Commit" — write local positions back to `app.graph.positions` (this is a destructive global mutation; requires explicit user confirmation in UX).
- **Divergent → Divergent** (lens change): Re-apply new physics profile to existing `local_simulation.positions`.

Use cases for Divergent: tree views, grid layouts, physics experimentation panes, and semantic projections. Projections assign positions analytically from node metadata rather than from physics: timeline (X axis = `created_at`, Y axis = domain group), kanban (columns by `status` tag, rows by priority), geospatial (lat/long from node metadata). These produce positions that have no meaningful relationship to the global force-directed layout, so they must not contaminate it.

Rendering dispatch in `EguiGraphState`:

- If `Canonical`: copy positions from `app.graph` (existing behavior).
- If `Divergent`: copy positions from `view.local_simulation.positions`.

Physics tick dispatch:

- Global `app.physics` ticks once per frame regardless of view count.
- Each Divergent view's `local_simulation` ticks independently (potentially at a different physics profile / energy level).

---

### 3. Physics Profile Presets ("States of Matter")

`PhysicsProfileRegistry` holds the following seed floor profiles (defined in `2026-02-22_registry_layer_plan.md §Core Seed Floor`). They are referenced by ID from any Lens:

| ID | Label | Behavior |
| --- | --- | --- |
| `physics:liquid` | Liquid | Organic clustering, languid motion. Nodes pool by domain/origin via attraction forces. Default for the main graph view. |
| `physics:gas` | Gas | High repulsion, volume-filling. Nodes spread to fill the viewport; density-adaptive. Useful for overview panes. |
| `physics:solid` | Solid | Stiff connections, directional gravity. Used with tree/grid layouts where position is structurally determined. |

These are *parameter presets only*. Physics engine execution and force profile integration are governed by `CanvasRegistry` (Layout Domain). A Lens composes a physics ID from this registry with a layout algorithm ID from `CanvasRegistry` — they are independent axes.

---

### 4. App State (`app.rs` / `GraphWorkspace`)

Refactor `GraphWorkspace` (post-Phase 6.1) to support multiple views:

- **Deprecate** singular `camera` and `egui_state`. Keep as `#[deprecated]` aliases pointing to `focused_view`'s state during migration.
- Add `views: HashMap<GraphViewId, GraphViewState>` to `GraphWorkspace`.
- Add `focused_view: Option<GraphViewId>` to `GraphWorkspace`.
- On startup, create a default Canonical view with `lens_id: "lens:default"`.

---

### 5. Tile System (`desktop/tile_kind.rs`)

Update `TileKind` to carry view identity:

```rust
enum TileKind {
    Graph(GraphViewId),   // was: Graph (no identity)
    WebView(NodeKey),
}
```

This makes it possible for two tiles to show different views of the same graph. Persistence (`persistence_ops.rs`) must serialize `GraphViewId` alongside tile kind.

---

### 6. Intents (`app.rs`)

Update view-dependent intents to target a specific view, defaulting to `focused_view` when `None`:

```rust
SetZoom { view: Option<GraphViewId>, zoom: f32 }
RequestFitToScreen { view: Option<GraphViewId> }
SetViewLens { view: GraphViewId, lens_id: LensId }
SetViewLayoutMode { view: GraphViewId, mode: ViewLayoutMode }
CommitDivergentLayout { view: GraphViewId }   // writes local positions to app.graph
ReheatPhysics { view: Option<GraphViewId> }   // None = global; Some = local_simulation only
```

`SetViewTopology` is removed. Topology policy is a component of the Lens, changed via `SetViewLens`.

---

### 7. Rendering (`render/mod.rs`)

- `render_graph_view` accepts `GraphViewId`. Looks up `GraphViewState` from `app.workspace.views`.
- Resolves `LensConfig` via `services.lens_compositor.resolve_lens(view.lens_id)`.
- Dispatches layout algorithm, topology policy, interaction/rendering policy from the resolved config.
- Handles "view not found" gracefully: log warning, render empty pane with error message (do not panic).

---

### 8. Input Routing

- Keyboard shortcuts (zoom, fit, lens switch) target `focused_view`.
- Mouse scroll/drag naturally target the hovered view via egui's widget hit-testing.
- `focused_view` updates on mouse-enter of a graph pane (hover focus) or click (click focus). Make this a preference.

---

## Implementation Steps

1. **Define types**: Add `GraphViewId`, `GraphViewState`, `ViewLayoutMode` to `app.rs`. `LensId` is already available from the registry layer.
2. **Migrate singular state**: Hard-break `camera` and `egui_state` — no deprecated aliases unless the call-site count makes it impractical. Replace with `views: HashMap<GraphViewId, GraphViewState>` and `focused_view`.
3. **Tile update**: Change `TileKind::Graph` to carry `GraphViewId`. Update `persistence_ops.rs` and `gui.rs`.
4. **Intent update**: Update `GraphIntent` with the new view-targeted variants. Update the reducer.
5. **Render update**: Update `render/mod.rs` to accept `GraphViewId` and perform per-view state lookup and Lens resolution.
6. **UI**: Add "Split Graph View" command (creates a new `GraphViewId` with `lens:default`). Add a Lens selector per pane (shows names from `LensCompositor`'s registered lenses). Add "Commit Layout" action for Divergent views.

Steps 1–4 should be done as a single hard-break migration (no production users; no compat shims needed).

---

## UX Considerations

**Shared topology, independent cameras**: `app.graph` is the single source of truth for nodes, edges, and metadata. Two Canonical panes always show the same positions; only zoom/pan differs.

**Divergent panes are explicit**: The user must consciously switch a pane to Divergent mode (e.g., "Use Local Layout"). There is no implicit divergence. The pane shows a badge or indicator when in Divergent mode.

**LOD is per-view**: Level-of-detail (labels, badges, edge detail) is calculated per-view based on that view's zoom level. Two panes at different zoom levels render different detail.

**Selection is global**: `app.workspace.selected_nodes` is shared. Selecting a node in one pane highlights it in all panes. This is the correct default for a single-graph multi-view system.

**Physics budget**: With N Divergent views each running a `LocalSimulation`, per-frame physics cost scales with N. Each `LocalSimulation` should auto-pause when its energy threshold is met (governed by `CanvasRegistry`'s physics execution policy). Global physics and per-view simulations should be on separate budgets.
