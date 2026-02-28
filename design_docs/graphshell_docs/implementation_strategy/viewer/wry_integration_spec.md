# Wry Integration — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active (Windows implementation target)

**Related**:

- `VIEWER.md`
- `viewer_presentation_and_fallback_spec.md`
- `universal_content_model_spec.md`
- `viewer/2026-02-23_wry_integration_strategy.md`
- `../aspect_render/frame_assembly_and_compositor_spec.md`
- `../../TERMINOLOGY.md` — `TileRenderMode`, `NativeOverlay`, `CompositorAdapter`, `Viewer`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Overlay vs texture distinction** — when Wry uses a native overlay vs a composited texture.
2. **WryViewer and WryManager** — the viewer backend and its manager lifecycle.
3. **NativeOverlay compositor dispatch** — how the compositor handles the NativeOverlay tile mode.
4. **Lifecycle integration** — Active/Warm/Cold states for Wry webviews.
5. **Backend selection** — when Wry is preferred over ServoViewer.
6. **Platform targeting** — platform-specific constraints and ordering.

---

## 2. Overlay vs Texture Distinction Contract

Wry renders web content in one of two modes, determined at viewer attachment time:

```
WryRenderMode =
  | NativeOverlay   -- Wry webview is a native child window; owns its own region
  | CompositedTexture  -- Wry renders to an offscreen texture composited into egui
```

**Platform matrix**:

| Platform | Available modes | Default mode |
|----------|----------------|--------------|
| Windows  | `NativeOverlay`, `CompositedTexture` (WebView2 off-screen) | `NativeOverlay` |
| macOS    | `NativeOverlay`, `CompositedTexture` (WKWebView off-screen) | `NativeOverlay` |
| Linux    | `NativeOverlay` only (WebKitGTK) | `NativeOverlay` |

**Invariant**: `WryRenderMode` maps directly to `TileRenderMode`. A `WryViewer` in `NativeOverlay` mode sets `TileRenderMode::NativeOverlay` on its `NodePaneState`; a `WryViewer` in `CompositedTexture` mode sets `TileRenderMode::CompositedTexture`.

**Invariant**: `CompositedTexture` is only available on platforms where the underlying webview engine supports off-screen rendering. On Linux, `WryRenderMode` is always `NativeOverlay`.

### 2.1 Mode Selection

The `WryManager` selects `WryRenderMode` at viewer attachment time based on:

1. Platform capability (Linux → forced `NativeOverlay`).
2. `AppPreferences.wry_render_mode_preference` — user preference (`Auto | ForceOverlay | ForceTexture`).
3. Tile context (if tile is in a floating window, `NativeOverlay` z-ordering may conflict → fallback to `CompositedTexture` if available).

**Invariant**: Mode selection is performed once at attach time. Mode cannot change while a `WryViewer` is active. To switch modes, the viewer must be detached and re-attached.

---

## 3. WryViewer Contract

`WryViewer` is the viewer backend for non-Servo web content. It implements the `Viewer` trait.

```
WryViewer {
    webview: wry::WebView,
    wry_render_mode: WryRenderMode,
    node_key: NodeKey,
    tile_rect: Rect,         -- last known tile rect, for sync_overlay
    manager: &WryManager,    -- shared manager; WryViewer does not own the webview pool
}
```

### 3.1 render_embedded

- Called only when `wry_render_mode == CompositedTexture`.
- Extracts the current frame texture from the WebView2/WKWebView off-screen buffer and composites it into the egui tile rect via `CompositorAdapter`.
- If no frame is available (first frame, loading), renders the `Placeholder` surface until a frame arrives.

**Invariant**: When `wry_render_mode == NativeOverlay`, `render_embedded` is a no-op. The native webview window owns its own rendering; egui does not composite it.

### 3.2 sync_overlay

- Called every frame when `wry_render_mode == NativeOverlay`.
- Updates the native webview window position and size to match the current `tile_rect` from the last layout pass.
- Must complete synchronously within the Overlay Affordance Pass (see `frame_assembly_and_compositor_spec.md §2`).

**Invariant**: `sync_overlay` must not block on the webview engine. Position updates are posted as async messages to the native window; the webview engine applies them asynchronously. This prevents frame deadline overruns.

### 3.3 is_overlay_mode

Returns `true` when `wry_render_mode == NativeOverlay`.

### 3.4 on_navigate

Calls `webview.load_url(address)`. For `NativeOverlay` mode this is a direct webview call. For `CompositedTexture` mode, navigation triggers a new off-screen render cycle.

---

## 4. WryManager Contract

`WryManager` owns the Wry webview pool and coordinates lifecycle across all active `WryViewer` instances.

```
WryManager {
    webviews: HashMap<NodeKey, wry::WebView>,
    platform: WryPlatform,   -- Windows | macOS | Linux
    event_loop_proxy: EventLoopProxy,
}
```

### 4.1 Webview Pool

`WryManager` creates and owns all `wry::WebView` instances. `WryViewer` holds a reference but does not own the `WebView`. This allows the manager to reclaim webviews from Warm/Cold nodes without waiting for the viewer to detach.

**Invariant**: A `wry::WebView` is created by `WryManager::create_webview(node_key)` and destroyed by `WryManager::destroy_webview(node_key)`. No code path other than `WryManager` may create or destroy Wry webviews.

### 4.2 Lifecycle Integration

`WryManager` tracks node lifecycle state and manages webview resource allocation accordingly:

| NodeState | WryManager action |
|-----------|-------------------|
| `Active` | Webview is active and rendering |
| `Warm` | Webview is suspended (JS paused, rendering paused); position/size preserved |
| `Cold` | Webview is destroyed and removed from pool; state is serialized to a snapshot |

**Invariant**: Transitioning a node from `Warm` to `Cold` destroys the Wry webview. Re-activating from `Cold` creates a new webview and restores the serialized snapshot (scroll position, form state where available).

### 4.3 Event Routing

Wry webviews emit navigation events (load started, load complete, title changed, URL changed). `WryManager` translates these into `GraphSemanticEvent` emissions:

```
GraphSemanticEvent::WryNavigation {
    node_key: NodeKey,
    event: WryNavigationEvent,   -- LoadStarted | LoadComplete | UrlChanged | TitleChanged
}
```

**Invariant**: `WryManager` must not call graph mutation APIs directly. All graph-affecting events cross the `GraphSemanticEvent` boundary.

---

## 5. NativeOverlay Compositor Dispatch Contract

When a tile's `TileRenderMode` is `NativeOverlay`, the compositor behaves differently from `CompositedTexture`:

| Pass | NativeOverlay behavior |
|------|------------------------|
| UI Chrome Pass | Renders tab bar, focus ring, and chrome in the tile's chrome/gutter region only |
| Content Pass | No GL callback; the native webview window owns the content region |
| Overlay Affordance Pass | Renders affordances in chrome/gutter only; must not draw inside the native content rect |

**Invariant**: No egui draw call may target the pixel region owned by a `NativeOverlay` window. The compositor must clip all egui draw commands to exclude the native content rect.

### 5.1 Z-ordering

The native webview window is always on top of the egui surface within its tile rect. This is a platform constraint: native child windows cannot be composited beneath egui's OpenGL surface.

**Implication**: Affordances that must appear over web content (e.g., selection rings, diagnostic overlays) cannot be rendered over `NativeOverlay` tiles. For these tiles, affordances are confined to the chrome/gutter region.

### 5.2 Tile Occlusion

When a `NativeOverlay` tile is occluded by a floating window or another pane (e.g., in a tab group where the tile is not the active tab), `WryManager` must hide the native webview window. Showing an occluded native window creates visual corruption.

**Invariant**: `WryManager` tracks active/occluded state per tile and calls `webview.set_visible(false)` when a tile is not the active tab or is fully occluded.

---

## 6. Backend Selection Contract

Wry is selected as the viewer backend when:

1. The node's `address_kind` is `Http` **and**
2. `AppPreferences.wry_enabled = true` **and**
3. `AppPreferences.wry_use_for_addresses` matches the node's address pattern (glob or domain list).

When Wry is not selected (disabled or address not matched), `ServoViewer` is used for `Http` nodes.

**Invariant**: Wry and Servo are mutually exclusive for any given node. A node cannot have both a `WryViewer` and a `ServoViewer` active simultaneously.

**Default**: Wry is disabled by default (`wry_enabled = false`). It must be explicitly enabled in preferences.

---

## 7. Platform Targeting Order

Implementation priority:

| Priority | Platform | Webview engine | Notes |
|----------|----------|---------------|-------|
| 1 | Windows | WebView2 | Primary target; both `NativeOverlay` and off-screen texture available |
| 2 | macOS | WKWebView | Secondary target; both modes available |
| 3 | Linux | WebKitGTK | Tertiary; `NativeOverlay` only; GTK/X11 z-ordering constraints |

**Invariant**: The `wry` crate version is pinned in `Cargo.toml`. Upgrading the version requires verifying that overlay z-ordering and off-screen rendering contracts remain intact on all three platforms.

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| `NativeOverlay` tile has no egui draw calls in content rect | Test: `NativeOverlay` mode → no egui draw commands inside native content rect |
| Occluded native webview is hidden | Test: switch tab away from `NativeOverlay` tile → `webview.set_visible(false)` called |
| `sync_overlay` updates position without blocking | Test: resize tile → webview position update is non-blocking; frame completes on time |
| `CompositedTexture` mode composites texture into egui | Test: `CompositedTexture` mode → Wry frame pixel data appears in egui tile rect |
| Webview destroyed on `Cold` transition | Test: node transitions `Warm → Cold` → webview removed from `WryManager.webviews` pool |
| Webview recreated from snapshot on `Cold → Active` | Test: node reactivated from `Cold` → webview created; scroll position restored |
| Navigation event emits `GraphSemanticEvent::WryNavigation` | Test: navigate webview → `WryNavigation` event in event stream |
| Wry disabled by default | Test: fresh preferences → `wry_enabled = false`; `Http` nodes use `ServoViewer` |
| `WryManager` is the only webview creator/destroyer | Architecture invariant: no `wry::WebView::new` calls outside `WryManager` |
| Linux forces `NativeOverlay` | Test: Linux build → `WryRenderMode` is always `NativeOverlay` |
