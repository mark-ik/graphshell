# Navigator Interaction Contract

**Date**: 2026-03-17 (moved from `workbench/navigator_graph_isomorphism_spec.md` 2026-03-15)
**Status**: Design — Pre-Implementation
**Purpose**: Canonical interaction grammar for Navigator hosts. Defines which
rows select nodes, which rows expand containers, and how node navigation
resolves between graph and workbench presentations.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md) — Navigator domain spec and authority boundaries
- [navigator_backlog_pack.md](navigator_backlog_pack.md) — implementation backlog
- `../../technical_architecture/graphlet_model.md`
- `../workbench/graphlet_projection_binding_spec.md`
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../canvas/frame_graph_representation_spec.md`
- `../canvas/2026-03-14_graph_relation_families.md`
- `../../TERMINOLOGY.md`

**Alignment note (2026-03-27)**: newer Navigator planning distinguishes between
multiple projection forms owned by the same Navigator authority:

- graphlet-oriented projections (ego/corridor/component/etc.),
- relation-family-oriented projections and section modes (`Workbench`,
  `Containment`, `Semantic`, `All nodes`),
- host/form-factor differences owned by Shell/Workbench rather than Navigator
  semantics.

This interaction contract therefore treats graphlets and relation-family modes
as parallel projection shapes with the same click grammar, not as competing
ownership models.

---

## 1. Core Principle

The Navigator is a projection over graph-backed nodes and arrangement
containers, but not every row represents the same kind of thing.

The canonical click grammar is therefore **row-type specific**:

| Row kind | Single click | Double click |
|---------|-------------|--------------|
| `Node` row | Select node | Navigate to node presentation target |
| `Frame` row | Expand/collapse contents | No-op |
| `Tile` row | Expand/collapse contents | No-op |
| Other structural row (`Split`, `Group`) | Expand/collapse contents | No-op |

All Navigator hosts must behave identically at the interaction-grammar level.
Differences in host edge, form factor, scope, and margin settings must not
change row semantics.

---

## 2. Projection / Actions / Authority

### 2.1 Projection

The Navigator projects:

- graph-backed nodes that belong to the currently resolved graphlet projection,
  **regardless of lifecycle**. Cold graphlet members appear with a ○ residency
  badge; warm/active members appear with a ● badge. Cold members are not
  suppressed.
- structural arrangement containers (`Frame`, `Tile`, `Split`, `Group`)
- node residency/presentation state (`NodeLifecycle`) needed to decide whether
  a node is live in workbench memory or cold on the graph

In family-oriented Navigator modes, the same row grammar applies to
relation-family sections and rows derived from arrangement/containment/semantic
projection sources. The projection shape may change, but row-type interaction
semantics do not.

**Updated from prior specification**: the Navigator previously suppressed nodes
without a live tile representation. Under the graphlet model, graph membership
(selector-resolved connectivity + lifecycle) is sufficient for Navigator
projection. The tile tree is not the authority. Cold members are always visible
so users can discover and activate them.

Projection resolution follows the same scope stack as workbench graphlet
routing:

- selection override
- graph-view override
- graph default

Graphlets and relation-family projections should not be conflated:

- graphlets remain the canonical model for ego/corridor/component-style local
  worlds defined in `../../technical_architecture/graphlet_model.md`,
- relation families remain the canonical model for family-oriented section and
  mode semantics defined in `../canvas/2026-03-14_graph_relation_families.md`.

Either may be the active Navigator projection shape, but both remain
Navigator-owned projections over shared graph/workbench inputs rather than
independent truth models.

Bare panes with no container-backed node representation (e.g. graph-view panes
not belonging to any graphlet) do **not** appear as Navigator rows, even if they
are persisted in a frame snapshot.

### 2.2 Actions

The Navigator may emit:

- `SelectNode { node_key }`
- `NavigateToNodePresentation { node_key }`
- `ToggleNavigatorExpansion { row_id }`
- `DismissNode { node_key, container_context }`
- `SwitchNodePresentationSurface { node_key }`

### 2.3 Authority

- Graph subsystem owns node selection truth.
- Runtime/workbench residency authority owns node presentation-target
  resolution (`live in workbench` vs `cold on graph`).
- Navigator projection owns container expansion/collapse state.
- Workbench arrangement authority owns removal of a node from its current
  frame/group container.

---

## 3. Node Row Contract

### 3.1 Single Click on a Node Row

**Sentence form**:

When the user single-clicks a node row in the Navigator, `SelectNode` is
emitted, which updates graph selection truth owned by the graph subsystem,
resulting in a selected row plus matching graph selection highlight.

**Behavior**:

1. The node becomes selected in `GraphViewId` selection truth.
2. The selected row reveals contextual trailing actions:
   - `Dismiss`
   - `Switch Surface`
3. If the graph canvas is visible, the matching node highlight appears there.
4. Focus remains in the Navigator surface.
5. No navigation occurs on single click.

### 3.2 Double Click on a Node Row

**Sentence form**:

When the user double-clicks a node row in the Navigator, the action depends on
the node's lifecycle:

- **Warm/active node**: `NavigateToNodePresentation` — routes to the node's
  existing tile in the workbench.
- **Cold node**: `OpenNode(N)` — activates the node; opens a tile in the
  graphlet's existing tab group (or a new tile if the graphlet has no warm
  members). The node's lifecycle transitions from `Cold` to `Warm`/`Active`.

**Behavior**:

1. The node is selected if it is not already selected.
2. If the node is warm/active (has a live tile), navigation routes to workbench.
3. If the node is cold, `OpenNode(N)` fires; the tile opens in the graphlet's tab
   group; lifecycle transitions to `Active`.
4. Focus moves to the resolved presentation target.

### 3.3 Selected Node Row Actions

Only selected node rows expose these actions:

#### Dismiss

When the user activates `Dismiss` on a selected warm/active node row, `DismissTile`
is emitted: the tile is closed and the node's lifecycle becomes `Cold`. All graph
edges are preserved — the node remains in its graphlet and continues to appear in
the Navigator with a ○ badge.

To remove a node from its graphlet entirely (retract durable edges), use
`RemoveFromGraphlet`. To delete the node from the graph, use the Delete action.
These three gestures must never be aliased.

#### Switch Surface

When the user activates `Switch Surface` on a selected node row,
`SwitchNodePresentationSurface` is emitted, which resolves the non-focused
presentation target owned by runtime/workbench residency authority, resulting
in the graph being shown when workbench is focused, or the workbench being
shown when graph is focused.

---

## 4. Structural Row Contract

### 4.1 Single Click on a Frame / Tile / Split / Group Row

**Sentence form**:

When the user single-clicks a structural Navigator row, `ToggleNavigatorExpansion`
is emitted, which updates Navigator expansion state owned by the Navigator
projection, resulting in the container expanding or collapsing to show or hide
its contents.

**Behavior**:

1. No graph selection write occurs.
2. No navigation occurs.
3. Expansion reveals child node-bearing rows, which then participate in the
   node-row rules from §3.

### 4.2 Double Click on Structural Rows

Double-click on structural rows is currently a no-op. If future specs add frame
activation or group activation from the Navigator body, that must be specified
explicitly rather than inferred.

### 4.3 Header Chips

Frame chips and tile chips in the workbench header/sidebar are structural
switch affordances, not node rows. They do not participate in node selection.

### 4.4 Security and Permission Chips

Navigator header chrome may also render focused-node trust and origin
permission chips. These are neither structural rows nor node rows; they are
read-only status affordances with detail-launch behavior.

Required behavior:

1. Trust chips summarize the focused or selected node's current transport trust
  state and degraded-origin warnings.
2. Permission chips summarize per-origin state for camera, microphone,
  location, and notifications.
3. Clicking a trust or permission chip opens the relevant detail surface,
  inspector, or permission-management route. It does not write graph
  selection truth.
4. Trust and permission chips must not disappear solely because the Navigator
  switches between host form factors, host edges, or graph/workbench scope
  while a node-backed content surface remains active.

### 4.5 Focused Content Status and Pane/Tile Alignment

Navigator chrome may project **focused-content status**, but it is not the
canonical command surface for viewer-local actions.

The command-ownership split is:

- **Floating pane**: Promote + Dismiss only; no viewer toolbar
- **Docked tile**: reduced identity chrome only; no viewer toolbar strip
- **Tiled tile**: tile-local viewer chrome owns Back / Forward / Reload / Zoom /
  Find in page / compat-mode affordances
- **Workbench-scoped Navigator host**: structural management plus focused-pane
  status projection; no surrogate viewer toolbar

Allowed Navigator projections for focused content are therefore limited to
read-only or detail-launch status affordances such as:

- **Load-state badge**: loading / idle / failed summary; activation may focus
  the tile or open a detail surface, but does not replace tile-local Stop/Reload
  chrome
- **Backend/degraded badge**: effective viewer backend, compatibility/degraded
  state, or blocked reason summary
- **Media badge**: indicates active media state and may open the relevant media
  detail surface
- **Downloads badge**: indicates active or recent download state and may open
  downloads history/manager

Required behavior:

1. Navigator hosts must not render Back / Forward / Reload / Find-in-page /
   content zoom / compat toggle as primary command ownership for the focused
   viewer.
2. Floating panes must not acquire a viewer toolbar through Navigator chrome;
   they remain ephemeral preview surfaces with Promote / Dismiss only.
3. Docked tiles may project presentation or backend badges in Navigator, but
   Navigator must not synthesize a full viewer toolbar for docked presentation.
4. Read-only status badges projected by Navigator route to viewer/runtime
   detail surfaces or focus handoff, not graph-selection mutation.
5. Keyboard routes such as `Ctrl+F`, `Ctrl+=`, `Ctrl+-`, and `Ctrl+0` must
   continue targeting tile-local viewer behavior when a tiled content surface is
   focused; Navigator projection must not hijack them.

---

## 5. Synchronization

Navigator node-row selection and graph canvas selection are synchronized because
they derive from the same selection truth. Expansion state is not synchronized
with the graph because it is Navigator-local structural state.

Suggested row state model:

```rust
pub enum NavigatorRowState {
    Unselected,
    SelectedCold,
    SelectedLiveInWorkbench,
}
```

This is enough to drive:

- row highlight
- row action visibility
- graph selection highlight
- residency badges (`cold`, `live`, etc.)

Additional focused-node chrome state, such as trust, per-origin permission,
backend/degraded badges, and load/media/download summaries, is synchronized
from security/runtime truth rather than from row state.

Navigator may project focused-pane status from viewer/runtime truth, but
actionable viewer controls remain tile-local and are not inferred from
Navigator-local state.

---

## 6. Relationship To Copy / Move / Associate

This spec assumes cross-context reuse is explicit. A node does not generically
"open into another frame." Instead, the user performs one of:

- `MoveNode`
- `AssociateNode`
- `CopyNode`

Copied nodes are distinct graph nodes with their own UUIDs. The Navigator
therefore projects node-bearing contextual entries, not presentation instances
of one shared node identity.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Single click on node row selects node | Test: click node row -> graph selection truth updated; no navigation |
| Selected node row shows trailing actions | Test: click node row -> `Dismiss` and `Switch Surface` appear |
| Double click on live node row navigates to workbench | Test: node live in memory -> double-click row -> workbench shown |
| Double click on cold node row opens tile in graphlet tab group | Test: node cold -> double-click row -> `OpenNode` fires -> tile opens in graphlet tab group; lifecycle becomes Active |
| Cold node row shows ○ badge | Test: dismiss node -> row remains in Navigator with ○ lifecycle badge |
| Warm/active node row shows ● badge | Test: node has live tile -> row shows ● badge |
| Single click on frame/group row expands contents | Test: click structural row -> child rows revealed |
| Structural rows do not write graph selection on click | Test: click frame/group row -> no node selection write |
| Bare panes are absent from Navigator | Test: pane without container-backed node/tile row -> not listed |
| Dismiss closes tile and demotes node to Cold | Test: dismiss selected live node -> tile closed; node becomes Cold; edges preserved; node remains in Navigator with ○ badge |
| Dismissed node remains in graphlet | Test: dismiss node -> durable edges intact; node still in Navigator row for its graphlet |
| Right-click cold node offers RemoveFromGraphlet | Test: right-click cold node row -> context menu shows `RemoveFromGraphlet`; activating retracts durable edges; node leaves Navigator row |
| Multiple Navigator hosts use the same grammar | Test: the same node/structural row interactions behave identically in top/bottom/left/right hosts regardless of form factor |
| Focused secure web node shows trust chip | Test: focus secure web-content node -> Navigator header shows secure trust indicator without opening settings |
| Mixed-content node shows degraded warning chip | Test: focus node with mixed content -> Navigator header shows degraded trust warning |
| Focused origin shows permission chips | Test: focus node with origin permission state -> camera/microphone/location/notifications chips show `allowed` / `blocked` / `prompt` as applicable |
| Trust/permission chip click does not change node selection | Test: click security chip -> detail surface opens; graph selection truth unchanged |
| Navigator does not become a viewer toolbar | Test: focused tiled viewer -> Navigator host shows at most status badges; Back/Forward/Reload/Zoom remain tile-local chrome |
| Floating pane remains chromeless in Navigator | Test: floating pane active -> Navigator does not expose viewer controls for it; pane keeps Promote/Dismiss only |
| Docked tile does not gain surrogate viewer chrome | Test: docked tile focused -> Navigator may show status badges but no Back/Forward/Reload/Zoom toolbar |
| Focused viewer `Ctrl+F` opens find in page | Test: focused tiled content viewer + `Ctrl+F` -> page-local find surface opens; graph search state unchanged |
| Content zoom remains distinct from graph zoom | Test: focused tiled content viewer + zoom shortcut -> rendered page zoom changes; graph camera unchanged |
| Playing media shows media status badge | Test: focused tile playing audio -> Navigator may show media indicator and opens relevant detail surface on click |
| Active download shows downloads status badge | Test: focused content starts a download -> Navigator shows download indicator and opens download manager on click |
