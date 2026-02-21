# Graphshell Module Map

Quick reference for navigating the current graphshell codebase.

**Base:** project root (standalone crate; no local Servo checkout required)
**Build:** `cargo check`, `cargo test`, `cargo run`

---

## Core Runtime Modules

### Graph and Reducer

- `graph/mod.rs`
- `graph/egui_adapter.rs`
- `app.rs`

Responsibilities:
- Graph model (`Node`, `EdgeType`, UUID identity)
- Lifecycle desired state (`Active` / `Warm` / `Cold`)
- Reducer intent handling (`GraphIntent`)
- Workspace membership/routing, persistence hooks, undo/redo checkpoints

### UI and Frame Orchestration

- `desktop/gui.rs`
- `desktop/gui_frame.rs`
- `desktop/toolbar_ui.rs`
- `render/mod.rs`

Responsibilities:
- Frame sequencing and intent apply boundaries
- Omnibar, command/radial/help/persistence panels
- Graph/tile rendering and interaction wiring

### Tile and Lifecycle Runtime

- `desktop/tile_runtime.rs`
- `desktop/tile_render_pass.rs`
- `desktop/tile_compositor.rs`
- `desktop/lifecycle_reconcile.rs`
- `desktop/webview_backpressure.rs`
- `desktop/webview_controller.rs`

Responsibilities:
- Tile tree node/pane mechanics
- Reconcile desired lifecycle against runtime webview state
- Retry/backpressure handling for webview creation

### Embedder and Window Integration

- `running_app_state.rs`
- `window.rs`
- `desktop/app.rs`
- `desktop/event_loop.rs`
- `desktop/headed_window.rs`
- `desktop/headless_window.rs`

Responsibilities:
- Servo delegate integration
- Platform window and webview collection management
- Graph semantic event emission from delegate callbacks

### Protocols and Routing

- `desktop/protocols/mod.rs`
- `desktop/protocols/router.rs`
- `desktop/protocols/resource.rs`
- `desktop/protocols/servo.rs`
- `desktop/protocols/urlinfo.rs`

Responsibilities:
- Servo protocol registration (`resource://`, `servo://`, `urlinfo://`)
- Outbound scheme router used by toolbar/provider suggestion fetches

### Persistence

- `persistence/mod.rs`
- `persistence/types.rs`
- `desktop/persistence_ops.rs`

Responsibilities:
- Fjall mutation log + redb snapshots + rkyv serialization
- Session/workspace snapshot persistence and maintenance utilities

---

## Key Data Flows

### Delegate Event -> Reducer

1. Servo delegate callback fires in `running_app_state.rs`.
2. `window.rs` records `GraphSemanticEvent`.
3. `desktop/gui_frame.rs` drains events via `window.take_pending_graph_events()`.
4. `desktop/semantic_event_pipeline.rs` converts events into `GraphIntent`.
5. `GraphBrowserApp::apply_intents` mutates reducer-owned state.

### Lifecycle Reconcile Loop

1. Reducer-owned lifecycle intent/state is applied.
2. `desktop/lifecycle_reconcile.rs` compares desired state with runtime mappings.
3. Reconcile emits lifecycle intents (`MapWebviewToNode`, `UnmapWebview`, promotions/demotions).
4. Intents are applied at the frame boundary.

### Toolbar Provider Suggestions

1. `desktop/toolbar_ui.rs` builds suggestion URL.
2. `desktop/protocols/router.rs` dispatches by outbound scheme.
3. `reqwest` fetches provider payload.
4. Toolbar parses payload into omnibar matches.

---

## Current Invariants

1. Identity is UUID-based; URLs are mutable metadata and may be duplicated.
2. Webview/node mappings are bidirectional and must remain inverse-consistent.
3. Lifecycle transitions are intent-driven at the reducer boundary.
4. Reconcile is side-effectful and should emit intents rather than mutating reducer state directly.
5. Workspace membership is UUID-keyed and derived from persisted workspace layouts.

---

## Large Files (Read in Sections)

- `render/mod.rs` (~3.1k)
- `app.rs` (~5.4k)
- `desktop/toolbar_ui.rs` (~2.7k)
- `desktop/gui.rs` (~1.7k)
- `desktop/gui_frame.rs` (~1.1k)

---

## Practical Debug Entry Points

- Lifecycle/reconcile bugs: `desktop/lifecycle_reconcile.rs`, `desktop/webview_backpressure.rs`
- Reducer transition bugs: `app.rs` (`GraphIntent` + lifecycle helpers)
- Delegate ordering/semantic conversion: `window.rs`, `desktop/semantic_event_pipeline.rs`
- Toolbar/provider fetch issues: `desktop/toolbar_ui.rs`, `desktop/protocols/router.rs`
- Workspace routing/membership issues: `app.rs`, `desktop/gui_frame.rs`, `desktop/persistence_ops.rs`

