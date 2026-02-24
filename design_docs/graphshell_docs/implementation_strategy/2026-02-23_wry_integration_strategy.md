# Wry Integration Strategy: Native Webviews & The Verso Mod

**Date**: 2026-02-23
**Status**: Research / Draft
**Context**: Investigating the integration of `wry` (cross-platform native webview) alongside Servo to provide a fallback/alternative rendering engine.

---

## 1. The Core Question: Mod Structure

**Question**: Should Servo and Wry be separate mods, or backends of the Verso mod?

**Recommendation**: **Verso should be the monolithic "Web Capability" Native Mod that registers both backends.**

### Rationale
1.  **User Mental Model**: Users install/enable "Verso" to get web browsing. They shouldn't need to manage dependencies like "Verso Core" + "Servo Renderer".
2.  **Shared Infrastructure**: Both engines share the same `ProtocolRegistry` (http/https) and `ActionRegistry` (navigation commands). Splitting them would duplicate this glue code.
3.  **Runtime Switching**: We want to allow users to switch engines per-tab or per-workspace (e.g., "Use Wry for this heavy React site"). This requires both engines to be available and managed by a single coordinator.

### Registry Impact
The Verso Mod will register two distinct viewers in the `ViewerRegistry`:
*   `viewer:servo` (The default, texture-based)
*   `viewer:wry` (The alternative, overlay-based)

It will also register a **default alias**:
*   `viewer:webview` -> points to `viewer:servo` (configurable via Settings).

---

## 2. The Technical Constraint: Texture vs. Overlay

This is the most critical architectural distinction.

### Servo (Texture Mode)
*   **Mechanism**: Renders to an OpenGL/WGPU surface or shared memory buffer.
*   **Capabilities**: Can be drawn *inside* the Graph View. It can rotate, fade, scale, and be occluded by other nodes.
*   **Usage**: Primary renderer for the **Graph Canvas** and **Workbench Tiles**.

### Wry (Overlay Mode)
*   **Mechanism**: Creates a native OS window (HWND/NSWindow) that sits *on top* of the application window.
*   **Limitations**:
    *   Cannot be rotated or skewed.
    *   Cannot be partially occluded by Graphshell UI elements (it floats on top).
    *   Cannot be rendered into the 3D/Physics Graph Canvas (nodes moving around would require moving heavy OS windows constantly, which is jittery and breaks z-ordering).
*   **Usage**: Restricted to **Workbench Tiles** (rectangular, static regions) or "Detached" windows.

### The "Hybrid" Compromise
If a user opens a Wry node in the Graph View:
1.  **Active State**: We cannot render the live webview on the moving node.
2.  **Fallback**: We render a **static screenshot/thumbnail** on the node in the graph.
3.  **Interaction**: To interact, the user must open it in a **Workbench Pane** (Split/Tab), where the Wry overlay can be safely positioned.

---

## 3. Implementation Strategy

### 3.1 Dependencies
*   **Crate**: `wry` (already used in the iOS plan).
*   **Feature Gate**: `features = ["wry"]` in `Cargo.toml`.

### 3.2 The `ViewerRegistry` Contract
We need to extend the `Viewer` trait to support overlays.

```rust
pub trait Viewer {
    /// Render to an egui Ui (Texture mode).
    /// Returns true if handled, false if this viewer requires overlay mode.
    fn render_embedded(&mut self, ui: &mut Ui, node: &Node) -> bool;

    /// Sync overlay position (Overlay mode).
    /// Called when the viewer is in a stable rectangular region (Workbench).
    fn sync_overlay(&mut self, rect: Rect, visible: bool);
}

3.3 Integration Steps
Extend Verso Mod: Add WryManager alongside the existing Servo glue.
Implement viewer:wry:
render_embedded: Returns false (or renders a placeholder/thumbnail).
sync_overlay: Calls wry::WebView::set_bounds().
Update Workbench: TileCompositor:
Needs to track which tiles are "Overlay-backed".
Must emit a "rect update" signal to the ViewerRegistry after layout is computed.
4. Summary
Verso remains the single "Browser Mod".
Servo is the "Graph-Native" engine (default).
Wry is the "Compatibility" engine (Workbench only).
Registry handles the selection via viewer:servo vs viewer:wry.