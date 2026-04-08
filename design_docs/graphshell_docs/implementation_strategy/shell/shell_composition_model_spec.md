<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Shell Composition Model Spec

**Date**: 2026-03-25
**Status**: Canonical / Active — Phases 1–4 implemented
**Scope**: How Shell mounts and composes its major surfaces using egui's
named-panel system; how `egui_tiles` is scoped to the Workbench area only;
the three graph canvas hosting contexts; and the Navigator/Shell omnibar seam.

**Related**:

- [SHELL.md](SHELL.md) — Shell domain spec (authority boundaries, what Shell owns)
- [../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — companion closure lane for command-surface provenance, semantic modeling, and AT validation
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — Navigator domain spec
- [../graph/GRAPH.md](../graph/GRAPH.md) — Graph domain spec; the canvas is its primary rendered surface
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — Workbench domain spec
- [../../technical_architecture/unified_view_model.md](../../technical_architecture/unified_view_model.md) — §3 host model, §13 not-everything-is-a-tile
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical term definitions

**Implementation anchors** (current code):

- `shell/desktop/ui/toolbar/toolbar_ui.rs` — current `TopBottomPanel::top("graph_bar")` and `TopBottomPanel::top("fullscreen_origin_strip")`
- `shell/desktop/ui/workbench_host.rs` — current `SidePanel` workbench host rendering
- `shell/desktop/ui/gui_frame.rs` — frame phase facade; owns `Tree<TileKind>` and panel sequence
- `shell/desktop/ui/gui_orchestration.rs` — orchestration layer above gui_frame

---

## 1. Problem: Ad-Hoc Panel Calls With No Composition Model

The current rendering path calls egui panels in sequence across multiple files
with no formal composition structure:

```
toolbar_ui.rs:
  TopBottomPanel::top("fullscreen_origin_strip")   // conditional
  TopBottomPanel::top("graph_bar")                 // toolbar + omnibar

workbench_host.rs:
  SidePanel::right(...)                            // workbench host

gui_frame.rs (remaining central area):
  egui_tiles Tree<TileKind>                        // tile tree — owns everything remaining
```

Two problems follow from this:

1. **`egui_tiles` claims the whole remaining area**, which means the graph
   canvas must always be a `TileKind::Graph` guest inside the tile tree. The
   graph does not exist independently of the Workbench.

2. **`graph_bar` mixes controls from three distinct authorities** — Shell
   (commands, settings, omnibar input), Navigator (graph view tabs, lens menu,
   physics menu, +Node, +Edge), and Viewer (back/forward/reload for the focused
   pane). This is the exact scope conflation documented in
   `chrome_scope_split_plan.md §1`.

This spec defines the replacement: a formal `ShellLayout` using egui's named
panel system where each slot has a declared authority, and `egui_tiles` governs
only the Workbench area slot.

---

## 2. The Shell Layout Skeleton

Shell owns the top-level window composition. The layout is a named-slot skeleton
using egui's `TopBottomPanel` and `SidePanel` hierarchy, with the remaining
`CentralPanel` area allocated to the primary graph canvas.

```
┌──────────────────────────────────────────────────────────┐
│  ShellSlot::CommandBar  (TopBottomPanel::top)            │  Shell
│  omnibar input, command palette, Shell status            │
├───────────────────────┬──────────────────────────────────┤
│ ShellSlot::            │  ShellSlot::GraphPrimary         │
│ NavigatorLeft          │  (CentralPanel)                  │
│ (SidePanel::left)      │  graph canvas — direct mount,    │  Navigator /
│                        │  no egui_tiles wrapper           │  Graph
│ Navigator hosts        │                                  │
│ (sidebar form factor)  │  when WorkbenchActive or Pinned: │
│                        │  graph canvas shrinks or hides;  │
├───────────────────────┤  WorkbenchArea takes precedence  │
│ ShellSlot::            │                                  │  Workbench
│ WorkbenchArea          ├──────────────────────────────────┤
│ (SidePanel::right      │  ShellSlot::NavigatorBottom      │
│  or overlay region)    │  (TopBottomPanel::bottom)        │
│                        │  optional toolbar-form Navigator │  Navigator
│ egui_tiles lives here  │  host                            │
└───────────────────────┴──────────────────────────────────┘
│  ShellSlot::StatusBar  (TopBottomPanel::bottom)          │  Shell
│  ambient system status, background task indicators       │
└──────────────────────────────────────────────────────────┘
```

### 2.1 Slot definitions

| Slot | egui primitive | Authority | Contents |
|------|---------------|-----------|----------|
| `CommandBar` | `TopBottomPanel::top("shell_command_bar")` | Shell | Omnibar (input + Navigator context display), command palette trigger, settings access, sync status |
| `NavigatorLeft` | `SidePanel::left("navigator_host_left")` | Navigator (content) / Shell (lifecycle, resize handle) | Navigator sidebar host when anchor = Left |
| `NavigatorRight` | `SidePanel::right("navigator_host_right")` | Navigator (content) / Shell (lifecycle, resize handle) | Navigator sidebar host when anchor = Right — only active when `WorkbenchArea` is not occupying right edge |
| `WorkbenchArea` | `SidePanel::right("workbench_area")` or overlay | Workbench | `egui_tiles::Tree<TileKind>` — panes, frames, splits. Active only in `WorkbenchActive` / `WorkbenchPinned` states |
| `GraphPrimary` | `CentralPanel` (remaining area) | Graph (content) / Shell (mount) | Primary graph canvas — direct egui render, no tile wrapper. Always present unless fullscreen pane overrides |
| `NavigatorBottom` | `TopBottomPanel::bottom("navigator_host_bottom")` | Navigator (content) / Shell (lifecycle) | Optional toolbar-form Navigator host when anchor = Bottom |
| `StatusBar` | `TopBottomPanel::bottom("shell_status_bar")` | Shell | Ambient system status, process indicators, background task count |

### 2.2 Panel order

egui resolves panels by declaration order. The required order is:

1. `StatusBar` (bottom — must be declared before CentralPanel)
2. `NavigatorBottom` (bottom — above status bar)
3. `CommandBar` (top)
4. `NavigatorLeft` (left — only when enabled)
5. `WorkbenchArea` (right — only when `WorkbenchActive` or `WorkbenchPinned`)
6. `NavigatorRight` (right — only when enabled and WorkbenchArea not on right)
7. `GraphPrimary` (CentralPanel — always last)

This is formalised as `ShellLayoutPass::render(&self, ctx, slots)` — a single
function that owns the panel declaration sequence and routes each slot to the
appropriate authority's render function.

### 2.3 Overlay behavior contract (`GraphOverlayActive`)

`GraphOverlayActive` is the only state where `WorkbenchArea` is not a docked
named panel. In this state, Shell renders `GraphPrimary` first, then renders
the Workbench as a floating overlay above it.

The overlay contract is:

1. **Z-order**: `GraphPrimary` renders first; the Workbench overlay renders
   after it and appears visually above it.
2. **Input routing**: pointer and keyboard input inside the overlay rect target
   the overlay only. Input outside the overlay rect targets `GraphPrimary`.
3. **Graph continuity**: `GraphPrimary` remains live and continues rendering
   behind the overlay. The overlay may dim the graph beneath it, but does not
   freeze graph state.
4. **Dismissal**: overlay dismissal policy is Shell-owned. Clicking outside the
   overlay does not implicitly dismiss it unless Shell explicitly enables that
   policy for a given command surface state.
5. **Geometry authority**: Shell computes the overlay rect and applies its
   z-order and hit-testing policy. Workbench renders only within the rect Shell
   assigns.

One concrete representation is:

```rust
pub struct WorkbenchOverlayLayout {
    pub rect: egui::Rect,
    pub modal: bool,
    pub blocks_graph_input: bool,
    pub dim_graph_primary: bool,
}
```

The exact fields may evolve, but the authority rule does not: Shell owns
overlay geometry, z-order, and hit-testing behavior.

### 2.4 The WorkbenchArea slot and `WorkbenchLayerState`

`WorkbenchArea` is conditional. It is shown only when `WorkbenchLayerState` is
`WorkbenchActive` or `WorkbenchPinned`. In `GraphOnly` and `GraphOverlayActive`
states the slot is not declared, so `GraphPrimary` expands to fill the full
window width.

When `WorkbenchArea` is declared, `GraphPrimary` shrinks to the remaining
central area. The graph canvas renders in that remaining area — it is still
present, just smaller.

This replaces the current model where the tile tree claims the central area
unconditionally and the graph canvas must be a `TileKind::Graph` tile to appear
at all.

---

## 3. `egui_tiles` Scope Restriction

`egui_tiles::Tree<TileKind>` is rendered **inside `WorkbenchArea` only**.

```rust
// CORRECT: tile tree rendered inside the WorkbenchArea slot
SidePanel::right("workbench_area")
    .show(ctx, |ui| {
        tree.ui(&mut tile_behavior, ui);
    });

// INCORRECT (current): tile tree rendered in CentralPanel, claiming all remaining space
CentralPanel::default()
    .show(ctx, |ui| {
        tree.ui(&mut tile_behavior, ui);  // ← this is what we are replacing
    });
```

**Consequences:**

- A `TileKind::Graph(GraphViewId)` tile is now *one of three valid hosting
  modes* for a graph canvas, not the mandatory wrapper. A graph canvas can exist
  without any tile tree entry.
- The graph canvas renders in `GraphPrimary` when it is the primary Shell
  surface. When the user explicitly brings a graph view into the Workbench for
  a split, `TileKind::Graph(GraphViewId)` hosts it there instead.
- A zero-Workbench product state (shell + graph only, no open panes) is valid
  and complete. `WorkbenchArea` is simply not rendered.

---

## 4. Graph Canvas Hosting Contexts

A graph canvas (`GraphViewId` + layout state + camera state) is a render unit
that is agnostic to its hosting context. The context determines lifecycle,
surrounding chrome, and rect allocation — not what is rendered.

```rust
pub enum GraphCanvasHostCtx {
    /// Mounted directly by Shell in GraphPrimary (CentralPanel).
    /// No surrounding chrome. Lifecycle owned by Shell.
    ShellPrimary,

    /// Hosted as TileKind::Graph(GraphViewId) inside the Workbench tile tree.
    /// Surrounded by a tab strip. Lifecycle owned by Workbench.
    WorkbenchTile { tile_id: TileId },

    /// Hosted inside a Navigator specialty host for a scoped graphlet view
    /// (ego, corridor, component, atlas, etc.).
    /// Surrounded by Navigator chrome. Lifecycle owned by Navigator.
    NavigatorSpecialty { graphlet_kind: GraphletKind },
}
```

### 4.1 Context comparison

| Property | `ShellPrimary` | `WorkbenchTile` | `NavigatorSpecialty` |
|---|---|---|---|
| Rect source | `CentralPanel` remaining area | tile rect from `egui_tiles` | Navigator host inner rect |
| Chrome | none | tab strip | Navigator chrome (graphlet label, scope controls) |
| Lifecycle owner | Shell | Workbench | Navigator |
| `GraphViewId` persistence | persisted as primary view | persisted with tile tree | session-scoped or pinned graphlet |
| Edge family mask | full (user's active lens) | full or lens-filtered | graphlet-specific (may be more restricted) |
| Camera state | independent per `GraphViewId` | independent per `GraphViewId` | independent per `GraphViewId` |
| Layout algorithm | user-chosen (`GraphLayoutMode`) | user-chosen | Navigator-chosen for graphlet shape (may differ from primary) |
| Mutation authority | Graph domain — full | Graph domain — full | Graph domain — full (Navigator emits intents, Graph executes) |

**The invariant**: all three contexts render the same graph truth via the same
`GraphCanvas` render unit. The hosting context changes presentation and
lifecycle; it does not change what graph truth is shown or who owns mutation.

### 4.2 Migration from the current model

Currently the primary graph canvas is always `TileKind::Graph(GraphViewId)`
inside the tile tree, rendered in the central area. The migration:

1. Add `ShellPrimary` as a valid hosting context; the canvas can render in
   `GraphPrimary` (CentralPanel) without a tile tree entry.
2. When `WorkbenchLayerState` is `GraphOnly` or `GraphOverlayActive`, the
   canvas renders in `ShellPrimary` context — no tile tree involved.
3. When the user explicitly opens a graph view pane in the Workbench, the
   canvas renders as `WorkbenchTile` — this is the existing behavior, unchanged.
4. Navigator specialty graph views use `NavigatorSpecialty` — new context,
   added when Navigator graphlet views are implemented.

The migration is additive: `TileKind::Graph` continues to work as-is.
`ShellPrimary` is a new path that replaces the forced-tile path for the default
zero-Workbench state.

---

## 5. The CommandBar Slot and Omnibar Seam

The `CommandBar` slot is Shell-owned. The omnibar within it is a composite
widget: Shell owns the widget frame and input handling; Navigator contributes
a read-only context projection to a designated display region.

### 5.1 `NavigatorContextProjection`

Navigator produces one `NavigatorContextProjection` per frame. Shell reads it
at render time.

```rust
/// Produced by Navigator each frame. Read by Shell for omnibar display mode.
/// Navigator owns the content; Shell owns the rendering context and widget.
pub struct NavigatorContextProjection {
    /// Stable breadcrumb path: active scope root + containment ancestry if present.
    /// Uses containment ancestry, NOT shortest path (per unified_view_model §15).
    /// None if no meaningful graph context is active.
    pub breadcrumb: Option<BreadcrumbPath>,

    /// Active graphlet label if a named or pinned graphlet is active.
    pub graphlet_label: Option<String>,

    /// Compact scope badge text for input mode (one word or short phrase).
    /// Shown even when omnibar is in input mode.
    pub scope_badge: Option<String>,
}

pub struct BreadcrumbPath {
    /// Ordered tokens: [scope_root?, containment_ancestors*, active_node_address]
    pub tokens: Vec<BreadcrumbToken>,
}

pub struct BreadcrumbToken {
    pub label: String,
    pub node_key: Option<NodeKey>,  // None for scope roots without a graph node
}
```

### 5.2 Omnibar rendering modes

| Mode | Trigger | Shell renders | Navigator contributes |
|---|---|---|---|
| **Display** | omnibar not focused | full widget background + right-side controls | `breadcrumb` rendered left-aligned in a read-only region; `graphlet_label` as a badge |
| **Input** | user clicks omnibar or presses shortcut | full widget background + text input field + completions | only `scope_badge` (small, non-interactive label beside input field) |
| **Fullscreen** | `fullscreen_origin_strip` active | condensed strip showing current URL | not shown |

The transition between display mode and input mode is Shell-owned. When the
omnibar gains focus (click, keyboard shortcut, or command palette invocation),
Shell switches to input mode and the `breadcrumb` region collapses to just the
`scope_badge`.

Navigator does not own the input field, completion list, or dispatch logic.
Shell does not own the breadcrumb content. The seam is the struct.

### 5.3 CommandBar host-thread contract

`CommandBar` is a **Shell host-thread surface** even when it depends on
background data.

The threading contract is:

1. input state, mode switches, and completion-list presentation are Shell-owned
   frame-loop state
2. Navigator contributes only frame-readable context projection
   (`NavigatorContextProjection`)
3. any background provider fetch, index lookup, or remote suggestion request
   must run under Shell/Register supervision (`ControlPanel` or equivalent),
   not via ad hoc toolbar-owned detached threads
4. background results are ingested by Shell at frame boundaries through an
   explicit mailbox/receiver owned by the current omnibar session

Current-state note: the landed baseline already follows this split through the
Shell-owned omnibar session carrier plus supervised `HostRequestMailbox<T>` and
typed frame-inbox drainage for longer-lived relays. The remaining work is to
prove and document that boundary consistently, not to reopen it as speculative design.

This keeps the omnibar aligned with the broader Shell-as-host rule:
Shell owns the widget, focus, and visible state; background runtime work only
feeds it through explicit host-owned seams.

### 5.4 CommandBar target resolution

The `CommandBar` may submit actions whose target depends on the currently
focused surface. That target resolution is not inferred ad hoc from whichever
subsystem most recently rendered; it is a first-class per-frame input to Shell.

One concrete shape is:

```rust
pub struct CommandBarFocusTarget {
    pub focused_surface: FocusedSurface,
   pub focused_node: Option<NodeKey>,
}

pub enum FocusedSurface {
    GraphPrimary(GraphViewId),
    WorkbenchTile { tile_id: TileId, pane_id: PaneId },
    NavigatorHost { host_id: NavigatorHostId },
}
```

The required precedence rule is:

1. keyboard focus owner wins
2. otherwise, last pointer-interacted surface wins
3. otherwise, no focused command target is exposed in `CommandBar`

This rule is Shell-owned and evaluated once per frame before rendering the
`CommandBar`. The bar itself no longer hosts viewer chrome directly; this
target carrier exists so omnibar submission, command entry, and other
Shell-owned routing can resolve the correct graph/workbench target without
depending on render order.
rather than re-deriving focus inside toolbar code.

Current-state note: `CommandBarFocusTarget` is now the landed baseline carrier.
The remaining closure work is its provenance/evidence model: diagnostics
receipts, UxTree trace projection, and focus-return / AT validation are tracked
in `../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`.

### 5.5 Controls previously in `graph_bar` — prototype redistribution

`toolbar_ui.rs` still renders these in `shell_command_bar` today, but the
prototype target architecture is stricter than "better labeled mixed chrome."
The `CommandBar` should converge to a Shell-owned bar containing only
Shell-owned controls plus the explicit Navigator read-only omnibar-context
seam.

| Current control | Correct authority | Prototype target |
|---|---|---|
| Omnibar input field | Shell | Keep in `CommandBar` input region |
| Omnibar breadcrumb / context display | Navigator (read-only seam) | Keep only as the explicit read-only omnibar-context seam |
| Graph view tabs | Navigator | Relocate to a Navigator host slot (`NavigatorLeft`, `NavigatorRight`, or `NavigatorBottom`) |
| Wry / Servo compat toggle | Viewer | Remove from `CommandBar`; relocate only if a viewer-local debug/control surface still needs it |
| Back / Forward buttons | Viewer (per-pane) | Relocate to pane-local viewer chrome, not `CommandBar` |
| Reload / Stop / Zoom controls | Viewer (per-pane) | Relocate to pane-local viewer chrome, not `CommandBar` |
| Undo / Redo | Graph | Relocate to graph-local chrome or command surfaces |
| +Node / +Edge / +Tag | Graph | Relocate to graph-local chrome or command surfaces |
| Lens menu | Graph / Navigator-adjacent | Relocate to graph-view or Navigator-owned chrome |
| Physics menu | Shell policy or graph-facing config, depending on final contract | Remove from `CommandBar`; if retained, expose through a Shell control surface rather than primary command chrome |
| Fit | Graph | Relocate to graph-local chrome |
| Settings button | Shell | Keep in `CommandBar` right region |
| Command palette button | Shell | Keep in `CommandBar` right region |
| Sync status dot | Shell (ambient system status) | Keep in `CommandBar` right region or `StatusBar` |

The redistribution therefore does require some controls to move or disappear,
not merely to be re-labeled inside the same mixed bar.

---

## 6. The `ShellLayoutPass` Type

The composition model is formalized as a single render coordinator:

```rust
/// Owns the egui panel declaration sequence for one frame.
/// Prevents panel order drift and makes slot authority explicit.
pub struct ShellLayoutPass<'a> {
    ctx: &'a egui::Context,
    layer_state: WorkbenchLayerState,
    navigator_ctx: &'a NavigatorContextProjection,
    navigator_hosts: &'a [NavigatorHostLayout],
}

impl<'a> ShellLayoutPass<'a> {
    /// Renders all Shell panels in the correct declaration order.
    /// Calls into Shell, Navigator, and Workbench render functions
    /// for each slot. Returns slot rects for downstream use.
    pub fn render(self, slots: &mut ShellSlotRenderArgs) -> ShellSlotRects;
}

pub struct ShellSlotRects {
    pub command_bar: egui::Rect,
    pub graph_primary: egui::Rect,
    pub workbench_area: Option<egui::Rect>,
    pub navigator_left: Option<egui::Rect>,
    pub navigator_right: Option<egui::Rect>,
    pub navigator_bottom: Option<egui::Rect>,
    pub status_bar: Option<egui::Rect>,
}
```

`ShellLayoutPass` replaces the current scattered panel calls in `toolbar_ui.rs`
and `workbench_host.rs`. It is called once per frame from `gui_frame.rs` (or
its successor) after pre-frame ingest completes.

---

## 7. Interaction With `WorkbenchLayerState`

`WorkbenchLayerState` continues to govern when `WorkbenchArea` is shown. The
mapping to `ShellLayoutPass` behavior:

| `WorkbenchLayerState` | `WorkbenchArea` slot | `GraphPrimary` | Navigator hosts |
|---|---|---|---|
| `GraphOnly` | not rendered | full remaining area | rendered if enabled |
| `GraphOverlayActive` | rendered as floating overlay | full area behind overlay | rendered if enabled |
| `WorkbenchActive` | rendered as right sidebar | shrunk to remaining area | rendered if enabled |
| `WorkbenchPinned` | rendered as pinned right sidebar | shrunk to remaining area | rendered if enabled |

Input routing expectations:

| `WorkbenchLayerState` | Pointer input inside Workbench rect | Pointer input outside Workbench rect | Keyboard focus precedence |
|---|---|---|---|
| `GraphOnly` | n/a | `GraphPrimary` | graph or active Shell widget |
| `GraphOverlayActive` | Workbench overlay | `GraphPrimary` | keyboard focus owner wins |
| `WorkbenchActive` | WorkbenchArea | `GraphPrimary` / other Shell slots | keyboard focus owner wins |
| `WorkbenchPinned` | WorkbenchArea | `GraphPrimary` / other Shell slots | keyboard focus owner wins |

`ChromeExposurePolicy` continues to derive from `WorkbenchLayerState` and
governs which Navigator hosts are shown in each state (per
`2026-03-13_chrome_scope_split_plan.md §8`).

---

## 8. Implementation Phases

### Phase 1 — `ShellLayoutPass` formalization (docs + code skeleton)

- Define `ShellSlot` enum and `ShellLayoutPass` struct
- Move panel declaration sequence into `ShellLayoutPass::render()`
- Rename `"graph_bar"` panel ID to `"shell_command_bar"` with a settings key
  migration (no behavioral change)
- `egui_tiles` continues to render in CentralPanel for now (deferred to Phase 2)

**Acceptance**: panel order is owned by one function; no behavioral change.

### Phase 2 — `egui_tiles` scoped to `WorkbenchArea`

- Add `WorkbenchArea` as a named `SidePanel::right` slot
- Move `Tree<TileKind>` render call inside `WorkbenchArea`
- `GraphPrimary` (CentralPanel) renders the graph canvas directly via
  `GraphCanvas::render(GraphViewId, ui, GraphCanvasHostCtx::ShellPrimary)`
  when `WorkbenchLayerState` is `GraphOnly` or `GraphOverlayActive`
- `TileKind::Graph` inside `WorkbenchArea` continues to work as-is

**Acceptance**: zero-Workbench state renders the graph canvas without a tile
tree entry; `WorkbenchActive` state renders both the canvas in `GraphPrimary`
and the tile tree in `WorkbenchArea`; no existing tile behavior changes.

### Phase 3 — `NavigatorContextProjection` and omnibar seam

- Define `NavigatorContextProjection` struct
- Navigator computes it each frame from current graphlet / scope / selection
- Shell reads it in omnibar display mode
- Graph view tabs move from `CommandBar` interior to Navigator projection or
  to a dedicated Navigator toolbar host

**Acceptance**: omnibar display mode shows Navigator breadcrumb; input mode
shows only scope badge; graph view tabs are not rendered by Shell-owned code.

### Phase 4 — `GraphCanvasHostCtx::NavigatorSpecialty`

**Status**: Implemented 2026-04-07

- Added `NavigatorSpecialty` to `GraphCanvasHostCtx`
- Navigator hosts now carry specialty graphlet state via
   `navigator_specialty_views` and render a scoped graph canvas inside the host
   using a `GraphletKind`-parameterized transient `GraphViewId`
- Specialty views now derive from the current focused selection, not an ad hoc
   local host cache
- Navigator now chooses specialty-view policy at activation time:
   corridor / bridge graphlets use tree layout, workbench-correspondence views
   use grid layout, and other current specialty kinds default to the standard
   force-directed graph canvas policy
- Specialty edge-projection override is applied when the graphlet kind implies
   a constrained family projection (`Session`, `WorkbenchCorrespondence`)

**Implementation receipt (2026-04-07):**

- `shell/desktop/ui/shell_layout_pass.rs` defines
   `GraphCanvasHostCtx::NavigatorSpecialty { graphlet_kind }`
- `app/intent_phases.rs` derives and maintains transient specialty graph views
   under `workbench_session.navigator_specialty_views`
- `shell/desktop/ui/workbench_host.rs` exposes ego / corridor / component
   specialty controls, clears the active specialty view, and mounts the graphlet
   canvas inside the Navigator host
- `shell/desktop/workbench/tile_render_pass.rs` renders the scoped specialty
   graph canvas and routes its mutation intents through Graph/Workbench paths

**Acceptance**: ego graphlet / corridor view renders in a Navigator host with
Navigator chrome; mutation intents from within it route to Graph domain normally.

---

## 9. Acceptance Criteria for Phase 1 + 2

1. Panel declaration sequence is owned by one function (`ShellLayoutPass::render`).
2. `egui_tiles::Tree<TileKind>` is rendered inside `WorkbenchArea` only —
   not in `CentralPanel`.
3. With no open panes (`WorkbenchLayerState::GraphOnly`), the graph canvas
   renders directly in `CentralPanel` with no `TileKind::Graph` entry.
4. Opening a pane transitions to `WorkbenchActive`; `WorkbenchArea` appears;
   `GraphPrimary` shrinks to its remaining area.
5. `TileKind::Graph(GraphViewId)` inside the tile tree continues to work
   identically to its current behavior.
6. Existing Navigator host resize, show/hide, and pinning behavior is unchanged.
7. All current tests pass. Snapshot assertions for `WorkbenchLayerState` and
   `ChromeExposurePolicy` pass without modification.

---

## 10. What This Does Not Change

- `WorkbenchLayerState` variants and their semantics
- `ChromeExposurePolicy` derivation from `WorkbenchLayerState`
- `TileKind`, `TileId`, `egui_tiles` tree structure, or tile behavior contracts
- `GraphViewId` identity model
- `GraphMutation / WorkbenchIntent / RuntimeEffect` authority split
- Navigator host scope settings (`Both` / `GraphOnly` / `WorkbenchOnly` / `Auto`)
- Any existing test scenarios or snapshot fixtures
