# Wry Integration Strategy: Native Webviews & The Verso Mod

**Date**: 2026-02-23
**Status**: Implementation-Ready (updated 2026-02-24)
**Relates to**:

- `2026-02-22_registry_layer_plan.md` — `ViewerRegistry` (Phase 2, complete) is the contract surface for both backends; `WorkbenchSurfaceRegistry` (Phase 3, complete) owns tile layout policy that drives overlay positioning
- `2026-02-22_multi_graph_pane_plan.md` — pane-hosted multi-view model; Wry applies to Node Viewer panes, not graph panes
- `2026-02-19_ios_port_plan.md` — `wry` is already in scope for the iOS port; coordinate feature-flag usage
- `2026-02-20_cross_platform_sync_and_extension_plan.md` — cross-platform deployment context

---

## Context

Servo (texture-based rendering) is the primary web backend and the only one currently integrated.
`wry` (native OS webview — WKWebView on macOS/iOS, WebView2 on Windows, WebKitGTK on Linux) provides
a compatibility fallback for sites that Servo cannot render correctly. The two backends have
fundamentally different rendering models that constrain where each can be used.

This plan defines how to add `wry` as a second backend under the existing Verso native mod without
splitting user-facing configuration or duplicating shared infrastructure.

In the pane-hosted multi-view model, this is specifically the **Node Viewer pane** backend path.
It should not introduce a separate pane category or bypass pane/compositor routing.

---

## The Critical Distinction: Texture vs Overlay

This is the most important architectural fact of this plan. Everything else follows from it.

**Servo — texture mode**: renders to an OpenGL/WGPU surface or shared memory buffer. The result is
a texture Graphshell owns and can draw anywhere in the scene — inside the graph canvas, rotated on
a moving node, faded, occluded by UI panels. This is why Servo works in both the graph view and
workbench tiles.

**Wry — overlay mode**: creates a native OS window handle (HWND on Windows, NSWindow/WKWebView on
macOS, GtkWindow on Linux) that the OS composites on top of the application surface. Graphshell does
not own the pixels. Consequences:

- Cannot be occluded by Graphshell UI elements — it floats above everything.
- Cannot be rotated, skewed, or scaled by Graphshell's renderer.
- Cannot be placed on a moving graph node — repositioning a native window every frame is jittery
  and breaks OS z-ordering.
- Can only be used in stable, rectangular, axis-aligned regions: workbench tiles or detached windows.

**Hybrid rule for Wry nodes in graph view**: if a node's backend is `viewer:wry` and it is currently
displayed in the graph canvas (not in a workbench tile), render the node's last thumbnail/screenshot
instead of a live webview. The user must open the node in a workbench pane to interact with it.
This is consistent with the existing thumbnail pipeline — no new mechanism needed.

---

## Mod Structure

Verso remains the single "Browser Capability" native mod. Both backends are registered by Verso;
users do not manage them separately.

Verso mod `ModManifest` additions (appended to existing Phase 2 manifest):

- `provides`: add `viewer:wry` alongside existing `viewer:servo` and `viewer:webview`
- `requires`: add `wry` feature gate (see Cargo.toml step below)
- `capabilities`: no change — `network` already declared

`ViewerRegistry` entries after Verso loads with `wry` feature:

| ID | Backend | Mode | Usable in |
| -- | ------- | ---- | --------- |
| `viewer:servo` | Servo | Texture | Graph canvas + workbench tiles |
| `viewer:wry` | wry | Overlay | Workbench tiles only |
| `viewer:webview` | alias | → `viewer:servo` (default) | Configurable via settings |

The `viewer:webview` alias is user-configurable: switching it to `viewer:wry` makes all new webviews
use wry by default. Per-node and per-workspace overrides use the specific IDs.

---

## Viewer Trait Contract Extension

The existing `Viewer` trait in `ViewerRegistry` needs a second rendering path for overlay backends.
Extend it with one additional method:

```rust
pub trait Viewer {
    /// Render content into an egui Ui region (texture mode).
    /// Returns true if the viewer handled rendering, false if it requires overlay mode.
    fn render_embedded(&mut self, ui: &mut egui::Ui, node: &Node) -> bool;

    /// Synchronize overlay position and visibility (overlay mode).
    /// Called by TileCompositor after layout is computed for overlay-backed tiles.
    /// `rect` is in physical screen coordinates. `visible` is false when the tile is
    /// occluded, minimized, or in a tab that is not the active tab.
    fn sync_overlay(&mut self, rect: egui::Rect, visible: bool);

    /// Returns true if this viewer requires overlay mode (cannot render embedded).
    fn is_overlay_mode(&self) -> bool { false }
}
```

`ServoViewer` implements `render_embedded` (returns true), `sync_overlay` is a no-op, and
`is_overlay_mode` returns false.

`WryViewer` implements `render_embedded` returning false (or rendering the thumbnail fallback),
`sync_overlay` calling `wry::WebView::set_bounds()` and `set_visible()`, and `is_overlay_mode`
returning true.

---

## Call Sites and Data Flow

### TileCompositor → ViewerRegistry → WryViewer

After `desktop/tile_compositor.rs` computes layout for each frame, it must notify overlay-backed
viewers of their new screen rect. This is a direct call — not a `GraphIntent`, because it is a
layout effect with no semantic meaning, analogous to how egui passes rects to child widgets.

Pane-hosted interpretation: this applies to overlay-backed **node viewer panes** (or transitional
tile equivalents during migration), not graph-pane render paths.

```rust
// tile_compositor.rs::compose_frame()
  for each tile:
    if viewer_registry.is_overlay_mode(tile.viewer_id):
      let screen_rect = tile.computed_screen_rect();
      let visible = tile.is_active_tab() && !tile.is_occluded();
      viewer_registry.sync_overlay(tile.viewer_id, screen_rect, visible);
```

`TileCompositor` must track which tile IDs are overlay-backed. Add `overlay_tiles: HashSet<TileId>`
to its state, updated when `WryViewer` is attached or detached from a tile.

### Graph View Thumbnail Fallback

When `render_graph_in_ui_collect_actions()` renders a node whose viewer ID is overlay-backed,
it calls `render_embedded` on the viewer. `WryViewer::render_embedded` returns false (or renders
the node's `thumbnail_data` if present). The render layer already handles the `false` case for
nodes without a live webview — Wry nodes in graph view use the same path.

No new mechanism is needed. The existing thumbnail pipeline in `Node.thumbnail_data` is the fallback.

---

## Implementation Plan

### Step 1: Feature Gate and Cargo.toml

- Add `wry` to `Cargo.toml` under a feature flag: `features = ["wry"]`.
- Gate all `WryViewer` and `WryManager` code under `#[cfg(feature = "wry")]`.
- Default: feature off. Enable explicitly for builds that require wry.
- The Verso mod's `ModManifest` `requires` field should include a `feature:wry` capability check;
  if the feature is not compiled in, `viewer:wry` is simply not registered.

Done gate: `cargo build` (without `--features wry`) clean. `cargo build --features wry` compiles.

### Step 2: WryManager Scaffold

Add `WryManager` to `mods/native/verso/` (alongside existing Servo glue) under `#[cfg(feature = "wry")]`:

- Holds a `HashMap<NodeKey, wry::WebView>` of active wry webviews.
- Provides `create_webview(node_key, url) -> Result<()>` and `destroy_webview(node_key)`.
- Provides `set_bounds(node_key, rect: egui::Rect, visible: bool)` translating egui rect to physical
  pixel coordinates using the window scale factor.

Done gate: `WryManager` constructs without error on Windows. Basic create/destroy roundtrip works
in a headless test.

### Step 3: WryViewer Implementation

Implement `WryViewer` in `registries/atomic/viewer/wry_viewer.rs` under `#[cfg(feature = "wry")]`:

- `render_embedded`: calls `WryManager::get_thumbnail(node_key)` and renders it, or renders a
  "Wry — open in pane to interact" placeholder if no thumbnail is available. Returns false.
- `sync_overlay`: calls `WryManager::set_bounds(node_key, rect, visible)`.
- `is_overlay_mode`: returns true.

Register `viewer:wry` in Verso mod's `register_viewers()` function.

Done gate: `viewer:wry` appears in `ViewerRegistry` when Verso mod loads with `wry` feature.
`is_overlay_mode()` returns true. `render_embedded` renders placeholder without panic.

### Step 4: TileCompositor Overlay Tracking

Update `desktop/tile_compositor.rs`:

- Add `overlay_tiles: HashSet<TileId>` field.
- In `attach_viewer_to_tile()`: if `viewer_registry.is_overlay_mode(viewer_id)`, insert into
  `overlay_tiles`.
- In `detach_viewer_from_tile()`: remove from `overlay_tiles`; call `sync_overlay(rect, false)` on
  the outgoing viewer to hide the OS window.
- In `compose_frame()`: after computing rects, iterate `overlay_tiles` and call `sync_overlay` on
  each.

Done gate: a wry-backed tile receives `sync_overlay` calls each frame. Moving/resizing the
workbench tile moves the underlying OS webview in sync (manual headed test).

### Step 5: Lifecycle Integration

Wry webviews must respect the same Active/Warm/Cold lifecycle as Servo webviews:

- Active: webview created and visible (`sync_overlay(..., visible: true)`).
- Warm: webview created but hidden (`sync_overlay(..., visible: false)`), or not yet created (cold
  promotion path).
- Cold: webview destroyed; node holds thumbnail only.

In `desktop/lifecycle_reconcile.rs`, add `viewer:wry` handling alongside the existing `viewer:servo`
path. The reconciler checks the node's `viewer_id` preference and calls the appropriate
`WryManager` method.

Done gate: promoting a cold node with `viewer_id = viewer:wry` creates a wry webview in the
workbench tile. Demoting destroys it and the `overlay_tiles` set is updated correctly.

### Step 6: Per-Node and Per-Workspace Backend Selection

Users can set a backend preference per node or per workspace:

- Node-level: `GraphIntent::SetNodeViewerPreference { node: NodeKey, viewer_id: ViewerId }`.
  Stored on `Node.viewer_id_override: Option<ViewerId>`. Persisted to the graph WAL (fjall) as a
  node metadata update.
- Workspace-level: stored in `WorkspaceManifest` as `viewer_id_default: Option<ViewerId>`.
  Falls back to the `viewer:webview` alias if absent.
- Resolution order: node override → workspace default → `viewer:webview` alias.

Done gate: setting `viewer_id_override` on a node to `viewer:wry` causes the next lifecycle
reconcile to use `WryManager` for that node. Contract test covers resolution order.

### Step 7: Settings UI

Expose backend selection in the settings UI:

- Global default: "Default web backend" dropdown in Settings → Web → Rendering, showing `viewer:servo`
  and `viewer:wry` (changes the `viewer:webview` alias target).
- Per-node: context menu → "Open with" → "Servo" / "wry". Dispatches `SetNodeViewerPreference`.
- Per-workspace: workspace settings page, "Default backend for this workspace".

Done gate: changing the global default persists across restarts. Per-node override appears in node
context menu and takes effect on next lifecycle reconcile.

---

## Platform Targeting

Implement and test in this order:

1. **Windows** (WebView2) — primary development platform; WebView2 is pre-installed on Windows 10+.
2. **macOS** (WKWebView) — second; requires entitlement for outbound network if sandboxed.
3. **Linux** (WebKitGTK) — third; requires `libwebkit2gtk-4.1-dev` system dependency; note in
   `BUILD.md` when implemented.

The `wry` crate handles platform abstraction. No platform-specific code in Graphshell except for
scale-factor and coordinate translation in `WryManager::set_bounds`.

---

## Risks and Mitigations

Overlay z-ordering conflicts: wry OS windows always paint above Graphshell UI. Mitigation: ensure
dialogs, panels, and radial menus are rendered as egui windows (which are also overlays but managed
by egui). If a wry webview must be hidden when a dialog opens, call `sync_overlay(..., false)`.

Jitter when tiling workbench: if tile layout changes rapidly (drag-to-resize), `sync_overlay` is
called each frame, which may cause visual lag on the OS webview. Mitigation: throttle `set_bounds`
calls to at most once per 16ms (one frame); skip if rect is unchanged.

Scale factor changes: DPI change events from winit must propagate to `WryManager::set_bounds` so
the webview tracks the new physical pixel rect. Add a `handle_scale_factor_changed` method to
`WryManager` and call it from the winit event handler.

Wry nodes in graph canvas showing stale thumbnails: the thumbnail pipeline currently updates on
page load and title change. Ensure `WryViewer` requests a screenshot snapshot on navigation
completion and stores it in `Node.thumbnail_data`. This uses the same `notify_url_changed` pipeline
as Servo — add a `request_thumbnail()` call to `WryManager` triggered by the URL-changed event.

Feature-flag build drift: `#[cfg(feature = "wry")]` gates must be maintained consistently. Add a
CI check that compiles with and without the feature.

---

## Findings

The "two backends in one mod" structure avoids the user-mental-model problem of managing separate
mods for the same browsing capability. The `ViewerRegistry` contract (`render_embedded` /
`sync_overlay` / `is_overlay_mode`) gives `TileCompositor` a clean interface that requires no
knowledge of which backend is active. The lifecycle reconciler's existing Active/Warm/Cold model
extends naturally to wry webviews without structural changes.

The only novel infrastructure required is `overlay_tiles` tracking in `TileCompositor` and
`WryManager` as a coordinator. Everything else — lifecycle, thumbnail fallback, settings persistence,
node identity — reuses existing mechanisms.

---

## Progress

### 2026-02-23

- Plan created as research/draft: core question (mod structure), texture vs overlay distinction,
  hybrid compromise, and basic Viewer trait extension identified.

### 2026-02-24 (implementation-ready revision)

- Promoted from draft to implementation-ready.
- `Viewer` trait extension made concrete with `is_overlay_mode()` method and full signatures.
- `TileCompositor` call site made explicit: direct call after layout, not a `GraphIntent`.
- Implementation plan structured as 7 sequential steps with done gates.
- Platform targeting order defined: Windows first, macOS second, Linux third.
- `WryManager` data model (`HashMap<NodeKey, wry::WebView>`) and `overlay_tiles: HashSet<TileId>`
  tracking in `TileCompositor` made concrete.
- Lifecycle integration with `lifecycle_reconcile.rs` and Active/Warm/Cold model described.
- Per-node (`Node.viewer_id_override`) and per-workspace (`WorkspaceManifest.viewer_id_default`)
  backend selection defined with resolution order and `GraphIntent` variant.
- Risks: z-ordering, resize jitter, scale factor changes, stale thumbnails, feature-flag drift.
- Thumbnail fallback for graph view aligned to existing `Node.thumbnail_data` pipeline.
- `wry` already noted in iOS port plan; feature-flag approach must be coordinated there.
