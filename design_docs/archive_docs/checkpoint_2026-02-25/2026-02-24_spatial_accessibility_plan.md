# Spatial Accessibility Implementation Plan (2026-02-24)

> **SUPERSEDED** — This document has been consolidated into
> `SUBSYSTEM_ACCESSIBILITY.md` (Cross-Cutting Subsystem: Accessibility).
> Retained for historical reference only. Do not use as authoritative.

**Status**: ~~Implementation-Ready~~ Superseded (2026-02-25)
**Research**: `../research/2026-02-24_spatial_accessibility_research.md`
**Required companion**: ~~`2026-02-25_accessibility_contracts_diagnostics_and_validation_strategy.md`~~ → see `SUBSYSTEM_ACCESSIBILITY.md`
**Goal**: Make Graphshell fully navigable and understandable for non-visual users.

## 1. Architecture

### 1.1 The Accessibility Bridge (`GraphAccessKitAdapter`)
A translation layer that converts the `petgraph` structure into an `accesskit::TreeUpdate`.
- **Input**: `Graph`, `SelectionState`, `MetadataFrame` (layout).
- **Output**: A virtual tree of `accesskit::Node`s (Clusters -> Nodes -> Edges).
- **Integration**: Injected into `egui`'s accessibility context via `ctx.accesskit_placeholder()`.

### 1.2 The WebView Bridge
A mechanism to graft Servo's internal accessibility tree into the host window's tree.
- **Mechanism**: `egui` provides a hook to append native child windows or external trees. We map the `WebView` widget's ID to Servo's root ID.

### 1.3 The Announcer
A service for "Live Regions" (polite/assertive notifications).
- **API**: `Announcer::speak(text, priority)`.
- **Backend**: `accesskit` live region updates.

### 1.4 The Audio Engine (`Sonifier`)
- **Crates**: `rodio` (output), `fundsp` (synthesis).
- **Role**: Generates spatial cues (panning, pitch) based on graph state.

---

## 2. Implementation Phases

### Phase 1: The WebView Bridge (Critical Fix)
**Goal**: Screen readers can read web content inside Graphshell.
1.  **Update `EmbedderWindow`**: Ensure `notify_accessibility_tree_update` forwards events to `Gui`.
2.  **Update `Gui` bridge state**: Track `WebViewId -> egui::Id` accessibility anchors for active webview tiles and queue pending updates per webview.
3.  **Compatibility layer**: Convert Servo `accesskit` tree updates to the `egui`-compatible `accesskit` type version (or align dependencies).
4.  **Bridge injection**: Inject converted nodes via `egui`'s AccessKit node-builder hook(s) under the registered anchor.
5.  **Diagnostics + degradation**: Emit accessibility bridge diagnostics for received/injected/dropped/conversion-failed updates and surface degraded status.

### Phase 2: Graph Linearization (The Graph Reader / Virtual Tree)
**Goal**: The graph canvas is no longer a "black box" but a navigable list of nodes accessible to screen readers and keyboard-only users.

#### 2.1 Graph Reader Modes

Two explicit modes are defined, corresponding to the "Room" and "Map" metaphors in the research doc:

**Mode A — Room Mode (Local Context, default when a node is focused)**
- Activated when the user focuses any node in the graph (e.g., via click, Tab, or programmatic focus).
- The focused node is treated as a "Room". The virtual tree rooted at that node exposes:
  - **Node summary**: title, URL/address, UDC tags, content type (MIME hint).
  - **Edges (Doors)**: connected nodes grouped by relationship type (Outgoing / Incoming / Bidirectional), each rendered as a child `accesskit::Node` with role `Link` and label `"<direction>: <neighbor title>"`.
  - **Cluster context (Walls)**: a read-only `accesskit::Node` with role `Note` announcing the node's cluster membership and local degree (e.g., `"In cluster 'Rust'. 5 connections."`).
- **Scope**: shallow — only the immediate neighborhood (depth 1) is materialized in the tree.

**Mode B — Map Mode (Global Linearization, for full graph traversal)**
- Activated explicitly by the user (see Entry Points below).
- The entire graph is flattened using the **Semantic Hierarchy** algorithm:
  - **Level 1 (Cluster nodes)**: One `accesskit::Node` per detected community/UDC cluster, role `Group`, label = cluster name or `"Uncategorized"`.
  - **Level 2 (Hub nodes)**: High-degree nodes within each cluster, role `TreeItem`, sorted descending by degree.
  - **Level 3 (Leaf nodes)**: Remaining nodes within each cluster, role `TreeItem`, sorted by title.
- Edges are not materialized at map level to keep the tree manageable; activating (pressing Enter on) a hub or leaf node switches into **Room Mode** for that node.
- **Fallback ordering**: If community detection is unavailable, fall back to **Spatial Sweep** (sort by Y then X, left-to-right reading order).

#### 2.2 Navigation Entry Points

| Trigger | Action |
|---|---|
| `Tab` / `Shift+Tab` (while graph canvas is focused) | Move to next / previous node in the active linearization (Room Mode: traverse edges; Map Mode: traverse hierarchy). |
| `Ctrl+L` | Toggle the active graph pane between **Canvas Mode** (visual) and **List Mode** (renders graph as `egui::Tree`, enters Map Mode linearization). |
| `Enter` on a focused node (in Map Mode) | Drill into **Room Mode** for that node. |
| `Escape` (while in Room Mode) | Return to Map Mode, restoring focus to the previously selected node in the hierarchy. |
| `Ctrl+Arrow` | Jump between cluster groups (Level 1 nodes) in Map Mode. |
| `F6` | Cycle focus across top-level regions: Toolbar → Graph Canvas → Active Pane. Entering the Graph Canvas activates Room Mode on the last focused node (or first node if none). |
| `Alt+Shift+R` | Explicitly enter **Map Mode** (Graph Reader) from anywhere in the application. |

#### 2.3 AccessKit Virtual Tree Output Shape

The `GraphAccessKitAdapter` produces an `accesskit::TreeUpdate` with the following structure:

```
Root (role: Window — owned by egui)
└── GraphCanvas (role: ScrollView, label: "Graph Canvas")
    ├── [Map Mode root, hidden when in Room Mode]
    │   GraphReaderRoot (role: Tree, label: "Graph Reader — <N> nodes")
    │   ├── ClusterGroup_<id> (role: Group, label: "<cluster name> — <k> nodes")
    │   │   ├── HubNode_<uuid> (role: TreeItem, label: "<title>", description: "<url> · degree <d>")
    │   │   │   └── ... (children omitted at map level; activated via Enter)
    │   │   └── LeafNode_<uuid> (role: TreeItem, label: "<title>", description: "<url>")
    │   └── ...
    └── [Room Mode root, active when a node is focused]
        FocusedNode_<uuid> (role: Article, label: "<title>", description: "<url> · <tags>")
        ├── ClusterContext (role: Note, label: "In cluster '<name>'. <k> connections.")
        ├── EdgeGroup_outgoing (role: Group, label: "Outgoing links — <n>")
        │   └── Edge_<uuid> (role: Link, label: "Outgoing: <neighbor title>", description: "<neighbor url>")
        ├── EdgeGroup_incoming (role: Group, label: "Incoming links — <n>")
        │   └── Edge_<uuid> (role: Link, label: "Incoming: <neighbor title>", description: "<neighbor url>")
        └── EdgeGroup_bidirectional (role: Group, label: "Bidirectional links — <n>")
            └── Edge_<uuid> (role: Link, label: "Bidirectional: <neighbor title>", description: "<neighbor url>")
```

**Node ID stability**: `accesskit::NodeId`s are derived deterministically from `Node.id` (UUID) via a stable hash, ensuring focus is preserved across tree refreshes.

**Update policy**: The adapter rebuilds only the subtree that changed (focused node's Room, or the full hierarchy when entering Map Mode). Updates are throttled to 10 Hz to avoid blocking the render thread.

#### 2.4 Implementation Steps

1.  **Implement `GraphAccessKitAdapter`**:
    -   Define `SemanticHierarchy` algorithm (Cluster → Hub → Leaf) with Spatial Sweep fallback.
    -   Implement `RoomView` builder (focused node + depth-1 neighborhood).
    -   Generate stable `accesskit::NodeId`s from `NodeKey`s (using `Node.id` UUIDs).
2.  **Implement mode state in `GraphViewState`**:
    -   Add `GraphReaderMode` enum: `Room { focused: NodeKey }` | `Map` | `Off`.
    -   Wire `Ctrl+L`, `Alt+Shift+R`, `Enter`, and `Escape` handlers in `input/mod.rs`.
3.  **Wire to `GraphView`**:
    -   In `render/mod.rs`, populate the adapter during the render pass.
    -   Submit the tree update to `egui` via `ctx.accesskit_placeholder()`.
4.  **Wire `F6` skip-link** in `input/mod.rs` to cycle: Toolbar → Graph → Active Pane.

### Phase 3: Navigation & Focus
**Goal**: Keyboard users can move efficiently between UI regions.
1.  **Skip Links**: Implement `F6` handler in `input/mod.rs` to cycle focus: Toolbar -> Graph -> Active Pane.
2.  **Programmatic Focus**: When `GraphAction::FocusNode` occurs, explicitly move `accesskit` focus to the node's virtual element.
3.  **Spatial D-Pad**: Map Arrow Keys in `GraphView` to find nearest node in direction (geometric search).

### Phase 4: Sonification (Audio Display)
**Goal**: Audio cues provide spatial context.
1.  **Add Dependencies**: `rodio`, `fundsp`.
2.  **Implement `Sonifier`**:
    -   `play_tone(frequency, pan, volume)`
    -   `update_density_hum(velocity)`
3.  **Wire to Physics**: In `app.rs`, update hum based on `physics.last_avg_displacement`.

### Phase 5: Diagnostics & Validation
**Goal**: Verify accessibility without needing a screen reader constantly.
1.  **Inspector**: Add "Accessibility" tab to Diagnostic Inspector.
    -   Render the current `accesskit` tree structure as a text tree.
    -   Log `Announcer` events.
2.  **Automation**: Add `test_linearization_order` to `desktop/tests/scenarios/accessibility.rs`.

---

## 3. Strategy: Ongoing Maintenance

This section is scoped to feature maintenance only. Project-level guarantees, diagnostics integration, contracts, and CI validation requirements are defined in the required companion:
- `2026-02-25_accessibility_contracts_diagnostics_and_validation_strategy.md`

### 3.1 Diagnostics Integration
Accessibility state is hidden state. We must make it visible.
- **Channel**: `registry.accessibility.tree_update` (logs size/latency of updates).
- **Visualizer**: The Diagnostic Inspector should show the "Virtual Cursor" position overlaid on the graph.

### 3.2 Testing Policy
- **Unit Tests**: Verify `SemanticHierarchy` produces deterministic ordering.
- **Integration**: Verify `F6` cycles focus through expected regions.

### 3.3 Performance Guard
- **Throttling**: Accessibility tree updates can be heavy. Throttle graph updates to 5Hz or 10Hz (decoupled from visual 60Hz).
- **Culling**: Only linearize nodes within the viewport + buffer? (Debatable: SR users might want to explore off-screen). *Decision*: Linearize full graph for now (N < 1000), optimize later.
