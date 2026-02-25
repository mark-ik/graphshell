# Copilot Implementation Guides

Generated 2026-02-25 during PR review. These guides cover the open Copilot PRs
that had empty "Initial plan" commits. Apply to the corresponding `copilot/`
branches.

---

## Already Implemented (apply these to copilot branches)

### #47 wire-ctrl-click-multi-select (`copilot/wire-ctrl-click-multi-select`)

**File:** `render/mod.rs:125`

The infrastructure is already in place — `multi_select_modifier` flows through
`collect_graph_actions` → `GraphAction::SelectNode { multi_select }` →
`GraphIntent::SelectNode { multi_select }` → `SelectionState::select()`.

**One-line fix:**
```rust
// Before (line 125):
let ctrl_pressed = ui.input(|i| i.modifiers.ctrl);
// After:
let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
```

Extend ctrl detection to include `i.modifiers.command` so macOS Cmd+Click
also triggers multi-select. No other changes needed.

---

### #49 replace-debug-titles-with-semantic-labels (`copilot/replace-debug-titles-with-semantic-labels`)

**File:** `shell/desktop/workbench/tile_render_pass.rs:84-93`

Replace the `tile_hierarchy_lines` format strings in the diagnostics display:

```rust
// Tabs container:
format!("Tab Group ({} tabs)", tabs.children.len())

// Linear container:
use egui_tiles::LinearDir;
let dir_label = match linear.dir {
    LinearDir::Horizontal => "Split ↔",
    LinearDir::Vertical => "Split ↕",
};
format!("{} ({} panes)", dir_label, linear.children.len())

// Generic container:
format!("Panel Group ({:?})", other.kind())
```

See `shell/desktop/workbench/tile_behavior.rs:409` for existing `LinearDir` usage.

---

### #48 add-channel-severity-to-descriptors (`copilot/add-channel-severity-to-descriptors`)

**File:** `registries/atomic/diagnostics.rs`

**Step 1** — Add enum before `DiagnosticChannelDescriptor`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ChannelSeverity {
    #[default]
    Info,
    Warn,
    Error,
}
```

**Step 2** — Add field to both descriptor structs:
```rust
pub(crate) struct DiagnosticChannelDescriptor {
    pub(crate) channel_id: &'static str,
    pub(crate) schema_version: u16,
    pub(crate) severity: ChannelSeverity,  // ADD
}

pub(crate) struct RuntimeChannelDescriptor {
    // ... existing fields ...
    pub(crate) severity: ChannelSeverity,  // ADD
}
```

**Step 3** — Propagate in `RuntimeChannelDescriptor::from_contract`:
```rust
severity: descriptor.severity,
```

**Step 4** — Update all `DiagnosticChannelDescriptor { ... }` struct literals to
include `severity:`. Use these defaults:
- `*_FAILED`, `*_DENIED`, `*_REJECTED`, `*_UNAVAILABLE` → `ChannelSeverity::Error`
- `*_FALLBACK_USED`, `*_MISSING`, `*_CONFLICT`, `*_TIMEOUT`, `*_LIMIT` → `ChannelSeverity::Warn`
- Everything else → `ChannelSeverity::Info`

**Step 5** — Add `severity: ChannelSeverity::Info` to all `RuntimeChannelDescriptor { ... }`
literals (mod/verse registrations default to Info; callers can set severity via API).

---

## Concrete Features (require implementation)

### #50 add-zoom-adaptive-label-lod (`copilot/add-zoom-adaptive-label-lod`)

**Current state:** `model/graph/egui_adapter.rs:414-446` already implements
zoom-adaptive labels with thresholds at `0.6` (no labels) and `1.5`
(domain vs full labels).

**What's needed:** Make the thresholds configurable via `CanvasStylePolicy`
so they can be tuned per canvas profile.

**Files:**
- `registries/domain/layout/canvas.rs:31-33` — extend `CanvasStylePolicy`:
  ```rust
  pub(crate) struct CanvasStylePolicy {
      pub(crate) labels_always: bool,
      pub(crate) label_lod_enabled: bool,    // ADD: enable zoom-adaptive LOD
      pub(crate) label_hide_below: f32,       // ADD: default 0.6
      pub(crate) label_domain_below: f32,     // ADD: default 1.5
  }
  ```
- `model/graph/egui_adapter.rs:418-446` — pass thresholds into
  `label_text_for_zoom_value()` instead of hardcoding `0.6` / `1.5`.
- `render/mod.rs:106-112` — thread canvas profile policy into the adapter
  when building egui graph state.

---

### #51 add-viewport-culling-policy (`copilot/add-viewport-culling-policy`)

**Files:**
- `registries/domain/layout/canvas.rs` — add culling toggle to `CanvasTopologyPolicy`:
  ```rust
  pub(crate) struct CanvasTopologyPolicy {
      pub(crate) viewport_culling_enabled: bool,  // ADD: default true
      pub(crate) label_lod_policy_enabled: bool,  // ADD
  }
  ```
- `render/spatial_index.rs:58` — `nodes_in_canvas_rect()` is already implemented.
  Wire it in `render/mod.rs` where graph nodes are rendered: skip nodes outside
  the viewport rect when `viewport_culling_enabled`.
- `registries/domain/layout/canvas.rs:65-100` — update `resolve()` to include
  the new fields in `CanvasSurfaceResolution`.

---

### #52 add-node-mime-address-hints (`copilot/add-node-mime-address-hints`)

**Context:**
- `registries/atomic/viewer.rs` — `ViewerRegistry` with MIME/extension → viewer
  mappings. `select_for_uri()` already uses MIME hint.
- `shell/desktop/workbench/pane_model.rs:113` — `NodePaneState` has
  `viewer_id_override: Option<ViewerId>`.

**What's needed:**
1. Add `mime_hint: Option<String>` to `graph::Node` (in `graph/mod.rs`).
2. When a node is navigated/loaded, detect MIME from content-type response and
   store it in `node.mime_hint`.
3. In `pane_model.rs`, when opening a node viewer, call
   `viewer_registry.select_for_uri(node.url, node.mime_hint.as_deref())` to
   pick the appropriate viewer.
4. Add `viewer:plaintext` descriptor to `ViewerRegistry::core_seed()` for
   `text/plain` MIME type (already has `mime_hint` in `ViewerDescriptor`).

---

### #53 implement-multi-view-state (`copilot/implement-multi-view-state`)

**Context:** Infrastructure already exists!
- `app.rs:79-91` — `GraphViewId` is defined.
- `app.rs:217-228` — `GraphViewState { id, name, camera, lens, layout_mode, ... }`.
- `app.rs:1183,1186` — `workspace.views: HashMap<GraphViewId, GraphViewState>`,
  `workspace.focused_view: Option<GraphViewId>`.
- `pane_model.rs:90-104` — `GraphPaneRef` links a pane to a `GraphViewId`.
- `pane_model.rs:74-88` — `ViewLayoutMode { Canonical, Divergent }`.

**What's needed:** Wire the `GraphPaneRef.view_id` through the render pipeline so
each graph pane reads its own `GraphViewState` (camera, lens) rather than the
shared workspace state.

1. In `tile_render_pass.rs`, when rendering a graph tile, read the tile's
   `GraphPaneRef.view_id` and look up `app.workspace.views[view_id]` for
   camera/lens state.
2. Add `GraphIntent::CreateGraphView { name }` and
   `GraphIntent::SwitchPaneToView { pane_id, view_id }` intents.
3. Handle `SetViewLens` (already exists in `GraphIntent`) to update per-view lens.

---

### #39 improve-graph-viewport-culling (`copilot/improve-graph-viewport-culling`)

**Context:** `render/spatial_index.rs` already implements R*-tree spatial
indexing with `nodes_in_canvas_rect()`.

**What's needed:**
1. In `render/mod.rs`, after building `egui_state`, apply viewport culling:
   use `app.workspace.spatial_index` (if it exists) or build one from node
   positions to get only visible node keys.
2. Pass only visible nodes to the egui_graphs renderer.
3. Add WebView accessibility bridge: when the graph has a selected/focused node,
   emit an accessibility event with the node's URL and title via AccessKit or
   a platform accessibility API.

---

## Roadmap / Planning Issues (design docs only)

These issues track future architectural directions. The "Initial plan" commit is
the appropriate deliverable. For each, add a design doc stub that captures the
acceptance criteria as a tracked milestone:

### #41 adopt-unified-omnibar
- Unified omnibar: URL bar + graph search + web search in one input.
- Add planning stub to `design_docs/.../implementation_strategy/`.

### #53-61 (roadmap adopt-* issues)
- Each needs a design doc confirming the concept is tracked as a roadmap item.
- See existing `2026-02-25_interactive_html_export_plan.md` for format reference.

---

## PRs Blocked on Conflicts

### #42 refactor-radial-menu-module / #45 refactor-extract-command-palette-module

These PRs conflict with the merged #38 (`unify-command-palette-radial-menu`).
#38 already extracts both modules (`render/radial_menu.rs` and
`render/command_palette.rs`) via `ActionRegistry`. PRs #42 and #45 should be
**closed** as superseded, or rebased onto main if additional changes are needed
beyond what #38 already delivers.
