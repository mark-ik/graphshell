# Pane-Hosted Multi-View Plan (formerly "Multi-Graph Pane Plan") (2026-02-22)

**Status**: Active — Scope expanded and aligned with registry/viewer architecture (revised 2026-02-25)
**Supersedes**: "Layout: Advanced Physics and Algorithms Plan" (integrated here) and the earlier graph-only framing of this document
**See also**:
- `2026-02-22_registry_layer_plan.md` (registry authority and terminology)
- `2026-02-23_wry_integration_strategy.md` (overlay viewer constraints)
- `2026-02-24_universal_content_model_plan.md` (viewer selection / content types)
- `2026-02-24_performance_tuning_plan.md` (culling / LOD / frame budgets)
- `2026-02-18_graph_ux_research_report.md` (layout quality and interaction research)

**Goal**: Treat the workbench pane as a universal host for view surfaces (graph views, Servo/Wry webviews, native content viewers, and tool panes), while preserving graph-specific multi-view features (independent cameras, per-pane Lens, Canonical/Divergent layouts).

---

## Context

The original version of this plan correctly identified the need for multiple independent graph
view panes (`GraphViewId`, per-view camera, per-view Lens), but it scoped the problem too narrowly.

Graphshell now has converging plans for:

- multiple graph views with independent layout semantics
- multiple viewer backends (Servo texture, Wry overlay)
- native non-web viewers (text/image/pdf/audio/directory)
- diagnostic/history/accessibility tool surfaces in panes

All of these are pane-hosted views with shared workbench behavior (splitting, tabs, focus, resize,
visibility, persistence). The pane system should therefore own a **generic pane-view model**, with
graph views modeled as one specific pane view kind.

---

## Core Principle: Panes Are Hosts, Views Are Payloads

The workbench should not special-case "graph pane" vs "webview pane" at the layout layer.
It should host a pane whose payload determines rendering and input behavior.

### Pane Categories (authoritative conceptual model)

1. **Graph Pane**
   - Renders a graph viewport.
   - Uses `GraphViewState` (`GraphViewId`, camera, Lens, Canonical/Divergent state).
   - Governed by `CanvasRegistry` + `LensCompositor`.

2. **Node Viewer Pane**
   - Renders a node using the selected viewer backend (`viewer:servo`, `viewer:wry`, `viewer:plaintext`, `viewer:pdf`, etc.).
   - Viewer selection is delegated to `ViewerRegistry` (using node metadata + overrides).
   - Viewport behavior is governed by `ViewerSurfaceRegistry`.

3. **Tool Pane**
   - Diagnostics, History Manager, Accessibility Inspector, Settings, etc.
   - Not tied to `NodeKey`.
   - Uses workbench pane mechanics, but tool-specific content dispatch.

This keeps the workbench layer backend-agnostic and aligns with `GRAPHSHELL_AS_BROWSER.md`:
semantic truth in graph/intents, tile tree as layout authority, viewers reconciled as runtime instances.

---

## Semantic Model: One Graph, Many Pane Projections

The underlying graph content remains shared (`GraphWorkspace.graph`). Panes provide different
projections and interaction surfaces over that shared data.

Examples:

- **Hub Graph Pane**: `lens:file_explorer` (`topology:dag`, tree layout, solid/static physics)
- **Context Graph Pane**: `lens:research` (`topology:free`, force-directed, `physics:liquid`)
- **Node Viewer Pane**: Active node opened in Servo or Wry (depending on viewer policy)
- **Directory Viewer Pane**: `file://` node rendered via `viewer:directory`
- **Tool Pane**: History Manager or Diagnostics Inspector

Multiple graph panes are still a primary target. The difference is that they now fit into the same
pane-hosted architecture as all other surface types instead of being a one-off exception.

---

## Architecture Changes

### 1. Pane Identity and View Payload

Introduce a pane-hosted view payload model. Names may vary in code, but the separation must hold:

```rust
struct PaneState {
    id: PaneId,
    name: String,
    view: PaneViewState,
}

enum PaneViewState {
    Graph(GraphPaneRef),
    Node(NodePaneState),
    Tool(ToolPaneState),
}

struct GraphPaneRef {
    graph_view_id: GraphViewId,
}

struct NodePaneState {
    node: NodeKey,
    // Optional explicit override; canonical selection still comes from ViewerRegistry.
    viewer_id_override: Option<ViewerId>,
}

enum ToolPaneState {
    Diagnostics,
    HistoryManager,
    AccessibilityInspector,
    Settings,
    // extensible
}
```

`GraphViewState` remains a graph-specific concept (see next section). It should not be overloaded
to represent non-graph pane views.

### 2. Graph View Identity & Configuration (Graph Pane Payload)

This is the retained core from the original plan.

```rust
struct GraphViewState {
    id: GraphViewId,
    name: String,                    // User-editable label ("Main View", "Tree Explorer")
    camera: Camera,                  // Independent zoom/pan per graph view
    lens_id: LensId,                 // Resolved lazily via LensCompositor
    layout_mode: ViewLayoutMode,     // Canonical or Divergent
    local_simulation: Option<LocalSimulation>, // Present only for Divergent
}

enum ViewLayoutMode {
    Canonical,
    Divergent,
}
```

`GraphViewState` is used only when `PaneViewState::Graph(..)` is active.

### 3. Canonical vs Divergent Layout (Graph Pane Only)

The underlying graph topology is shared. Spatial arrangement may differ per graph pane.

**Canonical (default)**:
- Reads node positions from shared graph state (global physics/manual layout).
- Multiple Canonical graph panes show the same positions, different cameras.

**Divergent**:
- Owns a `LocalSimulation` shadow position set and local physics/layout state.
- Does not mutate shared graph positions unless explicitly committed.

Transition semantics:

- **Canonical -> Divergent**: clone shared positions into `local_simulation`; start local physics with the pane's Lens profile.
- **Divergent -> Canonical**: discard local simulation, or explicit **Commit** writes local positions back to shared graph positions.
- **Divergent -> Divergent** (Lens change): keep positions, re-apply physics/layout config.

Use cases:
- timeline projection
- kanban projection
- geospatial projection
- layout experimentation / compare-and-commit workflows

These are graph-pane features, not workbench features.

### 4. Viewer Panes and Backend Constraints (Servo / Wry / Native Viewers)

Viewer panes host node content selected by `ViewerRegistry`.

Important distinction (must remain explicit):

- **Servo** (`viewer:servo`) = texture mode, embeddable in graph canvas and panes
- **Wry** (`viewer:wry`) = overlay mode, pane-only (stable rectangular workbench regions)
- **Native egui viewers** (`viewer:plaintext`, `viewer:image`, `viewer:pdf`, etc.) = embedded pane renderers

Consequences:

- A pane hosts the node viewer surface, not "a backend-specific tile type."
- Graph canvas rendering of Wry-backed nodes remains thumbnail/placeholder fallback.
- Overlay tracking is a pane/compositor concern, not a graph-pane concern.

### 5. Tile/Persistence Model

The existing `TileKind` representation should evolve toward pane identity carrying a generic pane
payload, even if migration happens in stages.

Interim-compatible direction:

```rust
enum TileKind {
    Pane(PaneId),
    // transitional variants may exist during migration
}
```

Persistence must serialize:

- pane identity (`PaneId`)
- pane view payload (`Graph`, `Node`, `Tool`)
- graph-pane references (`GraphViewId`) where applicable

This avoids proliferating content-specific tile variants (for example legacy `TileKind::WebView` / `TileKind::History`) now that pane identity and pane payload carry the semantic type.

### 6. Intent Model (View- and Pane-Targeted)

Retain graph-view-targeted intents for graph panes:

```rust
SetZoom { view: Option<GraphViewId>, zoom: f32 }
RequestFitToScreen { view: Option<GraphViewId> }
SetViewLens { view: GraphViewId, lens_id: LensId }
SetViewLayoutMode { view: GraphViewId, mode: ViewLayoutMode }
CommitDivergentLayout { view: GraphViewId }
ReheatPhysics { view: Option<GraphViewId> }   // None = global canonical physics
```

Add pane-level intents for generic workbench actions:

```rust
SplitPane { source_pane: PaneId, direction: SplitDirection }
SetPaneView { pane: PaneId, view: PaneViewState }
OpenNodeInPane { node: NodeKey, pane: PaneId }
OpenToolPane { kind: ToolPaneKind }
```

Topology policy changes remain Lens-driven (`SetViewLens`), not separate graph-pane topology intents.

### 7. Rendering and Compositor Dispatch

Render dispatch should switch on pane view payload, not on hardcoded tile kind assumptions.

Pseudo-dispatch:

```rust
match pane.view {
    PaneViewState::Graph(ref graph_ref) => render_graph_pane(graph_ref.graph_view_id, ...),
    PaneViewState::Node(ref node_pane)  => render_node_viewer_pane(node_pane.node, ...),
    PaneViewState::Tool(ref tool_pane)  => render_tool_pane(tool_pane, ...),
}
```

Graph pane rendering:
- resolves Lens via `LensCompositor`
- applies `CanvasRegistry` / `PhysicsProfileRegistry`
- uses graph-pane camera and Canonical/Divergent state

Node viewer pane rendering:
- resolves viewer via `ViewerRegistry` (`mime_hint`, `address_kind`, overrides)
- dispatches embedded vs overlay flow through viewer trait contract
- applies `ViewerSurfaceRegistry` viewport policy

Tool pane rendering:
- pure egui/tool-specific rendering (diagnostics, history, a11y inspector, etc.)

### 8. Input Routing (Pane-Generic Invariant)

This plan adopts the interaction invariant from `2026-02-23_graph_interaction_consistency_plan.md`:

- **Hover activates scroll target**
- **Scroll goes to hovered pane**
- **Keyboard goes to focused pane (last-clicked)**

Graph panes interpret scroll as zoom (configurable).
Viewer panes interpret scroll as content scroll.
Tool panes interpret scroll according to the tool surface.

This routing must operate on pane identity first, then delegate to pane view payload behavior.

---

## Layout Research Alignment (Why This Architecture Matters for UX Quality)

The graph UX research and performance plan add constraints that directly affect the pane design.

### 1. Mental Map Preservation Requires Per-Graph-View State

`2026-02-18_graph_ux_research_report.md` identifies mental-map preservation as critical for
incremental graph growth. Independent graph panes need:

- independent cameras
- explicit Canonical/Divergent modes
- non-destructive commit semantics for divergent layouts

Without graph-view identity, multi-pane comparison degrades into global camera fighting.

### 2. LOD and Culling Are Per Graph Pane, Not Global

Zoom-adaptive labels, label occlusion culling, and edge LOD depend on the pane's camera zoom and
viewport (`2026-02-24_performance_tuning_plan.md`, `2026-02-18_graph_ux_research_report.md`).

Two graph panes at different zoom levels must render different detail levels at the same time.

### 3. Physics Budget Must Be Split Across Global + Divergent Simulations

Global canonical simulation and each divergent graph pane simulation need separate budgets and
auto-pause behavior. Research-backed UX rule: users perceive non-settling layouts as broken if
they fail to stabilize within ~2-3 seconds for typical graphs.

Implication:
- global canonical physics budget and per-divergent local budgets must be tracked separately
- divergent simulations should auto-pause independently

### 4. New Node Placement and Reheat Semantics Must Respect Graph Pane Expectations

Research and layout behavior plans both call out:
- reheat on structural change
- spawn near semantic parent

These should affect canonical shared layout behavior, while divergent panes choose whether to mirror
the change immediately or only on next local recompute.

---

## Implementation Steps (Revised)

This is now a staged migration from graph-only pane assumptions to pane-hosted multi-view dispatch.

### Stage 1: Pane View Payload Model (Architecture Slice)

1. Define `PaneId`, `PaneViewState`, and graph/node/tool pane payload types.
2. Add pane payload persistence shape (even if only Graph/Node are initially wired).
3. Keep compatibility shims if needed, but make new dispatch code operate on pane view payloads.

**Done gate**: Workbench layout/render paths can dispatch by pane view kind without backend-specific tile assumptions.

### Stage 2: Graph Multi-View Hard-Break Migration

1. Define `GraphViewId`, `GraphViewState`, `ViewLayoutMode`.
2. Replace singular graph camera/egui state with per-view storage and `focused_view`.
3. Update graph-pane rendering to require `GraphViewId`.

**Done gate**: Two graph panes can render the same graph with independent cameras.

### Stage 3: Tile/Persistence Migration to Pane Identity

1. Move `TileKind` toward pane identity (`Pane(PaneId)` target shape).
2. Persist pane payloads and graph-view references.
3. Update restore/load paths.

**Done gate**: Snapshot roundtrip preserves graph panes, node panes, and graph-view associations.

### Stage 4: Viewer Pane Unification (Servo / Wry / Native)

1. Route node pane rendering through `ViewerRegistry` selection + viewer trait dispatch.
2. Integrate overlay tracking for Wry viewer panes.
3. Preserve thumbnail fallback for overlay viewers shown in graph canvas.

**Done gate**: Node pane can host either Servo or Wry (feature-gated) via the same pane model.

### Stage 5: Tool Pane Registration and Dispatch

1. Add `ToolPaneState` variants for Diagnostics and History Manager first.
2. Add workbench actions to open/split tool panes.
3. Keep tool pane rendering isolated from graph/viewer rendering paths.

**Done gate**: Diagnostics and History Manager open as first-class panes (not ad hoc windows only).

### Stage 6: Pane UX Commands

1. "Split Graph View" (new `GraphViewId`, default Lens)
2. "Open Node in Viewer Pane"
3. "Open Tool Pane"
4. Per-graph-pane Lens selector and Canonical/Divergent toggle
5. Divergent "Commit Layout" action with explicit confirmation

**Done gate**: User can create, split, and operate on graph, viewer, and tool panes with consistent focus semantics.

---

## UX Considerations (Updated)

### Shared Graph Truth, Multiple Surface Types

- `GraphWorkspace.graph` is the single authority for nodes, edges, and metadata.
- Graph panes are projections over the graph.
- Viewer panes are focused content renderers for nodes.
- Tool panes are workspace utilities and diagnostics.

### Wry Is Pane-Only by Design

- Wry overlay viewers are valid only in stable workbench pane rectangles.
- Graph canvas shows thumbnail/placeholder for Wry-backed nodes.
- This is a rendering-mode constraint, not a UX inconsistency.

### Selection Defaults

- Graph node selection remains global by default (`selected_nodes` shared) so graph panes stay in sync.
- Viewer-pane focus is pane-local (which pane is active), but node activation still routes through graph/intents.

### LOD and Performance Budgets

- LOD decisions are per graph pane.
- Overlay sync is per viewer pane.
- Diagnostics and tool panes should not starve graph or viewer rendering loops.

### Accessibility

- Pane focus/hover routing must be explicit so keyboard and screen-reader users can navigate regions predictably.
- Tool panes (Diagnostics, Accessibility Inspector) benefit from the same pane model and focus semantics.

---

## Risks and Mitigations

**Risk**: Over-generalizing pane model slows delivery of multi-graph UI.
- **Mitigation**: Land Stage 1 as a minimal payload model, then immediately execute Stage 2 (graph multi-view hard break).

**Risk**: Pane payload persistence churn breaks snapshot compatibility during migration.
- **Mitigation**: Use explicit versioning or transitional deserialization adapters; keep migrations mechanical and test-backed.

**Risk**: Wry overlay logic leaks into graph-pane rendering paths.
- **Mitigation**: Keep overlay dispatch exclusively in node viewer pane / compositor paths.

**Risk**: Per-pane budgets become implicit and hard to tune.
- **Mitigation**: Make graph physics, viewer overlay sync, and diagnostics refresh budgets separately observable in diagnostics tooling.

---

## Progress

### 2026-02-22

- Original graph-only multi-pane plan created (`GraphViewId`, Canonical/Divergent concept).

### 2026-02-24

- Aligned terminology with registry architecture (`LensId`, `CanvasRegistry`, `PhysicsProfileRegistry`).

### 2026-02-25 (scope expansion)

- Generalized plan from graph-only panes to pane-hosted multi-view architecture.
- Preserved graph-specific `GraphViewState` and Canonical/Divergent semantics as a graph-pane payload.
- Added explicit viewer-pane backend constraints (Servo texture vs Wry overlay vs native embedded viewers).
- Added tool-pane model and pane-generic input-routing invariant.
- Added layout-research and performance alignment constraints (mental map, per-pane LOD, split physics budgets).
