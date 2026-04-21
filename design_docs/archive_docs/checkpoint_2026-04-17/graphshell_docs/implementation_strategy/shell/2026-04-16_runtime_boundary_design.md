<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# M3.5 Runtime Boundary Design Pass

**Date**: 2026-04-16
**Status**: Archived 2026-04-17
**Scope**: Classify every `Gui` responsibility into runtime, host-adapter,
render-backend, or OS-integration buckets, and specify the service-port and
view-model surfaces that M4 will extract.

**Archive note**:

- M3.5 is complete; this document is retained as the landed design receipt for the runtime/host boundary classification.
- Active migration execution remains in `2026-04-14_iced_host_migration_execution_plan.md` and the follow-on shell cleanup plans.
- The host/runtime seam defined here is already referenced by the runtime scaffolding in `shell/desktop/ui/host_ports.rs`, `frame_model.rs`, and `gui_state.rs`.

**Related**:

- [`2026-04-14_iced_host_migration_execution_plan.md`](2026-04-14_iced_host_migration_execution_plan.md) — source of the M3.5 mandate (§5, M3.5 block)
- [`SHELL.md`](SHELL.md) — shell subsystem policy
- [`../aspect_render/render_backend_contract_spec.md`](../aspect_render/render_backend_contract_spec.md) — render backend boundary
- [`../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`](../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md) — UI framework comparison

---

## 1. Context

Post-M3 the compositor is host-neutral at its public surface, but `Gui`
(~1260 lines in [`shell/desktop/ui/gui.rs`](../../../../shell/desktop/ui/gui.rs))
still bundles four distinct concerns under a single owner:

1. **Durable runtime logic** — graph model, workbench membership/layout,
   focus authority, toolbar session state, async worker supervision.
2. **Host-adapter state** — egui-specific widget state, texture caches,
   `egui_tiles::Tree` layout projection.
3. **Render-backend glue** — Servo rendering contexts, content-surface
   registry, thumbnail capture plumbing.
4. **OS/window/event-loop integration** — winit window, Servo running-app
   state, frame inbox.

M4 ("Extract a Host-Neutral Shell Runtime") splits this into a `Runtime` owned
by the core and a `Host` owned by the adapter (egui today, iced later). Before
M4 implementation, this document specifies the extraction line explicitly so
the work is a mechanical move rather than ad hoc negotiation.

---

## 2. Field Classification

The `Gui` struct today (verified against [`shell/desktop/ui/gui.rs`](../../../../shell/desktop/ui/gui.rs)
lines 144–234 and `GuiRuntimeState` in [`gui_state.rs:116`](../../../../shell/desktop/ui/gui_state.rs)):

### A. Durable runtime logic (moves to `GraphshellRuntime`)

| Field | Role |
|------|------|
| `graph_app: GraphBrowserApp` | Core app state — graph, selection, intents |
| `graph_tree: GraphTree<NodeKey>` | Workbench membership + layout authority |
| `workbench_view_id: GraphViewId` | Persistence slot identity |
| `toolbar_state: ToolbarState` | Toolbar session state (text, cursor, drafts) |
| `bookmark_import_dialog: Option<BookmarkImportDialogState>` | Dialog session (not rendering) |
| `control_panel: ControlPanel` | Async worker supervisor + intent queue |
| `registry_runtime: Arc<RegistryRuntime>` | Semantic services runtime |
| `tokio_runtime: Runtime` | Async runtime (host-agnostic) |
| `viewer_surfaces: ViewerSurfaceRegistry` | Content-surface authority (Phase D, host-neutral) |
| `webview_creation_backpressure` | Runtime backpressure state |
| `frame_inbox: GuiFrameInbox` | Async signal bridges into shell |

From `GuiRuntimeState` (currently nested inside `Gui`):

| Field | Role |
|------|------|
| `focus_authority: RuntimeFocusAuthorityState` | Focus policy truth |
| `focused_node_hint: Option<NodeKey>` | Focus hint carried across frames |
| `graph_surface_focused: bool` | Focus mode toggle |
| `focus_ring_{node_key,started_at,duration}` | Focus ring timing state |
| `command_palette_toggle_requested: bool` | Pending UI command |
| `toolbar_drafts: HashMap<PaneId, ToolbarDraft>` | Per-pane toolbar input drafts |
| `omnibar_search_session` | Omnibar session state |
| `graph_search_*` (5 fields) | Graph search query/results/navigation |
| `pending_webview_context_surface_requests` | Deferred surface routing |
| `deferred_open_child_webviews` | Webview lifecycle queue |

### B. Host-adapter state (stays with `EguiHost`; mirrored for `IcedHost`)

| Field | Role |
|------|------|
| `context: UiRenderBackendHandle` | egui + wgpu bridge |
| `toasts: egui_notify::Toasts` | egui toast notification widget |
| `renderer_favicon_textures: RendererFaviconTextureCache` | egui texture cache |
| `tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)>` | egui-keyed favicon map |
| `pending_webview_a11y_updates` | accesskit bridge (accesskit is cross-host; injection point is host-specific) |
| `tiles_tree: Tree<TileKind>` | egui_tiles layout projection (retires in M7) |
| `clipboard: Option<Clipboard>` | Currently via arboard; could lift to runtime if iced uses same crate |

### C. Render-backend glue (shared infrastructure, host-neutral)

| Field | Role |
|------|------|
| `rendering_context: Rc<OffscreenRenderingContext>` | Servo offscreen render context |
| `window_rendering_context: Rc<WindowRenderingContext>` | Servo window render context |
| `thumbnail_capture_tx/rx` | Thumbnail capture channel |
| `thumbnail_capture_in_flight: HashSet<WebViewId>` | Thumbnail request tracking |

### D. OS/window integration (`EmbedderWindow` layer; already separated)

| Field | Role |
|------|------|
| `state: Option<Rc<RunningAppState>>` | Servo running-app state reference |
| `toolbar_height: Length<f32, DeviceIndependentPixel>` | Chrome layout constant |

Note: the actual `winit::Window` and `EmbedderWindow` are not fields of `Gui` —
they're passed in per frame. That separation is already correct.

### E. Diagnostics (feature-gated, cross-cutting)

| Field | Role |
|------|------|
| `#[cfg(feature = "diagnostics")] diagnostics_state` | Cross-layer telemetry |

Stays with runtime (diagnostics is not host-specific).

---

## 3. Boundary Tables by Responsibility

For each cross-cutting concern currently mixed through `Gui`, specify who
owns what.

### 3.1 Focus authority

| Concern | Owner after M4 |
|---------|----------------|
| Focus policy (which surface may take focus, return targets) | **Runtime** — `focus_authority: RuntimeFocusAuthorityState` |
| Focus hint (focused_node_hint) | **Runtime** |
| Graph-surface vs. node-pane focus mode | **Runtime** — `graph_surface_focused` |
| Focus ring visual (timing, animation) | **Runtime** state, **Host** renders |
| Pointer hover position | **Host** samples, passes to runtime per-frame |
| egui keyboard/pointer `wants_input` | **Host** — per-frame query, surfaced as input policy to runtime |
| accesskit focus node id | **Host** derives from runtime view-model |

### 3.2 Command routing

| Concern | Owner after M4 |
|---------|----------------|
| Pending workbench intents queue | **Runtime** (already in `graph_app`) |
| Keyboard action → intent translation | **Runtime** — takes raw `KeyAction`, produces `GraphIntent` |
| Command palette toggle | **Runtime** — state; **Host** — rendering |
| Action registry resolution | **Runtime** — `registry_runtime` |

### 3.3 Toolbar / omnibar session state

| Concern | Owner after M4 |
|---------|----------------|
| Toolbar drafts (per-pane text, cursor) | **Runtime** — `toolbar_drafts` |
| Omnibar search session | **Runtime** — `omnibar_search_session` |
| Toolbar input widget focus / IME state | **Host** — transient, not runtime |
| Toolbar chrome layout | **Host** — renders from runtime view-model |

### 3.4 Pane targeting

| Concern | Owner after M4 |
|---------|----------------|
| `active_pane_rects` (cached from GraphTree) | **Runtime** (already landed in M3) |
| `pane_render_modes`, `pane_viewer_ids` | **Runtime** (already landed in M3) |
| PaneId ↔ TileId bridging | **Host** — only while egui_tiles survives (retires in M7) |
| Which pane is "active" | **Runtime** — derived from GraphTree + focus authority |

### 3.5 Thumbnail / update queues

| Concern | Owner after M4 |
|---------|----------------|
| Thumbnail capture request queue | **Runtime** — request-side state |
| Thumbnail capture channel (tx/rx) | **Render backend** — shared infra |
| `thumbnail_capture_in_flight` tracking | **Runtime** |
| Favicon texture cache | **Host** — per-framework texture handles |

### 3.6 Compositor-facing services

| Concern | Owner after M4 |
|---------|----------------|
| `ViewerSurfaceRegistry` (content-surface state) | **Runtime** (already host-neutral since M3) |
| Content callback registration | **Runtime** → **Render backend** via `BackendGraphicsContext` |
| Overlay descriptors (`OverlayStrokePass`) | **Runtime** produces; **Host** paints |
| Overlay painting operations | **Host** — reimplements against its painter |

---

## 4. Service Ports (runtime ← host boundary)

The runtime needs a small set of capabilities from whatever host is driving it.
These are the **ports**. Each host implements them against its own primitives.

### `HostInputPort` — raw input ingress

```rust
trait HostInputPort {
    fn poll_events(&mut self) -> Vec<HostEvent>;
    fn pointer_hover_position(&self) -> Option<ScreenPoint>;
    fn wants_keyboard_input(&self) -> bool; // does a host widget have focus?
    fn wants_pointer_input(&self) -> bool;
    fn modifiers(&self) -> ModifiersState;
}
```

`HostEvent` already exists in [`shell/desktop/workbench/ux_replay.rs:13`](../../../../shell/desktop/workbench/ux_replay.rs#L13)
— that type is the canonical host-neutral event, reuse it.

### `HostSurfacePort` — surface mounting and presentation

```rust
trait HostSurfacePort {
    /// Allocate/reuse a content surface for a node.
    fn ensure_content_surface(&mut self, node_key: NodeKey, size: Size);
    /// Notify host that a surface's content has changed.
    fn present_surface(&mut self, node_key: NodeKey);
    /// Retire a surface (node closed or tombstoned).
    fn retire_surface(&mut self, node_key: NodeKey);
    /// Register a content callback that paints on-demand.
    fn register_content_callback(
        &mut self,
        node_key: NodeKey,
        callback: Arc<dyn Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync>,
    );
}
```

The callback signature already exists and is host-neutral — see
[`compositor_adapter.rs:281`](../../../../shell/desktop/workbench/compositor_adapter.rs#L281).

### `HostPaintPort` — draw commands against whatever painter the host has

```rust
trait HostPaintPort {
    fn draw_overlay_stroke(&mut self, node_key: NodeKey, rect: Rect, stroke: Stroke, rounding: f32);
    fn draw_overlay_glyphs(&mut self, node_key: NodeKey, rect: Rect, glyphs: &[GlyphOverlay], color: Color);
    fn draw_degraded_receipt(&mut self, rect: Rect, message: &str);
    // ... matches OverlayStrokePass descriptor variants
}
```

egui implements this against `egui::Context::layer_painter`. iced implements
it against its renderer's drawing primitives. The `OverlayStrokePass`
descriptor (today in [`compositor_adapter.rs:680`](../../../../shell/desktop/workbench/compositor_adapter.rs#L680))
is already the host-neutral intent; only its `EguiRect`/`Stroke` field types
still pull egui into the descriptor — those are the last cosmetic leak to
address (non-blocking; can be a follow-on).

### `HostTexturePort` — favicon/image cache

```rust
trait HostTexturePort {
    type TextureHandle: Clone; // egui::TextureHandle or iced equivalent
    fn load_texture(&mut self, key: &str, data: &[u8]) -> Self::TextureHandle;
    fn drop_texture(&mut self, key: &str);
}
```

### `HostClipboardPort` — clipboard access

```rust
trait HostClipboardPort {
    fn get(&mut self) -> Option<String>;
    fn set(&mut self, text: &str);
}
```

Both egui and iced use arboard under the hood; this port is effectively
already host-neutral — just needs lifting out of `Gui`.

### `HostToastPort` — transient notifications

```rust
trait HostToastPort {
    fn enqueue(&mut self, severity: ToastSeverity, message: String, duration: Duration);
}
```

Today backed by `egui_notify::Toasts`. iced has its own notification system.

### `HostAccessibilityPort` — accesskit bridging

```rust
trait HostAccessibilityPort {
    fn inject_tree_update(&mut self, webview_id: WebViewId, update: accesskit::TreeUpdate);
    fn request_focus(&mut self, node_id: accesskit::NodeId);
}
```

accesskit is cross-host but injection differs per framework.

---

## 5. View-Model Surface (runtime → host)

Each frame the runtime produces a read-only view-model the host renders.
This is what replaces the current direct `graph_app` / `graph_tree` poking
from host code.

```rust
/// Snapshot produced by the runtime once per frame, consumed by the host
/// to paint its chrome. All fields are host-neutral.
pub struct FrameViewModel {
    /// Active workbench layout: visible panes + their rects.
    pub active_pane_rects: Vec<(PaneId, NodeKey, Rect)>,

    /// GraphTree rows for sidebar rendering.
    pub tree_rows: Vec<OwnedTreeRow<NodeKey>>,

    /// Tab order for flat tab rendering.
    pub tab_order: Vec<TabEntry<NodeKey>>,

    /// Split boundaries (draggable handles).
    pub split_boundaries: Vec<SplitBoundary<NodeKey>>,

    /// Currently active pane member.
    pub active: Option<NodeKey>,

    /// Focus state: what is focused, which mode, ring timing.
    pub focus: FocusViewModel,

    /// Toolbar: current text, cursor, drafts per pane.
    pub toolbar: ToolbarViewModel,

    /// Overlays the host must paint this frame (`OverlayStrokePass` descriptors).
    pub overlays: Vec<OverlayStrokePass>,

    /// Dialogs currently open (command palette, bookmark import, settings, etc).
    pub dialogs: DialogsViewModel,

    /// Toast queue (messages to display).
    pub toasts: Vec<ToastSpec>,

    /// Content surfaces that need presenting this frame.
    pub surfaces_to_present: Vec<NodeKey>,

    /// Degraded receipts (UX-visible diagnostic messages).
    pub degraded_receipts: Vec<DegradedReceipt>,
}
```

Most of these types already exist in the codebase as compositor input data.
This view-model is a re-assembly, not a greenfield design.

### Intent/event flow back to runtime

```rust
/// Input events and host-layer decisions flowing into the runtime.
pub struct FrameHostInput {
    pub events: Vec<HostEvent>,
    pub pointer_hover: Option<ScreenPoint>,
    pub viewport_size: Size,
    pub wants_keyboard: bool,
    pub wants_pointer: bool,
    pub modifiers: ModifiersState,
}
```

The runtime consumes `FrameHostInput`, produces `FrameViewModel` plus any
side-effects (webview creation requests, clipboard writes, etc. via ports).

---

## 6. Intentionally Host-Specific Residue

After M4, the host keeps these concerns — they're not drift, they're
fundamentally framework-coupled:

- **Painting primitives** — `egui::Context::layer_painter(...)` and friends.
  iced will implement its own equivalents.
- **Widget state machines** — text input cursor blink, scroll momentum,
  drag-and-drop gestures. Too tied to the widget library to lift cleanly.
- **Texture handle types** — `egui::TextureHandle` vs iced equivalent.
  Abstracted behind `HostTexturePort::TextureHandle` associated type.
- **Layer ordering / z-index** — `egui::LayerId` vs iced's painting order.
  Each host manages its own stacking.
- **Repaint request mechanism** — `ctx.request_repaint_after(duration)`
  vs iced's subscription model. Abstract via an `on_dirty` callback.
- **Framework tile tree** — `egui_tiles::Tree<TileKind>` stays in the egui
  host until M7 retires it. iced host will never have one; it renders panes
  directly from `active_pane_rects` in the view-model.
- **Event loop integration** — how frames are scheduled. egui runs inside
  winit's `AboutToWait`; iced has its own event loop. Abstract at the host
  level, not at the runtime level.
- **IME/text input** — OS keyboard delivery differs per framework.
- **Favicon/toast rendering** — host uses its widget library to render these
  visually, but the *state* (which toasts, which favicons) lives in the runtime.

---

## 7. Target Architecture

```text
                 ┌─────────────────────────────────────────┐
                 │          GraphshellRuntime              │
                 │                                         │
                 │  - graph_app: GraphBrowserApp           │
                 │  - graph_tree: GraphTree                │
                 │  - focus_authority: RuntimeFocusState   │
                 │  - toolbar_state: ToolbarState          │
                 │  - control_panel: ControlPanel          │
                 │  - viewer_surfaces: ViewerSurfaceReg    │
                 │  - registry_runtime: Arc<RegistryRt>    │
                 │  - tokio_runtime: Runtime               │
                 │                                         │
                 │  fn tick(input: FrameHostInput,         │
                 │          ports: &mut dyn HostPorts)     │
                 │    -> FrameViewModel                    │
                 └──────────────┬──────────────────────────┘
                                │
                                │ FrameViewModel / FrameHostInput
                                │ HostInputPort, HostSurfacePort,
                                │ HostPaintPort, HostTexturePort,
                                │ HostClipboardPort, HostToastPort,
                                │ HostAccessibilityPort
                                │
     ┌──────────────────────────┴──────────────────────────┐
     │                                                     │
┌────┴────────────────┐                         ┌──────────┴──────────┐
│    EguiHost         │                         │    IcedHost          │
│                     │                         │    (future)          │
│  - egui::Context    │                         │  - iced::Element     │
│  - tiles_tree       │                         │  - iced renderer     │
│  - texture caches   │                         │  - texture caches    │
│  - Toasts (egui)    │                         │  - iced notifications│
│  - egui_tiles Tree  │                         │  - native panes      │
└─────────────────────┘                         └─────────────────────┘
```

---

## 8. Extraction Sequence (M4 plan sketch)

M4 will implement this extraction in order:

1. **Extract `GraphshellRuntime` struct** — move Category A fields from `Gui`
   into a new type. `Gui` keeps references/borrows.
2. **Define `HostPorts` trait bundle** — the six `Host*Port` traits above.
3. **Implement `HostPorts` for egui** — back each port by its current
   implementation (`EguiHostPorts` struct wrapping today's state).
4. **Define `FrameViewModel` and `FrameHostInput`** — tie-break on existing
   types where possible.
5. **Refactor frame pipeline** — `Runtime::tick(input, ports) -> view_model`;
   egui host renders view_model using ports.
6. **Verify parity** — replay harness confirms no behavioral drift.
7. **Delete runtime code paths from egui host** — state that now lives in
   the runtime is removed from host structs.

Each step is independently testable; the parity harness (M0) is how we
detect regression.

---

## 9. Explicit Non-Goals

This design pass intentionally does **not** decide:

- **Whether `Runtime` lives in its own crate.** Could be a module in
  `graphshell` for M4; crate extraction is a follow-on if useful.
- **Whether ports are synchronous or async.** Current code is synchronous;
  port traits can be `async fn` later if needed.
- **Whether iced comes before chrome redesign.** That's a product-level
  decision separate from this extraction.
- **The fate of `tiles_tree`.** M7 retires it; until then it's host-local
  in the egui host only.

---

## 10. Acceptance Shape

M3.5 is complete when this document exists and is approved. The extraction
line is explicit; M4 can begin with a mechanical execution plan rather than
discovering the boundary as it goes.

M4 is complete when:

- `GraphshellRuntime` owns all Category A fields
- `Gui` is renamed to `EguiHost` and only holds Categories B + D
- `HostPorts` traits are defined and implemented by `EguiHost`
- Frame pipeline runs via `runtime.tick(input, ports) -> view_model`
- All tests that passed before M4 still pass after

---

## 11. Summary

The extraction boundary runs between:

- **What survives host migration** (the runtime): graph model, workbench
  authority, focus policy, toolbar session, async supervision,
  content-surface registry, view-model assembly.
- **What gets rewritten per host**: painting, widget state, texture
  caches, event-loop integration, framework-specific notifications and
  IME.

M4's job is to make that boundary visible in the type system. M3.5 makes
it explicit in specification so M4 is a mechanical move.
