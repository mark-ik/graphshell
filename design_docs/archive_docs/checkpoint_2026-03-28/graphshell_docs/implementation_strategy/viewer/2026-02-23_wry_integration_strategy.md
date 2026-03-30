# Wry Integration Strategy: Native Webviews & The Verso Mod

**Date**: 2026-02-23
**Status**: Implementation-Ready (updated 2026-02-26)
**Relates to**:

- `2026-02-22_registry_layer_plan.md` — `ViewerRegistry` (Phase 2, complete) is the contract surface for both backends; `WorkbenchSurfaceRegistry` (Phase 3, complete) owns tile layout policy that drives overlay positioning
- `2026-02-22_multi_graph_pane_plan.md` — pane-hosted multi-view model; Wry applies to Node Viewer panes, not graph panes
- `2026-02-19_ios_port_plan.md` — `wry` is already in scope for the iOS port; coordinate feature-flag usage
- `2026-02-20_cross_platform_sync_and_extension_plan.md` — cross-platform deployment context
- `2026-02-26_composited_viewer_pass_contract.md` — canonical surface-composition contract and Appendix A foundation sequencing (`A.2`, `A.7`, `A.8`, `A.9` dependencies)
- `clipping_and_dom_extraction_spec.md` — backend-neutral clipping contract; Wry and Servo must expose equivalent clip/extract semantics at Graphshell boundary
- `node_viewport_preview_spec.md` — viewport preview tiering; Wry remains preview-only in graph canvas, pane-interactive in workbench

---

## Context

Servo (texture-based rendering) is the primary web backend and the only one currently integrated.
`wry` (native OS webview — WKWebView on macOS/iOS, WebView2 on Windows, WebKitGTK on Linux) provides
an explicit Compatibility Mode backend for cases where the user chooses system-webview realization.
The two backends have
fundamentally different rendering models that constrain where each can be used.

This plan defines how to add `wry` as a second backend under the existing Verso native mod without
splitting user-facing configuration or duplicating shared infrastructure.

In the pane-hosted multi-view model, this is specifically the **Node Viewer pane** backend path.
It should not introduce a separate pane category or bypass pane/compositor routing.

Architectural clarification:

- `viewer:wry` is a **viewer backend** choice for a `Node Viewer pane`, not a distinct semantic
  pane kind.
- A node viewed through Servo and the same node viewed through Wry remain the same promoted node
  and the same pane kind.
- The graph-visible distinction should be pane kind and content kind first; backend/render mode are
  secondary runtime traits that may be surfaced as badges or diagnostics metadata.

---

## Compatibility Mode Invariant

Backend switching between `viewer:webview` (Servo) and `viewer:wry` (system webview) is an
explicit user-intent operation by default.

Invariants:

- No automatic Servo-to-Wry transition based only on internal compatibility heuristics.
- No hidden backend migration during normal lifecycle reconcile.
- Any backend transition must carry an explicit transition reason.
- Active backend and transition reason must be visible in shell/runtime status surfaces.

Compatibility Mode is therefore a user-controlled realization mode, not an involuntary conditional
downgrade path.

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

This rule does **not** imply a separate “Wry node” class. It means that the same node viewer pane
has a backend whose runtime constraints limit live interaction to stable pane regions.

---

## Mod Structure

Verso remains the single "Browser Capability" native mod. Both backends are registered by Verso;
users do not manage them separately.

Verso mod `ModManifest` additions (appended to existing Phase 2 manifest):

- `provides`: add `viewer:wry` alongside existing `viewer:webview`
- `requires`: add `wry` feature gate (see Cargo.toml step below)
- `capabilities`: no change — `network` already declared

`ViewerRegistry` entries after Verso loads with `wry` feature:

| ID | Backend | Mode | Usable in |
| -- | ------- | ---- | --------- |
| `viewer:webview` | Servo | Texture | Graph canvas + workbench tiles |
| `viewer:wry` | wry | Overlay | Workbench tiles only |

`viewer:webview` is the canonical default web viewer ID. Users can still set frame-level or
node-level defaults/overrides to `viewer:wry`.

Non-goal:

- Do not split workbench node-viewer semantics into `ServoPane` vs `WryPane` categories.
- Do not encode backend swaps as graph-node identity changes.
- Do not duplicate node-viewer lifecycle/selection logic per backend when the distinction belongs
  in viewer selection and render mode.

---

## Viewer Trait Contract Extension

**Canonical trait definition**: `universal_content_model_spec.md §3`. The trait sketched in this
plan's original draft is superseded by the spec. Key changes from the draft:

- `render_embedded` has no return value and no `node: &Node` parameter. Node state is received
  at `on_attach` time; per-frame rendering works from cached state only.
- `sync_overlay` signature is `fn sync_overlay(&self, overlay_ctx: &mut OverlayContext)` —
  not `(rect, visible)` directly; rect and visibility are in `OverlayContext`.
- Lifecycle hooks `on_attach`, `on_detach`, `on_navigate` are required methods.

`ServoViewer` implements `render_embedded` (renders into the tile rect), `sync_overlay` is a
no-op, and `is_overlay_mode` returns false.

`WryViewer` implements `render_embedded` (renders thumbnail fallback or placeholder),
`sync_overlay` calling `wry::WebView::set_bounds()` and `set_visible()` via `OverlayContext`,
and `is_overlay_mode` returning true.

---

## Call Sites and Data Flow

### TileCompositor → ViewerRegistry → WryViewer

After `desktop/tile_compositor.rs` computes layout for each frame, it must notify overlay-backed
viewers of their new screen rect. This is a direct call — not a `GraphIntent`, because it is a
layout effect with no semantic meaning, analogous to how egui passes rects to child widgets.

Pane-hosted interpretation: this applies to overlay-backed **node viewer panes** (or transitional
tile equivalents during migration), not graph-pane render paths.

Graph/UI representation guideline:

- graph view should be allowed to show that a promoted node is a `Node Viewer pane`,
- graph view may additionally badge that the effective backend is `viewer:wry`,
- shell overview/status surfaces must show the active backend and last switch reason for the active viewer surface,
- graph view should not treat Wry as a different pane category from Servo.

```rust
// tile_compositor.rs::compose_frame()
  for each tile:
    if tile.render_mode == TileRenderMode::NativeOverlay:
      let screen_rect = tile.computed_screen_rect();
      let visible = tile.is_active_tab() && !tile.is_occluded();
      viewer_registry.sync_overlay(tile.viewer_id, screen_rect, visible);
```

`TileCompositor` should branch from `NodePaneState.render_mode` (`TileRenderMode`) rather than
maintaining a separate overlay-tracking set. Render mode is resolved at viewer attachment time and
serves as the runtime-authoritative source for compositor pass dispatch.

### Graph View Thumbnail Fallback

When `render_graph_in_ui_collect_actions()` renders a node whose viewer ID is overlay-backed,
it calls `render_embedded` on the viewer. `WryViewer::render_embedded` returns false (or renders
the node's `thumbnail_data` if present). The render layer already handles the `false` case for
nodes without a live webview — Wry nodes in graph view use the same path.

No new mechanism is needed. The existing thumbnail pipeline in `Node.thumbnail_data` is the fallback.

---

## Storage Continuity and Backend Switching

Switching between Servo and Wry is a backend transition, not proof that both
backends share one physical storage implementation.

## Backend Transition Authorization

Authorized transition initiators:

- user command (`Open with`, per-node/per-frame/global backend settings)
- recovery prompt acceptance after explicit user confirmation
- policy pinning only when the user has opted into a persistent compatibility preference

Disallowed by default:

- unprompted runtime auto-switches triggered by render anomalies, script errors, or unsupported-feature detection

Transition reason enum (host/runtime boundary):

```rust
pub enum ViewerSwitchReason {
    UserRequested,
    RecoveryPromptAccepted,
    PolicyPinned,
}
```

Rules:

- Servo remains the canonical target for browser-origin storage semantics.
- Wry is a Compatibility Mode backend with its own native profile/session handling.
- Graphshell may coordinate transitions between the two, but should not invent a
  rival browser-storage hierarchy to do so.
- Backend switching must be routed through a host-side policy layer rather than
  through ad hoc assumptions in viewer lifecycle code.

Recommended authority split:

- browser storage truth belongs to a Servo-compatible `ClientStorageManager`
  (or backend-native equivalent while Wry remains compatibility-mode-only)
- Graphshell-owned app durability remains in `GraphStore`
- backend-switch policy belongs to a thin host/runtime orchestration layer
  (`StorageInteropCoordinator`)

Transition policy classes:

| Class | Meaning | Default use |
| --- | --- | --- |
| Shared logical context | Preserve the same logical storage context id across backends | Only when semantics and implementation are known compatible |
| Cloned compatibility context | Copy or approximate relevant state into a backend-specific context | When continuity is desirable but exact sharing is unsafe |
| Isolated compatibility context | Start the target backend with a fresh isolated context/profile | Default when compatibility is uncertain |

Default posture for Wry Compatibility Mode:

- cookies and permissions may be clonable backend-by-backend
- `localStorage` / `sessionStorage` may be clonable, but are not assumed to be
  physically shareable
- IndexedDB, Cache API, OPFS, and service-worker state are not assumed shareable
  between Servo and Wry

This means a command such as `Try in Wry` is the canonical Compatibility Mode
entrypoint. It is user-triggered and explicit. It initiates a backend transition
with declared continuity policy (shared, cloned, or isolated), and does not
imply shared physical browser storage across backends.

Node lifecycle also remains separate from site-data lifecycle:

- deleting a node does not implicitly clear site data
- clearing site data is an explicit storage-context action
- Graphshell may expose a compound action that does both, but only explicitly

---

## Implementation Plan

### Foundation-first sequencing note (2026-02-26)

To avoid over-scoping Wry integration before compositor foundations are stable, execute in this order:

1. Land render-mode and compositor pass correctness (`TileRenderMode` + pass-order diagnostics).
2. Land differential composition / GPU degradation rails for composited mode (`A.8`/`A.7` prerequisites).
3. Land Wry baseline (Steps 1–5 below).
4. Land backend hot-swap (`A.2`) and local telemetry schema (`A.9` groundwork).

`A.9` Verse publication remains deferred until local telemetry quality and privacy boundaries are validated.

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

### Step 4: TileCompositor NativeOverlay Dispatch

Update `desktop/tile_compositor.rs`:

- Add `render_mode: TileRenderMode` to `NodePaneState` (or consume it once added by the viewer-platform lane).
- At viewer attach/detach boundaries, resolve and persist `TileRenderMode` from `ViewerRegistry`.
- In `compose_frame()`: after computing rects, iterate node viewer panes and call `sync_overlay` only
  when `render_mode == TileRenderMode::NativeOverlay`.
- On detachment or mode transition away from `NativeOverlay`, call `sync_overlay(rect, false)` to hide
  the OS window.

Done gate: a wry-backed tile receives `sync_overlay` calls each frame. Moving/resizing the
workbench tile moves the underlying OS webview in sync (manual headed test).

### Step 5: Lifecycle Integration

Wry webviews must respect the same Active/Warm/Cold lifecycle as Servo webviews:

- Active: webview created and visible (`sync_overlay(..., visible: true)`).
- Warm: webview created but hidden (`sync_overlay(..., visible: false)`), or not yet created (cold
  promotion path).
- Cold: webview destroyed; node holds thumbnail only.

In `desktop/lifecycle_reconcile.rs`, add `viewer:wry` handling alongside the existing `viewer:webview`
path. The reconciler checks the node's `viewer_id` preference and calls the appropriate
`WryManager` method.

Done gate: promoting a cold node with `viewer_id = viewer:wry` creates a wry webview in the
workbench tile. Demoting destroys it and the tile transitions away from
`TileRenderMode::NativeOverlay` correctly.

### Step 6: Per-Node and Per-Frame Backend Selection

Users can set a backend preference per node or per frame:

- Node-level: `GraphIntent::SetNodeViewerPreference { node: NodeKey, viewer_id: ViewerId }`.
  Stored on `Node.viewer_id_override: Option<ViewerId>`. Persisted to the graph WAL (fjall) as a
  node metadata update.
- Frame-level: stored in `FrameManifest` as `viewer_id_default: Option<ViewerId>`.
  Falls back to canonical `viewer:webview` if absent.
- Resolution order: node override → frame default → `viewer:webview`.

Done gate: setting `viewer_id_override` on a node to `viewer:wry` causes the next lifecycle
reconcile to use `WryManager` for that node. Contract test covers resolution order.

Clarification:

`viewer_id_override = viewer:wry` selects a viewer backend. It does not, by
itself, define whether the transition is shared-context, cloned-context, or
isolated-compatibility. That decision belongs to runtime storage interop policy.

### Step 7: Settings UI

Expose backend selection in the settings UI:

- Global default: "Default web backend" dropdown in Settings → Web → Rendering, showing `viewer:webview`
  and `viewer:wry` (changes the default selection target for new web nodes).
- Per-node: context menu → "Open with" → "Servo" / "wry". Dispatches `SetNodeViewerPreference`.
- Per-frame: frame settings page, "Default backend for this frame".

Done gate: changing the global default persists across restarts. Per-node override appears in node
context menu and takes effect on next lifecycle reconcile.

### Compatibility Policy Acceptance Criteria

- Servo remains the default backend unless user preference says otherwise.
- No silent backend changes occur during normal lifecycle reconcile.
- Every backend transition records `ViewerSwitchReason`.
- Recovery prompt transitions are explicit, reversible, and session-scoped by default.
- Shell/runtime status exposes the active backend plus transition reason.

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

## Failure Recovery Prompt Contract

If Servo fails hard for a target (for example repeated crash on the same page), Graphshell may
present a recovery prompt:

- `This page failed in Servo. Open in Compatibility Mode (system webview)?`

Rules:

- The prompt is opt-in; there is no automatic persistent switch.
- If accepted, the transition reason is `RecoveryPromptAccepted`.
- Recovery switches are session-scoped by default unless the user explicitly chooses to persist them.
- UI must provide `Try Servo Again` so the user can reverse Compatibility Mode for that surface.

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

The only novel infrastructure required is `TileRenderMode`-driven compositor dispatch in
`TileCompositor` and `WryManager` as a coordinator. Everything else — lifecycle, thumbnail fallback,
settings persistence, node identity — reuses existing mechanisms.

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
- `WryManager` data model (`HashMap<NodeKey, wry::WebView>`) and overlay-mode compositor dispatch
  in `TileCompositor` made concrete.
- Lifecycle integration with `lifecycle_reconcile.rs` and Active/Warm/Cold model described.
- Per-node (`Node.viewer_id_override`) and per-frame (`FrameManifest.viewer_id_default`)
  backend selection defined with resolution order and `GraphIntent` variant.
- Risks: z-ordering, resize jitter, scale factor changes, stale thumbnails, feature-flag drift.
- Thumbnail fallback for graph view aligned to existing `Node.thumbnail_data` pipeline.
- `wry` already noted in iOS port plan; feature-flag approach must be coordinated there.
