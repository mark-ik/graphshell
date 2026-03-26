# Multi-View Pane — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active

**Related**:

- `GRAPH.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-22_multi_graph_pane_plan.md`
- `2026-03-05_hybrid_graph_view_overview_atlas_plan.md`
- `../workbench/WORKBENCH.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../workbench/pane_chrome_and_promotion_spec.md`
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../system/register/canvas_registry_spec.md`
- `../system/register/workbench_surface_registry_spec.md`
- `../../TERMINOLOGY.md` — `Graph View`, `GraphViewId`, `Scope Isolation`, `Graph Scope`

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- graph-scoped Navigator hosts = chrome surfaces naming the active
  `GraphViewId` / graph target.
- `Navigator` = graph-backed hierarchical projection over relation families. Legacy alias: "file tree".
- workbench = arrangement boundary.

This spec governs view/pane interaction contracts without collapsing graph truth into workbench structure.
Graph views are graph-owned targets first and hosted pane payloads second.

## Contract template (inherits UX Contract Register §2A)

Normative multi-view contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- Navigator is not content truth authority.
- Physics presets are not camera modes.
- Nodes remain graph identity; tiles are their workbench presentation grammar.
- Graphlets remain grouped graph arrangement objects; tile groups are their workbench presentation grammar.

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Pane as universal host** — panes host view payloads; graph views are one kind.
2. **Graph view identity** — `GraphViewId`, per-view camera, per-view lens.
3. **Per-view local layout** — multi-pane layout isolation semantics.
4. **Graph-view layout manager** — slot-grid lifecycle and pane routing semantics.
5. **Scope isolation** — interaction independence between sibling panes.
6. **Semantic tab overlay** — `FrameTabSemantics`, structural hoist/unhoist, simplification repair.

Reading order note:

- Graph-scoped Navigator hosts own graph-target naming and graph-view
  switching.
- This spec starts one level lower: how those graph-owned targets are hosted, routed, and isolated when they appear in panes.

---

## 2. Hosted Surface Contract

The workbench does not special-case graph panes vs. viewer panes vs. tool panes at the layout layer. A pane is a host; its **payload** (`TileKind`) determines rendering and input behavior.

But pane-hosting is not the semantic root:

- a `GraphViewId` exists as graph-scoped identity whether or not it is currently hosted in a pane
- a graph-scoped Navigator host may name the active graph target and remain top-level graph chrome
- the workbench hosts contextual leaves for the active branch, which may include graph-view panes, node/document/media panes, and tool panes

Projection rule:

- nodes project as tiles in workbench chrome
- graphlets project as tile groups in workbench chrome
- this is presentation correspondence, not a collapse of graph terms into workbench terms

```
TileKind =
  | Graph(GraphViewId)       -- graph canvas view
  | Node(NodePaneState)      -- node viewer pane
  | Tool(ToolPaneState)      -- tool/subsystem pane
```

**Invariant**: Workbench layout operations (split, move, close, reorder) apply uniformly across all `TileKind` variants. The workbench must not branch on payload type for structural operations.

**Hierarchy guardrail**: `TileKind::Graph(GraphViewId)` means "this pane presents a graph-owned scoped view." It must not be read as "the workbench owns graph-view identity."

---

## 3. Graph View Identity Contract

### 3.1 GraphViewId

Each hosted graph view pane presents a stable `GraphViewId`. `GraphViewId` is the canonical identity for:

- Per-view camera state (pan offset, zoom level).
- Per-view `ViewDimension` (TwoD / ThreeD).
- Per-view Lens assignment.
- Per-view layout/physics state (local simulation state for this view only).

`GraphViewId` is generated at pane creation and persisted as part of the frame snapshot. It does not change when the pane is moved, split, or reordered.

Desktop chrome tie-in: `GraphViewId` is the identity named by a graph-scoped
Navigator host's target chip, lens/dimension controls, and graph-view slot
strip. Workbench chrome may host panes that present a `GraphViewId`, but it
does not own `GraphViewId` semantics.

### 3.2 Per-View Camera

Camera state is per-`GraphViewId`. Camera commands (`fit`, `fit selection`, zoom, pan) target the focused graph view. Camera state from one graph view must not bleed into a sibling graph view.

### 3.3 Per-View Lens

A Lens may be assigned per-`GraphViewId` or inherited from the workspace default. Per-view Lens assignment overrides the workspace default for that view only. Lens resolution follows: `View → Workspace → User → Default` fallback chain.

---

## 4. Per-View Local Layout Contract

Multiple graph view panes may open simultaneously. Each pane owns its own local layout state.

```
GraphLayoutOwnership =
  | LocalPerView   -- each GraphViewId owns independent node positions/simulation state
```

### 4.1 Local-Per-View Mode (default and only mode)

- Each graph view pane has independent node positions and simulation state.
- Camera remains per-view (independent pan/zoom), and layout state is also per-view.
- A layout change in pane A must not mutate pane B unless an explicit future bridge/sync feature is invoked.
- This is the default and only supported mode for graph view layout ownership.

### 4.2 Cross-View Transfer (explicit action only)

- Cross-view copy/paste or duplicate flows are explicit user actions and are not implicit layout sharing.
- This spec does not define automatic shared-layout behavior between graph views.

---

## 5. Graph-View Layout Manager Contract

Graphshell provides a graph-view layout manager for creating and organizing `GraphViewId`
instances independent of pane hosting.

### 5.1 Entry/Exit triggers

- Enter manager: `GraphIntent::EnterGraphViewLayoutManager`
- Exit manager: `GraphIntent::ExitGraphViewLayoutManager`
- Toggle manager visibility: `GraphIntent::ToggleGraphViewLayoutManager`

### 5.2 Slot lifecycle

Each slot is `GraphViewSlot { view_id, name, row, col, archived }`.

- Create slot/view: `GraphIntent::CreateGraphViewSlot { anchor_view, direction, open_mode }`
- Rename slot/view: `GraphIntent::RenameGraphViewSlot { view_id, name }`
- Move slot: `GraphIntent::MoveGraphViewSlot { view_id, row, col }`
- Archive slot: `GraphIntent::ArchiveGraphViewSlot { view_id }`
- Restore slot: `GraphIntent::RestoreGraphViewSlot { view_id, row, col }`

Guardrails:

- Active (non-archived) slots must have unique `(row, col)` coordinates.
- Move/restore into occupied coordinates must reject or auto-place deterministically.
- Archiving does not delete graph content; it only removes active slot visibility.
- Active non-archived slots are surfaced as compact selectors in a graph-scoped
  Navigator host; the full row/column manager remains a graph-surface workflow,
  not a workbench-scoped Navigator responsibility.

### 5.3 Routing to workbench panes

Routing from manager to pane hosting is explicit:

- `GraphIntent::RouteGraphViewToWorkbench { view_id, mode }` emits
  `WorkbenchIntent::OpenGraphViewPane { view_id, mode }`.
- Workbench authority opens/focuses the pane and applies split/tab mode.
- Routed panes become visible in workbench-scoped Navigator host/tree
  projection, while a graph-scoped Navigator host continues to name the active
  graph target independently.
- Reducer never mutates tile tree directly.

This is the key separation: routing a graph view into the workbench changes contextual presentation, not graph-view semantic ownership.

### 5.4 Persistence shape

Layout manager state persists as:

- `PersistedGraphViewLayoutManager { version, active, slots[] }`
- reserved storage key: `workspace:settings-graph-view-layout-manager`

This persists slot metadata and manager active state, not tile geometry.

---

## 6. Scope Isolation Contract

Graph scopes rendered in separate panes within the same workbench are **interaction-isolated by default**.

| Interaction type | Default behavior |
|-----------------|-----------------|
| Selection | Independent per pane; selecting a node in pane A does not change selection in pane B |
| Camera | Independent per pane |
| Gestures (lasso, drag) | Local to the pane that receives the gesture |
| Layout/physics state | Independent per pane |

**Explicit sync**: Scope sync (e.g., "mirror selection across panes") requires an explicit bridge rule. Bridge rules are future work; this spec records that isolation is the default.

---

## 7. Semantic Tab Overlay Contract

### 7.1 Purpose

`FrameTabSemantics` is an optional overlay on top of the `egui_tiles` structural tree. It persists semantic tab group membership so that meaning is not lost when `egui_tiles` simplification (`simplify()`) restructures the tree.

**Invariant**: The `egui_tiles` workbench tree is structural state, not semantic truth. `FrameTabSemantics` is semantic truth.

### 7.2 Data Model

```
FrameTabSemantics {
    version: u32,
    tab_groups: Vec<TabGroupMetadata>,
}

TabGroupMetadata {
    group_id: TabGroupId,    -- Uuid
    pane_ids: Vec<PaneId>,   -- ordered tab membership
    active_pane_id: Option<PaneId>,   -- must be a member, or None after repair
}
```

- A pane belongs to at most one semantic tab group.
- Persistence: serialized with rkyv into the frame bundle (redb). This is frame state, not WAL data — it must not appear in `LogEntry` variants.
- `FrameTabSemantics` is also a semantic input to sidebar/tree projection per
  `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`; the
  projection is read-only and must not become a second semantic owner.

### 7.3 Structural Hoist / Unhoist Contract

This section is about structural workbench-tree changes only. It is not the graph-citizenship boundary.

**Hoist** (pane rest state → tab container): pane is hoisted into a `Container::Tabs` node in the tile tree. Semantic metadata is created or updated.

**Unhoist** (tab container → pane rest state): pane is unhoisted; tab container is removed. Semantic metadata is retained (the tab group still exists in `FrameTabSemantics`; only the visual container is removed). The pane can be re-hoisted without losing group membership.

**Invariant**: Hoist and unhoist are explicit structural intents (e.g., `HoistPaneToTabs`, `UnhoistPaneFromTabs`). They must not be ad hoc tree rewrites at UI callsites. The `render/*` layer captures the user event and routes it to `graph_app.rs` as an intent; `graph_app.rs` is the authority for the structural decision; `desktop/*` applies the workbench tree mutation.

Terminology guardrail:

- `Promotion` is reserved for pane enrollment into graph-backed `Tile` state per `../workbench/2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` and `../../TERMINOLOGY.md`.
- Structural hoist/unhoist must not perform graph enrollment writes.

### 7.4 Simplification Repair

When `egui_tiles::simplify()` runs and removes a tab container that has semantic metadata:

1. The semantic metadata (`TabGroupMetadata`) is preserved.
2. The pane's rest state (pane-only representation without a tab container) is valid for all tab-aware features.
3. The `active_pane_id` is validated on restore: if the previously active pane is no longer a member, `active_pane_id` is repaired to `None`.

**Invariant**: Graphshell must remain compatible with `egui_tiles` simplification. No semantic data may be stored in the tile tree shape alone.

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Camera in pane A does not affect pane B | Test: pan in pane A → pane B camera unchanged |
| Selection in pane A does not affect pane B (isolation) | Test: select node in pane A → pane B selection unchanged |
| Layout positions are isolated between panes | Test: move node in pane A → pane B positions unchanged |
| Enter/exit manager updates manager active state | Test: enter/exit/toggle intents update `GraphViewLayoutManagerState.active` |
| Slot create/rename/move/archive/restore flows are deterministic | Test: lifecycle intent sequence yields expected slot metadata |
| Slot coordinate collision is guarded | Test: moving slot into occupied coordinates is rejected |
| Graph-view route intent dispatches workbench pane-open intent | Test: route intent enqueues `OpenGraphViewPane` |
| Routed graph view appears in workbench chrome projection without losing graph target identity | Test: route view to workbench -> host/tree row appears while a graph-scoped Navigator host target remains `view_id`-stable |
| `GraphViewId` persists across reorder | Test: reorder pane → `GraphViewId` unchanged |
| Hoist creates `Container::Tabs` node | Test: hoist pane → tile tree contains `Container::Tabs` parent |
| Unhoist removes container but retains semantic metadata | Test: unhoist → `TabGroupMetadata` still present in `FrameTabSemantics` |
| `simplify()` does not lose tab group membership | Test: simplify removes tab container → `TabGroupMetadata` intact; re-hoist restores group |
| `active_pane_id` repaired when member removed | Test: remove pane from group → `active_pane_id` set to `None` |
| Hoist/unhoist routed through structural intents | Architecture invariant: no direct tile tree mutation from render callsites for hoist/unhoist |
| Structural hoist/unhoist does not enroll graph citizenship | Test: hoist/unhoist cycle without Tile enrollment emits no graph node write |
