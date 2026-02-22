# Multi-Graph Pane Plan (2026-02-22)

**Status**: Draft
**Goal**: Enable multiple independent graph view panes in the workbench, with data-driven physics profiles and advanced layout capabilities.

## Context
Currently, Graphshell supports a single graph view pane (`TileKind::Graph`). The camera state (`app.camera`) and visual state (`app.egui_state`) are singular in `GraphBrowserApp`. Users want to view different parts of the graph simultaneously (e.g., overview + detail, or two different clusters) and interact with the graph using different topological rules (e.g. tree view vs. force-directed).

This plan supersedes the "Layout: Advanced Physics and Algorithms Plan" by integrating its algorithmic improvements into the multi-view architecture.

## Architecture Changes

### 1. View Identity & Configuration
Introduce `GraphViewId` (UUID) to uniquely identify a graph viewport.
Each view needs configuration to define its environment.

```rust
struct GraphViewState {
    id: GraphViewId,
    name: String,           // User-editable label (e.g. "Main View", "Physics Debug")
    camera: Camera,         // Independent zoom/pan
    profile: PhysicsProfile, // Data-driven physics/layout configuration
    // ...
}
```

### 2. Layout Independence
While the underlying graph topology (`app.graph`) is shared, the *spatial arrangement* can differ per view.

- **Canonical Layout (Default)**: The view displays nodes at `node.position` (driven by the global physics engine).
- **Local Layout (Override)**: The view calculates its own node positions (e.g., Grid, Radial, Tree) without mutating the global `node.position`.


### 2. Topology & Physics Environments ("States of Matter")
Views can represent the graph using different "states of matter" or topological rules. These are combined layout, physics, and visual presets that decouple the *spatial simulation* from the *semantic graph*.

**Physics Presets:**
- **Gas (Expansive/Fill)**: Nodes spread out to fill the available viewport or compact down when zoomed in (density adaptation), maintaining non-overlap. High repulsion, volume-filling behavior.
- **Liquid (Pooling/Languid)**: Nodes passively and languidly reorient into energetically restful positions. Similar nodes (by domain or origin) "pool" together via attraction forces. This is the canonical organic graph behavior.
- **Solid (Structured/Stiff)**: Nodes and edges are stiff but reorientable.
  - *Standard*: Normal gravity, rigid connections.
  - *Tree*: Directional gravity (downward). Introduces a "ground" plane. Nodes (defaulting to square shape) extend off the ground, stacked or branching.
  - *Crystal*: Grid/Lattice layout with user-definable spawning rules.

**Scoping Global to View Context:**
- **Global Scope (`app.graph`)**: The source of truth for content (Nodes, Edges, Metadata) and the "Canonical" positions.
- **View Scope (`GraphViewState`)**: The lens through which the graph is seen.
  - **Canonical Topology**: Reads/writes `app.graph.positions`. Uses `app.physics`.
  - **Divergent Topology**: Owns a `LocalSimulation` (shadow copy of positions + local physics state). Used by "Gas" or "Solid" views to experiment/visualize without mutating the global map.

**Session Portability:**
- Users can switch a pane's "State of Matter" (Preset) on the fly.
- **Transition Logic**:
  - *Canonical -> Divergent*: Clone global positions to local state. Start local physics.
  - *Divergent -> Canonical*: Discard local positions (revert) OR "Commit" local positions to global.
  - *Divergent -> Divergent*: Apply new physics rules to current local positions.

**Implementation Strategy (Rendering):**
The `EguiGraphState` (adapter) currently copies positions from `app.graph`.
- If view is **Canonical**, it continues to copy from `app.graph`.
- If view is **Divergent**, it copies from `view.local_simulation.positions`.
- Physics steps run globally for `app.physics` and per-view for any active `view.local_simulation`.

### 3. App State (`app.rs`)
Refactor `GraphBrowserApp` to support multiple views:
- **Deprecate** singular `camera` and `egui_state`.
- Add `views: HashMap<GraphViewId, GraphViewState>`.
- Add `focused_view: Option<GraphViewId>`.

### 4. Tile System (`desktop/tile_kind.rs`)
Update `TileKind` to carry identity:
```rust
enum TileKind {
    Graph(GraphViewId),
    WebView(NodeKey),
}
```

### 4. Intents (`app.rs`)
Update view-dependent intents to target a specific view (or default to focused):
- `SetZoom { view: Option<GraphViewId>, zoom: f32 }`
- `RequestFitToScreen { view: Option<GraphViewId> }`
- `SetViewTopology { view: GraphViewId, topology: ViewTopology }`
- `ReheatPhysics` (global or per-view depending on topology)

### 5. Rendering (`render/mod.rs`)
- `render_graph_view` signature update to accept `GraphViewId`.
- Look up camera/state from `app.views`.
- Handle "view not found" gracefully (recreate or show error).

### 6. Input Routing
- Ensure keyboard shortcuts (zoom, fit) target the `focused_view`.
- Mouse interactions (scroll, drag) naturally target the hovered view.

## Implementation Steps

1.  **Define Types**: Add `GraphViewId`, `GraphViewState`, and `ViewTopology` (with Gas/Liquid/Solid presets) to `app.rs`.
2.  **Migration**: Temporarily keep `camera` (marked deprecated) to allow incremental migration of call sites, or do a hard break.
3.  **Tile Update**: Change `TileKind`. Fix `persistence_ops.rs` and `gui.rs`.
4.  **Intent Update**: Update `GraphIntent` and reducer.
5.  **Render Update**: Update `render/mod.rs` to use per-view state.
6.  **UI**: Add "Split Graph Pane" command and "Physics Environment" settings per pane.

## UX Considerations
- **Shared Physics**: The underlying graph topology and physics simulation remain singular (`app.graph`, `app.physics`). All views show the same simulation state.
- **Independent Cameras**: Pan/zoom are independent.
- **LOD**: Level-of-detail (labels, badges) is calculated per-view based on its zoom level.
- **Selection**: Selection is currently global (`app.selected_nodes`). This is likely desired (selecting in one view highlights in others).