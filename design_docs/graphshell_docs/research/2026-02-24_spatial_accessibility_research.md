# Accessibility (A11y) Research & Strategy (2026-02-24)

**Status**: Research
**Goal**: Define the architecture for making the entire application (Graph, UI, Web Content) navigable, understandable, and efficient for non-visual users.

## 0. Scope: Holistic Accessibility

Accessibility is not limited to the spatial graph. The application has three distinct accessibility domains that must be unified:
1.  **Standard UI (Toolbar, Panels, Settings)**: Handled by `egui`'s built-in `accesskit` integration. Requires validation of tab order and label semantics.
2.  **Web Content (Servo)**: Handled by Servo's internal accessibility tree. Requires a **Bridge** to merge Servo's tree into the host window's accessibility tree.
3.  **Spatial Graph (Canvas)**: The novel challenge. Requires custom linearization and sonification (detailed below).

---

## 1. The Core Conflict: Spatial vs. Linear

Graphshell's primary interface is spatial (force-directed graph). Screen readers (SR) are linear (stream of text/controls).
*   **Visual User**: Perceives clusters, outliers, and density at a glance.
*   **Non-Visual User**: Perceives one item at a time.

**The Goal**: Provide "Glanceability" via audio/haptics and "Navigability" via structured linearization.

## 2. Linearization Strategies (The "Graph Reader")

We need a deterministic way to flatten the graph into a list or tree structure that `accesskit` can consume.

### 2.1 The "Room" Metaphor (Local Context)
When a node is focused, treat it as a "Room".
*   **Content**: The node's title, URL, tags.
*   **Doors (Edges)**: List of connected nodes, grouped by relationship type (Parent, Child, Associated).
*   **Walls (Context)**: "You are in the 'Rust' cluster. 5 other nodes nearby."

### 2.2 Global Linearization (The "Map")
How to traverse the whole graph without getting lost?
*   **Spatial Sweep**: Sort nodes by Y, then X (reading order). Good for finding "what's top-left".
*   **Semantic Hierarchy**: Use UDC tags or Community Detection (Louvain/Leiden) to build a tree.
    *   Level 1: Clusters ("Rust", "News", "Uncategorized").
    *   Level 2: Hub nodes (high degree).
    *   Level 3: Leaf nodes.
*   **Minimum Spanning Tree (MST)**: A spanning tree allows traversing all nodes without cycles.

**Recommendation**: Implement **Semantic Hierarchy** as the primary navigation tree for SRs. It aligns with the mental model of "folders".

## 3. Navigation Models

### 3.1 Spatial D-Pad (Geometric Navigation)
Map Arrow Keys to physical direction.
*   **Up**: Find nearest node in the -Y cone (45 degrees).
*   **Right**: Find nearest node in the +X cone.
*   **Benefit**: Allows exploring the physical layout created by physics.

### 3.2 Structural Navigation (Logical Navigation)
*   **Tab**: Next node in the Linearization (see §2.2).
*   **Shift+Tab**: Previous node.
*   **Ctrl+Arrow**: Jump between Clusters.

## 4. Sonification (Audio Display)

Using audio to convey spatial properties that text cannot.

### 4.1 Spatial Panning (Where am I?)
*   **Stereo Pan**: Map Node X coordinate (relative to viewport center) to Left/Right audio balance.
*   **Volume/Reverb**: Map Node Y coordinate (or distance from center) to Volume or Reverb (farther = quieter/wetter).

### 4.2 Data Sonification (What is this?)
*   **Pitch**: Map Node Degree (importance) to Pitch. High degree = Lower, resonant pitch (Bass). Leaf = High, light pitch (Tink).
*   **Timbre**: Map Content Type (MIME) to instrument.
    *   Web: Piano.
    *   PDF: Strings.
    *   Image: Percussion.

### 4.3 Density Hum (The "Geiger Counter")
As the cursor moves (or during physics simulation), play a background texture representing graph density/energy.
*   **High Energy (Moving)**: Active, chaotic texture.
*   **Low Energy (Settled)**: Calm, harmonic drone.

## 5. Implementation Strategy

### 5.1 The Accessibility Bridge
We cannot rely solely on `egui`'s default widget accessibility for a custom painted graph.
*   **Action**: Implement a `GraphAccessKitAdapter`.
*   **Function**: Syncs `Graph` state to `egui::Context::accesskit_root`.
*   **Optimization**: Only update the "Virtual Tree" when the graph settles or selection changes.

### 5.2 Async Architecture (The "A11y Worker")
Linearizing a 10k node graph or synthesizing audio shouldn't block the UI.
*   **AccessibilityWorker**: A background task (supervised by `ControlPanel`) that computes the Linearization and drives the Audio Engine.
*   **Updates**: Listens to `GraphIntent` (like `AddNode`, `SelectionChanged`) and pushes updates to the UI/Audio.

### 5.3 Crate Selection
*   **`accesskit`**: Already in use by `egui`. We need to feed it custom tree updates for the canvas.
*   **`rodio` / `symphonia`**: Already selected for `AudioViewer`. Reuse for sonification.
*   **`fundsp`**: For procedural audio synthesis (generating the "Density Hum" or UI sounds dynamically).

### 5.4 The "List View" Fallback
A dedicated `TileKind::List` or a mode in `TileKind::Graph` that renders the graph as a standard `egui::Table` or `Tree`.
*   **Why**: Sometimes a list is just better.
*   **Integration**: `Ctrl+L` toggles the active Graph Pane between "Canvas Mode" and "List Mode".

### 5.5 The WebView Bridge (Servo Integration)
Currently, `gui.rs` drops accessibility updates from Servo (`notify_accessibility_tree_update`).
*   **Gap**: Web content is invisible to screen readers.
*   **Fix**: Implement a bridge that accepts `accesskit::TreeUpdate` from Servo and grafts it into the `egui` accessibility tree at the `WebView` widget's node ID.
*   **Mechanism**: `egui` exposes hooks to append external trees. We must map Servo's root ID to the egui widget ID.

## 6. Validation
*   **Blind Test**: Navigate from Node A to Node B using only keyboard and audio.
*   **Screen Reader**: Verify NVDA/VoiceOver reads "Node: Rust Homepage, 3 connections" instead of "Graphic".
*   **Web Content**: Verify screen reader can enter a webview and read page content (via the Bridge).

## 7. Gaps & Refinements

### 7.1 Live Regions & Announcements
*   **Gap**: Dynamic events (physics settling, sync completion, node arrival) are currently visual-only.
*   **Refinement**: Implement an `Announcer` service that pushes text updates to `accesskit`'s live region API.
*   **Policy**: "Polite" announcements for background events (sync), "Assertive" for errors or direct interactions.

### 7.2 Focus Management & Trap Avoidance
*   **Gap**: Users may get "trapped" in the graph canvas or webview if keyboard navigation doesn't provide an escape hatch.
*   **Refinement**:
    *   **Skip Links**: "Skip to Toolbar", "Skip to Graph", "Skip to Content" shortcuts (e.g., `F6`).
    *   **Programmatic Focus**: When switching views (Graph <-> Detail), explicitly move `accesskit` focus to the primary element of the new view.

### 7.3 Preference Integration (Reduced Motion)
*   **Gap**: Force-directed motion triggers vestibular disorders.
*   **Refinement**: Query OS `prefers-reduced-motion`.
    *   If true: Physics defaults to `Paused` (static layout) or `Instant` (compute layout in background, then render).
    *   Animations (zoom, orbit) become instantaneous.

### 7.4 Audio Level-of-Detail (Semantic Zoom)
*   **Gap**: Sonifying 500 nodes individually creates audio chaos.
*   **Refinement**: Audio LOD must match Visual LOD.
    *   **Zoomed In**: Hear individual nodes (pitch by degree).
    *   **Zoomed Out**: Hear cluster "drones" or summary sounds (pitch by cluster size).