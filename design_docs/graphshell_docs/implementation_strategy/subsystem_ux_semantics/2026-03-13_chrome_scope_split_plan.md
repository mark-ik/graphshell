# Chrome Scope Projection and Host Exposure Plan

**Date**: 2026-03-13
**Last updated**: 2026-03-22
**Status**: Design — Pre-Implementation
**Purpose**: Define the host-based chrome architecture that replaces the current
monolithic toolbar, aligning control surfaces with the semantic authority
boundaries already present in the codebase. The desktop default is a
graph-scoped toolbar Navigator host plus a workbench-scoped sidebar Navigator
host, but host count, edge, and form factor are layout policy rather than fixed
surface types.

**Alignment note (2026-03-23)**: `../navigator/NAVIGATOR.md §12` is the
canonical source for Navigator host count, scope, anchor edge, and form factor.
This document remains the execution-plan source for how graph/workbench/pane
controls project into those hosts, and for the derived `WorkbenchLayerState`,
`ChromeExposurePolicy`, and `WorkbenchChromeProjection` contracts.

**Boundary note (2026-03-27)**: the newer five-domain split in `SHELL.md`,
`NAVIGATOR.md`, `WORKBENCH.md`, and `VIEWER.md` sharpens the ownership model
used here:

- Shell owns top-level host composition and app/control routing,
- Navigator owns projection semantics rendered inside Navigator hosts,
- Workbench owns host layout chrome, visibility policy, and arrangement/session
  inputs,
- Viewer owns pane-local content rendering and viewer controls.

`WorkbenchChromeProjection` therefore describes a derived host-chrome model for
the workbench-scoped host, not an alternate owner of Navigator semantics.

**Related**:
- `2026-03-01_ux_execution_control_plane.md`
- `2026-02-28_ux_contract_register.md`
- `2026-03-04_model_boundary_control_matrix.md`
- `2026-03-08_unified_ux_semantics_architecture_plan.md`
- `../../../archive_docs/checkpoint_2026-04-06/graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-03-23_navigator_host_runtime_naming_plan.md` — archived runtime naming migration receipt
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `../graph/multi_view_pane_spec.md`
- `../graph/2026-03-14_graph_relation_families.md` — relation family vocabulary and navigator projection contract
- `../graph/semantic_tagging_and_knowledge_spec.md`
- `../graph/node_badge_and_tagging_spec.md`
- `../graph/layout_behaviors_and_physics_spec.md`
- `../aspect_render/2026-03-12_compositor_expansion_plan.md`

---

## 1. Problem

The current toolbar (`shell/desktop/ui/toolbar/toolbar_ui.rs`) renders as one
flat bar containing controls from three distinct semantic scopes:

| Current location | Actual authority |
|-----------------|-----------------|
| Back / Forward / Reload | Pane/viewer — per-focused-node |
| Omnibar / location field | Graph — cross-node navigation plus stable graph-position projection |
| Frame pin / recall (P+/P-/W+/W-) | Workbench — tile-tree structure |
| View toggle (Graph ↔ Detail) | Workbench — tile-tree visibility |
| Settings menu | App — global preferences |
| Clear data | App — destructive graph mutation |
| Sync status dot | Graph/Verse — ambient social state |
| Command palette button | App — global overlay |

This conflation means:
- Controls appear and disappear based on `has_node_panes` in ad hoc ways
- The bar carries no durable semantic identity
- Graph-scope controls (sync, lens, physics, tag filters) have no stable home
  and are scattered across panels and settings menus
- Pane-local controls (back/forward/reload) are elevated to global chrome,
  implying they act on everything when they act only on the focused pane

The rule is: **only show controls at the scope that owns the state.**

---

## 2. What Graph and Workbench Mean

Before specifying the chrome surfaces, the semantic distinction they encode:

**Graph layer**: The persistent authority. The graph is where content is
persisted, related, classified, and mediated. It is also the social and
metabrowsing layer — Verse sync, peer presence, trust, and knowledge
organization all live here. The graph is always present. It is not a view
you open; it is the substrate the browser runs on.

**Workbench layer**: The structural presentation layer. The workbench is
where active content is staged, compared, and arranged for reading and
manipulation. It is driven by tile-tree geometry — pane splits, tab groups,
and frames — which are themselves projections of graph relationships into
spatial layout. The workbench is transient: you enter it by opening nodes
and leave it by closing all tiles back to the graph.

**Pane/viewer layer**: The document and content layer. What a single node
shows you. Navigation history, reader/viewer controls, capture and clip
actions. These belong with the focused pane's **tile-local chrome** when the
pane is presented as a tiled workbench citizen, not in graph chrome and not in
workbench-scoped host chrome.

**Transient overlay surfaces**: Some app-owned pages may open over the graph
without immediately becoming tiled workbench structure. They remain Graphshell
surfaces, but they do not enter workbench hosting until the user explicitly
promotes or tiles them.

This is the architectural basis for the separation. The chrome surfaces expose
it visually.

### 2.1 Cross-Spec Tie-Ins

This plan does not replace the adjacent canonical specs; it defines where their
semantics surface in the desktop chrome.

- `../workbench/workbench_frame_tile_interaction_spec.md` remains the authority
  for frame/tile/pane semantics. This plan only defines how frame, group, and
  pane structure are projected into the default workbench-scoped Navigator host.
- `../canvas/multi_view_pane_spec.md` remains the authority for
  `GraphViewId`, slot lifecycle, and routed graph panes. This plan makes the
  default graph-scoped Navigator host the always-visible place where the active
  graph target and graph view slots are named.
- `../aspect_control/settings_and_control_surfaces_spec.md` remains the
  authority for tool-page routing, apply semantics, and return paths. This plan
  adds the presentation distinction between transient graph overlays and
  explicitly tiled workbench-hosted panes.
- `../graph/node_badge_and_tagging_spec.md` and
  `../aspect_render/2026-03-12_compositor_expansion_plan.md` remain the
  authority for canvas badges and tile affordances. This plan determines which
  of those signals surface as graph-scoped host chips versus
  workbench-scoped host row/header badges.

### 2.2 Shared Verb Mapping

The shared verbs from the unified view model apply here as a chrome-filtering
rule:

- `select` and `scope` belong naturally in graph-scoped chrome
- `activate` and `arrange` often route through workbench-scoped chrome
- `reveal` is a local orientation effect and may appear in either host
- `mutate` only belongs in chrome when the control explicitly targets durable
  graph truth

Chrome should therefore expose the verb at the scope that owns it rather than
promoting pane-local or workbench-local controls into graph-global chrome.

---

## 3. Target Architecture: Default Desktop Host Layout

```text
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ Graph-scoped Navigator host (default: top toolbar, always visible)                      │
│ [View ▾] [Undo] [Redo] [+node] [+edge] [+tag] [cmd] ··· [Omnibar] ··· [slots] [⟳] [⋯] │
└──────────────────────────────────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────┬───────────────────────────┐
│ Graph surface / tiled panes                                 │ Workbench-scoped Navigator │
│                                                              │ host (default: sidebar)   │
│                                                              │ [←] [→] [R] [clip] [view] │
│                                                              │ [Frame A ▾] [Group ▾]     │
│                                                              │ > Pane tree / tab tree    │
│                                                              │   • active pane           │
│                                                              │   • sibling pane          │
│                                                              │ [split] [adjacent] [life] │
└──────────────────────────────────────────────────────────────┴───────────────────────────┘
```

In the default desktop preset, the graph-scoped Navigator host remains the
only top chrome. It names the current graph target, anchors graph interaction
controls, keeps the Omnibar centered, and carries graph-scope state chips and
overflow. This layout stays stable regardless of what workbench structure is
open.

Workbench chrome renders primarily as a right-side workbench-scoped Navigator
host on desktop. Its header carries pane-local controls for the focused pane,
its scope row summarizes the active frame and tile group, and its scroll body
renders a tree-style projection of the current pane structure. A compact
horizontal host form may exist later as a narrow-width fallback, but it is not
the primary desktop expression.

**Default side assumption:** the initial desktop implementation should place the
workbench-scoped sidebar host on the right so the left side remains available
for other graph/navigation host configurations if needed.

---

## 4. Graph-Scoped Navigator Host

Always visible. The persistent semantic operating layer. Never changes layout
based on what tiles are open.

### 4.1 Control Layout (left → right)

**Left cluster — graph interaction:**

| Control | Intent | Notes |
| --- | --- | --- |
| Active graph-view chip | Focused `GraphViewId` target | Names the graph scope the bar is acting on; collapses when only one graph view is live |
| Undo / Redo | `Undo`, `Redo` | Graph-scope history; remains graph-scoped even when a pane is focused |
| New node | `CreateNodeNearCenterAndOpen` | Core graph mutation |
| New edge | `CreateUserGroupedEdgeFromPrimarySelection` | Requires selection |
| New tag | `TagNode` flow | Opens tag input for selected node(s) |
| Inspect / command | `ToggleCommandPalette` | Opens command palette scoped to current graph context |

**Center — omnibar:**

| Control | Intent | Notes |
| --- | --- | --- |
| Omnibar | Navigation + search | Cross-node, cross-graph. Also carries stable graph-position context: active scope token first, then canonical containment ancestry when available |

**Right cluster — ambient state + overflow:**

| Control | Intent / Authority | Notes |
| --- | --- | --- |
| Graph view slot strip | `RouteGraphViewToWorkbench`, `MoveGraphViewSlot` | Compact slot strip; active view should be legible here and/or in the left target chip |
| Fit to screen | `RequestFitToScreen` | Single button, always relevant |
| Lens / view-dimension chip | `SetViewLensId`, `SetViewDimension` | Collapsed chip; expands on click. Replaces in-canvas lens overlay |
| Physics chip | `TogglePhysics`, `SetPhysicsProfile`, `ReheatPhysics` | Collapsed chip; expands to profile picker + on/off. Replaces in-canvas physics panel |
| Active tag filter chips | Semantic filter state | Zero or more dismissible chips; visible when a filter is active |
| Semantic depth badge | `ToggleSemanticDepthView` | On/off badge chip |
| Sync / Verse badge | Verse peer presence | Dot expands on click to peer list, `SyncNow`, trust controls |
| Overflow (⋯) | Settings launcher, diagnostics export, clear data, help | Menu; settings acts as a launcher/router into page-backed settings surfaces rather than a dump of inline toggles (see §4.2) |

**Omnibar path rule**

The graph-position context shown in the Omnibar should not be based on arbitrary
graph shortest path. Shortest path is valuable for explicit graph explanation
commands, but chrome breadcrumbs must remain stable as unrelated graph edges
change.

The canonical order is:

1. active graph/workbench scope token
2. canonical containment ancestry when it exists
3. compact fallback to scope root + node address when no containment ancestry exists

Any "show path" or "open path" affordance should be a separate explicit command,
not a hidden breadcrumbing algorithm.

### 4.2 Settings and Config Page Routing

Settings and graph-relevant configuration pages remain page-backed surfaces,
not modal dialogs, but they now have two presentation modes:

- If the user is in **graph-only mode**: the config page opens as a transient
  overlay surface above the graph canvas. The graph remains visible behind it,
  and Workbench chrome does not appear just because an overlay is open.
- The transient overlay may be **promoted / tiled into the workbench** via an
  explicit action such as `Tile This Page`, at which point it becomes a normal
  workbench-hosted pane with a sidebar row and tab/tree presence.
- If the user is already in **workbench mode**: the config page may open
  directly into the tile tree using the current open mode (tab in the focused
  group, or a new split).

This keeps settings page-backed and composable while preserving a lighter
graph-only flow for quick configuration and inspection.

**Default launcher policy**

- The default graph-scoped Navigator host `Settings` entry should open the most natural presentation
  for the current scope: overlay when the graph is the active context, hosted
  settings pane when the workbench is the active context.
- The launcher may offer direct entry points to specific settings pages
  (Persistence, Appearance and Viewer, Input and Commands, Physics, Sync,
  Advanced), but it should not itself host the editable controls.
- Related control surfaces such as History Manager, Diagnostics, and Help may
  be reachable nearby, but they are not settings categories.

---

## 5. Workbench-Scoped Navigator Host

Visible when the workbench layer is active. The structural presentation layer.
This host is a **live projection of the tile tree onto a navigator surface**
— it should feel like the workbench rendered as a side rail, not like another
toolbar with more buttons.

### 5.1 Host Header — Pane-Local Controls

The host header acts on the focused pane's **structure and status**, not as a
second viewer toolbar.

Allowed header content:

| Surface | Authority | Notes |
|---------|-----------|-------|
| Presentation / residency badge | Workbench | Floating / Docked / Tiled, warm/cold, active badges |
| Backend / degraded badge | Viewer/runtime status | Read-only status; may open detail surface or focus handoff |
| Clip / capture entry point | `OpenClip`, `CreateNoteForNode` | Optional nearby entry, but not a full viewer toolbar |

Disallowed here as primary command ownership:

- Back / Forward / Reload / StopLoad
- Find in page
- Content zoom controls
- Compat / backend toggle

Those belong to tile-local viewer chrome for tiled panes only. The graph-scoped
host `Undo` / `Redo` do not morph in this direction. Graph history stays
graph-scoped; pane navigation stays tile-scoped.

### 5.2 Scope Chips — Frame and Tile Group

The top scope row summarizes the active structural context without flattening
all of it into a strip:

- **Frame chip**: active frame name, dropdown to switch or create frames
- **Frame actions**: save current frame snapshot, restore a named frame, and
  prune empty named frames live with the frame dropdown/chip rather than inside
  settings pages
- **Tile group chip**: active tab/tile-group summary, scrollable dropdown to
  jump between groups in the current frame context
- **Topology token**: compact structural summary such as
  `Frame A · Group 2 · Split H`; this is a workbench-structure token, not a
  graph-position breadcrumb. Full structural breadcrumb remains hover/detail
  affordance

Frames and tile groups are summarized here because they have broader structural
or graph-visible meaning. Panes do not, so panes are given the richer, primary
representation in the tree body below.

### 5.3 Pane Tree Projection (the interesting part)

The main body of the sidebar is a tree-style projection of the current
workbench context. It uses `WorkbenchChromeProjection` (§6) as its data model.

**Tile groups and frames:**

- **Tile groups** (`Container::Tabs`) represent the familiar tabs-style browsing
  unit: a set of nodes rendered in the same space. In the sidebar direction,
  the active group is summarized by the group chip while member panes appear as
  rows in the pane tree.
- **Frames** are persistent, named pane arrangements — nearly arbitrary spatial
  compositions of splits and tab groups that persist as a unit. A frame is the
  composable, grouped, reusable layout object that arc/zen gestures at with their
  "split" feature, but generalized. Frames are derived from graph truth: a frame
  is a subgraph of nodes whose spatial relationships (splits, tab groupings) are
  made explicit and saved. Both tile groups and frames are projected from graph
  relationships into the tile tree, not the reverse.

The sidebar body shows:
- pane rows as the primary leaves: node panes, graph panes, and tool panes
- lightweight split/group structure only where it helps orientation
- active pane highlighting, runtime badges, and per-row actions

**Pane row behavior:**

Pane rows are compact at rest: icon + truncated title + badge tokens. On click:
focuses that pane. On hover or expand activation: the row reveals per-pane
actions without a context menu:
- Close (×)
- Pin / Unpin snapshot
- Promote to Active / Demote to Warm
- "Open with…" — viewer backend selector (see §5.4)

### 5.4 Viewer Backend Selector

When the active viewer backend for a pane is ambiguous (multiple registered
viewers can handle the MIME type, including mod-contributed viewers), the
"Open with…" action on the pane row opens a small picker:

```
Open [page title] with:
  ○ Web Viewer (Servo) — current
  ○ Web Viewer (Wry)
  ○ Reader Mode
  ○ PDF Viewer
  ○ [mod: Custom Viewer]
```

This makes viewer backend selection a first-class affordance instead of a
debug/settings concern. It lives on the pane row because it is a per-pane
decision.

### 5.5 Structural Controls

| Control | Authority | Notes |
|---------|-----------|-------|
| Open in split | `WorkbenchIntent::SplitPane` | Creates a split from the focused pane |
| Promote / Demote | `PromoteNodeToActive`, `DemoteNodeToWarm` | Node lifecycle for focused pane |
| Route to adjacent node | `OpenConnected` | Opens a node connected by a graph edge to the focused node in the adjacent split (or a new split if none exists). The "route" is the graph edge; the destination is the connected node |
| Collapse / restore tile group | `WorkbenchIntent` | Best surfaced from the tile-group chip dropdown rather than as a permanent top-level button |
| Pane runtime badges | Diagnostics | Backend mode, degraded state, loading, crash, pin state — shown primarily on pane rows and optionally mirrored in the header |

**"Route to adjacent node" clarification:** This button reads the graph edges
of the node in the focused pane and opens the most relevant connected node in
a split alongside the current pane. "Most relevant" is defined by edge weight
and recency. It is a graph-driven spatial action: the graph relationship
between two nodes becomes their tile-tree proximity.

### 5.6 Visibility Rule

The default workbench-scoped Navigator host appears automatically when a hosted
workbench surface is active beyond the primary graph surface: node panes, tool
panes, additional graph-view panes, or promoted/tiled config pages. It
collapses when the app returns to the graph-only substrate. It can be pinned
open explicitly — see §8.

Transient overlay surfaces above the graph do not, by themselves, force the
sidebar to appear.

---

## 6. WorkbenchChromeProjection — Derived Host-Chrome Model

The Workbench chrome renders only from a derived model, never directly from
scattered app state. `WorkbenchChromeProjection` is computed each frame from
graph state and tile tree structure, and is intentionally render-form agnostic:
the desktop default is the sidebar, but the same projection could drive a
future compact bar/rail fallback.

This model should be read narrowly:

- it is the data contract for workbench-scoped host chrome,
- it is not the semantic definition of Navigator sections, row meaning, or
  relation-family projection rules,
- any relation-family or section grammar shown inside the host is still
  Navigator-owned and should align with `NAVIGATOR.md` and
  `graph/2026-03-14_graph_relation_families.md`.

```rust
/// Computed each frame from graph + tile tree; fed directly into Workbench chrome render.
pub struct WorkbenchChromeProjection {
    /// Read-only focused-pane status badges (backend, degraded, load/media/download summaries).
    /// Actionable viewer controls remain tile-local and are not projected here.
    pub focused_pane_status: Vec<PaneStatusBadge>,
    /// Active frame name and switchable frame list.
    pub frame: FrameProjection,
    /// Active tile-group summary for the focused pane context.
    pub active_group: Option<TileGroupProjection>,
    /// Tree rows for the current frame / workbench context.
    pub pane_tree: Vec<WorkbenchTreeRow>,
    /// Compact topology path for the focused pane.
    pub topology_path: Vec<TopologySegment>,
    /// Runtime badges for the focused pane.
    pub pane_badges: Vec<PaneBadge>,
    /// Connected nodes available via "route to adjacent" action.
    pub adjacent_candidates: Vec<NodeKey>,
}

pub struct TileGroupProjection {
    pub group_id: Option<TileId>,
    pub title: String,
    pub member_count: usize,
    pub members: Vec<TileId>,
}

pub struct WorkbenchTreeRow {
    pub tile_id: TileId,
    pub pane_id: PaneId,
    pub title: String,
    pub depth: u8,
    pub row_kind: WorkbenchTreeRowKind,
    pub badges: Vec<Badge>,
    pub is_active: bool,
    pub lifecycle: Option<NodeLifecycleState>,
    pub backend: Option<ViewerBackendKind>,
    /// Viewer backends that could also open this node (for "Open with…").
    pub available_backends: Vec<ViewerBackendKind>,
    /// Presentation mode drives graduated chrome and row action visibility.
    pub presentation_mode: PanePresentationMode,
}

pub enum WorkbenchTreeRowKind {
    SplitH,
    SplitV,
    Group,
    GraphView(GraphViewId),
    Node(NodeKey),
    Tool(ToolPaneState),
}

pub enum TopologySegment {
    Frame(String),
    SplitH,
    SplitV,
    TabGroup,
    Pane(String),
}
```

**Derivation source priority:**
1. Navigator/graph-backed arrangement and relation-family projection inputs —
   primary for semantic grouping and section meaning
2. Workbench frame membership / frame context summaries needed for host chrome
3. `egui_tiles` Container shape (structural fallback for orientation only)

Semantic grouping takes priority over tile-tree container shape so the chrome
remains stable when `egui_tiles` simplification rewrites the tree (e.g.,
collapses a single-child container). The sidebar should feel pane-first, not
container-first: panes are the primary rows, while split/group rows are only
as visible as needed for orientation.

Implementation note: older wording in this section may still refer to
`FrameTabSemantics` or pane-tree derivation as though they define the sidebar's
meaning. Under the current boundary model, they are inputs to host chrome and
orientation, not the authority for Navigator projection semantics.

---

## 7. Enter / Exit Workbench Semantics

The transition between graph-only view, overlay-only view, and workbench-hosted
view is an explicit semantic state, not an implicit side-effect of whether some
tile happens to exist.

```rust
pub enum WorkbenchLayerState {
  /// Primary graph surface only. The default graph-scoped Navigator host is the only chrome.
    GraphOnly,
  /// A transient graph overlay surface is open; the graph-scoped host remains the only chrome.
    GraphOverlayActive,
    /// One or more hosted workbench surfaces are active beyond the primary graph surface.
    WorkbenchActive,
  /// The default workbench-scoped Navigator host is pinned; persists even if hosted workbench surfaces close.
    WorkbenchPinned,
}
```

**EnterWorkbench** is triggered by:
- Opening any hosted workbench surface beyond the primary graph surface:
  `OpenNode`, `OpenNodeFrameRouted`, `OpenGraphViewPane`, tiled tool/config
  pages, etc.
- Explicitly promoting a transient overlay surface into a tiled workbench pane
- Explicitly pinning the default workbench-scoped Navigator host

**GraphOverlayActive** is triggered by:
- Opening a transient over-graph settings/config/inspector page that is not yet
  tiled into the workbench

**ExitWorkbench** is triggered by:
- Closing the last hosted workbench surface so the app returns to the primary
  graph surface only
- Explicitly unpinning the default workbench-scoped Navigator host when no hosted workbench surfaces
  remain

**ExitWorkbench default:** when the last hosted workbench surface closes, the
app returns to `GraphOnly` — graph canvas focused, workbench-scoped host hidden.
This is the correct default: the graph is the persistent substrate and closing
all staged content should return you to it, not leave you with empty structure
chrome.

**Overlay return:** closing the last transient overlay in `GraphOverlayActive`
returns to `GraphOnly`. Promoting that overlay to a tiled pane transitions the
app into `WorkbenchActive`.

**Workbench without staged content:** `WorkbenchPinned` is valid even when the
layout has collapsed back to the primary graph surface — the sidebar stays
visible, showing an empty or minimal projection with structural controls
available. This supports users who want workbench chrome persistently available.
It does not change the source of truth; it only exposes one face of it.

The `WorkbenchLayerState` replaces `is_graph_view: bool` / `has_node_panes: bool`
in `toolbar_ui.rs Input` and becomes the single derived value that drives
default workbench-host visibility and graph-only overlay routing.

---

## 8. Chrome Exposure Policy

`WorkbenchLayerState` is the internal state machine. The render system derives
a `ChromeExposurePolicy` from it each frame:

```rust
pub enum ChromeExposurePolicy {
  /// Default graph-scoped Navigator host only.
    GraphOnly,
  /// Default graph-scoped Navigator host plus transient overlay surface; no workbench-scoped host.
    GraphWithOverlay,
  /// Default graph-scoped Navigator host plus the default workbench-scoped host.
  GraphPlusWorkbenchHost,
  /// Default graph-scoped Navigator host plus pinned workbench-scoped host.
  GraphPlusWorkbenchHostPinned,
}
```

Pinned state is persisted as a user preference, not as a `GraphIntent`.

---

## 9. Focus Architecture

The current `RuntimeFocusAuthorityState` distinguishes `SemanticRegionFocus`
variants (GraphSurface, NodePane, Toolbar, etc.). The split chrome direction
adds explicit chrome-region tracking:

```rust
pub enum ChromeRegion {
  GraphScopedNavigatorHost,
  WorkbenchScopedNavigatorHost,
}
```

Focus transitions:
- **F6 cycle**: `GraphSurface` → `GraphScopedNavigatorHost` → `WorkbenchScopedNavigatorHost` (when visible) → `GraphSurface`
- **Tab within graph-scoped host**: advances through control groups left-to-right
- **Tab within workbench-scoped host**: advances header → scope chips → pane tree → structural footer
- **Arrow navigation within pane tree**: `Up` / `Down` move between rows; `Left` / `Right`
  collapse/expand structural rows when exposed
- **Escape from workbench-scoped host**: returns to `GraphSurface` or last active pane
- **Transient overlays**: remain focus-capture surfaces of their own and do not
  force the sidebar into the region cycle unless promoted/tiled

The UxTree build order (§4.2 of `SUBSYSTEM_UX_SEMANTICS.md`) will need
updating: the single toolbar/chrome landmark becomes two default host landmarks
— `GraphScopedNavigatorHost` and `WorkbenchScopedNavigatorHost` — each with
their own traversal sequence and focus return target.

---

## 10. Intent Authority Boundaries

No new intents are required for Phase 1. The existing vocabulary covers all
controls:

- Graph-scoped host: `GraphIntent` variants (physics, lens, tags, fit, undo/redo, view slots, new node/edge/tag)
- Workbench-scoped host header: structural context plus read-only focused-pane status badges; no viewer-scoped navigation ownership
- Workbench-scoped host scope chips: `WorkbenchIntent` for frame/group navigation; projection read-only
- Workbench-scoped host pane tree + structural footer: `WorkbenchIntent` (split, close, promote/demote, pin); `OpenConnected` for adjacent routing
- Settings/overflow: direct app preference mutation plus page open/promotion routing

**What changes is routing, not vocabulary.** Controls that currently dispatch
`GraphIntent` from the toolbar continue dispatching the same intents — from
the correct surface.

New intents that may be added as part of this work (not strictly required for
Phase 1, but named here for completeness):
- `WorkbenchIntent::EnterWorkbench` / `ExitWorkbench` — explicit layer transitions
- `WorkbenchIntent::PinWorkbenchHost` / `UnpinWorkbenchHost` — persistence toggle
- `WorkbenchIntent::SelectViewerBackend { tile_id, backend }` — viewer picker result
- `WorkbenchIntent::PromoteOverlaySurfaceToWorkbench { route, mode }` — explicit
  transient-overlay promotion

---

## 11. Implementation Slices

### Slice 1 — Structural split, sidebar scaffold, no new graph features

1. Extract graph-scoped and workbench-scoped host render functions in
   `toolbar_ui.rs`, each with a typed input struct.
2. Introduce `WorkbenchLayerState` on `GuiRuntimeState`; derive it from tile
   tree + overlay routing state each frame.
3. Remove Back/Forward/Reload from global/host ownership paths; do not re-home
  them in the workbench-scoped host. Stage them for tile-local chrome on
  tiled panes only.
4. Move frame pin controls (P+/P-/W+/W-) into the sidebar structural area.
5. Add the default workbench-scoped host as a right-side `SidePanel`; drive visibility from
   `ChromeExposurePolicy`.
6. Keep all existing controls in place otherwise — do not move physics or lens yet.

**Acceptance criteria:**
- Default graph/workbench Navigator hosts render without regressions when hosted
  workbench surfaces are active
- Workbench-scoped host does not claim Back/Forward/Reload ownership
- `WorkbenchLayerState` transitions correctly on hosted-surface open/close and
  overlay promotion/demotion
- Existing toolbar tests pass

### Slice 2 — WorkbenchChromeProjection and pane tree

1. Implement `WorkbenchChromeProjection` derivation from tile tree + frame
   semantics (§6).
2. Render the pane tree in the workbench-scoped host body using the projection.
3. Add the frame chip + switcher dropdown and active tile-group chip +
   scrollable dropdown.
4. Implement expand-on-hover for pane rows: Close, Promote/Demote, Pin inline.
5. Add the compact topology token and hover-detail breadcrumb.

**Acceptance criteria:**
- Pane tree rows match the current frame/workbench context for the focused pane
- Hover expands a pane row to show Close + Promote/Demote
- Frame and tile-group chips route correctly through their dropdowns
- Topology token/breadcrumb updates correctly as pane focus changes
- Semantic grouping takes priority over tile-tree container shape

### Slice 3 — Dedicated Graph vs Tile Navigation Controls

1. Keep `Undo` / `Redo` fixed in the graph-scoped host as graph-scope history controls.
2. Route Back / Forward / Reload / StopLoad / content zoom into tile-local
  chrome for `PanePresentationMode::Tiled` only.
3. Keep the workbench-scoped host limited to structural context and read-only
  focused-pane status rather than pane navigation dispatch.

**Acceptance criteria:**
- Undo/Redo visible and functional when graph canvas is focused
- Back/Forward/Reload visible and functional in tiled tile chrome when a tiled
  content pane is focused
- Floating panes and docked tiles do not gain surrogate viewer toolbars through
  the workbench-scoped host
- Pane focus does not repurpose graph-scope buttons

### Slice 4 — Graph-scoped host controls migration

1. Add the active graph-view target chip to the graph-scoped host.
2. Move physics controls out of the in-canvas overlay into a graph-scoped host chip.
3. Move lens / view-dimension picker into a graph-scoped host chip.
4. Add `GraphViewSlot` strip to the graph-scoped host.
5. Move active tag filter chips to the graph-scoped host.
6. Expand sync badge to include `SyncNow` and trust controls on click.

**Acceptance criteria:**
- Physics panel no longer renders as a floating in-canvas overlay
- Lens and view-dimension accessible from the graph-scoped host without entering settings
- Active graph-view target is always legible in the graph-scoped host
- `GraphViewSlot` controls render and route correctly
- Sync badge expands to Verse controls

### Slice 5 — Viewer backend selector + tile-local viewer controls

1. Implement viewer-control contribution in tile-local chrome (each viewer
  declares what controls it contributes to tiled tile chrome, not to the
  workbench-scoped host header).
2. Implement "Open with…" picker on pane row expand using `available_backends`
   from `WorkbenchTreeRow`.
3. Wire `WorkbenchIntent::SelectViewerBackend` through the apply layer.

**Acceptance criteria:**
- Reader mode toggle / compat affordance appears in tile-local chrome when the
  focused pane is a tiled web node
- "Open with…" appears on pane row expand when multiple backends are available
- Selecting a backend re-attaches the viewer for that tile

### Slice 6 — Settings routing + ambient badge cleanup

1. Settings and config pages route through §4.2 logic (transient graph overlay,
   or tiled workbench pane depending on layer state and explicit promotion).
2. Semantic depth active → badge chip in the graph-scoped host.
3. Backend/degraded state → pane badge in the workbench-scoped host row/header.
4. Remove the standalone "Clr" button; move Clear Data into overflow menu.
5. Remove the "View toggle" button; `WorkbenchLayerState` + sidebar visibility
   replace it.

**Acceptance criteria:**
- Config pages can remain transient overlays over the graph or be explicitly
  promoted into tiled workbench panes
- Semantic depth active state is visible in the graph-scoped host
- Pane backend/degraded state is visible in the workbench-scoped host row/header
- Clear Data and similar destructive actions live in overflow, not as permanent chrome buttons

---

## 12. Acceptance Criteria (Overall)

- [ ] The default graph-scoped Navigator host is stable — same controls regardless of what tiles are open
- [ ] The default workbench-scoped Navigator host appears/disappears on EnterWorkbench / ExitWorkbench
- [ ] `GraphOverlayActive` does not force the workbench-scoped Navigator host to appear
- [ ] ExitWorkbench (last hosted workbench surface closed) defaults back to graph canvas focus
- [ ] Workbench host pinning (`WorkbenchPinned`) persists across sessions
- [ ] Pane tree accurately projects the current frame/tile/group context
- [ ] Pane row expand exposes Close, Promote/Demote, Pin, Open-with without a context menu
- [ ] Graph-scoped host Undo/Redo remain graph-scoped
- [ ] Back/Forward/Reload live in tile chrome, not in any Navigator host
- [ ] Physics and lens controls accessible from graph-scoped host chips (no overlay panel)
- [ ] Sync badge expands to Verse controls on click
- [ ] Config pages open as transient overlays or workbench panes (no modal windows)
- [ ] All existing `GraphIntent` dispatch paths preserved (no regressions)
- [ ] Focus cycling (F6) covers the default graph/workbench Navigator hosts in the correct order
- [ ] `WorkbenchLayerState` transitions tested in `gui_orchestration_tests.rs`
- [ ] UxTree build updated to emit separate default host landmarks (`GraphScopedNavigatorHost`, `WorkbenchScopedNavigatorHost`)

---

## 13. Non-Goals

- Per-viewer sidebar/header extensions beyond the `ViewerControl` contribution contract
  (reading mode specifics, PDF controls) — those are Viewer trait implementation
  details, not this plan's scope
- Radial menu or Command Palette surface changes — separate plans
- Mobile / touch layout — not a current target
- New graph-scope features (new lens types, new tag filters) — this plan only
  relocates controls, not adds capabilities
- Left-vs-right sidebar preference and a narrow-width compact fallback bar —
  valid future work, but Phase 1 assumes a right-side desktop sidebar
- Extending or replacing `egui_tiles` Container types — that is valid future work
  (new container semantics for frames, custom tile group shapes) but out of scope
  for the chrome split itself
