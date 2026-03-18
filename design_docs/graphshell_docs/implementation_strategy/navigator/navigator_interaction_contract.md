# Navigator Interaction Contract

**Date**: 2026-03-17 (moved from `workbench/navigator_graph_isomorphism_spec.md` 2026-03-15)
**Status**: Design — Pre-Implementation
**Purpose**: Canonical interaction grammar for the Sidebar Navigator and Toolbar
Navigator. Defines which rows select nodes, which rows expand containers, and
how node navigation resolves between graph and workbench presentations.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md) — Navigator domain spec and authority boundaries
- [navigator_backlog_pack.md](navigator_backlog_pack.md) — implementation backlog
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../canvas/frame_graph_representation_spec.md`
- `../canvas/2026-03-14_graph_relation_families.md`
- `../../TERMINOLOGY.md`

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

The Sidebar Navigator and Toolbar Navigator must behave identically.

---

## 2. Projection / Actions / Authority

### 2.1 Projection

The Navigator projects:

- graph-backed nodes that have container-backed tile representations
- structural arrangement containers (`Frame`, `Tile`, `Split`, `Group`)
- node residency/presentation state needed to decide whether a node is live in
  workbench memory or cold on the graph

Bare panes with no container-backed tile/node representation do **not** appear
as Navigator rows, even if they are persisted in a frame snapshot.

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

When the user double-clicks a node row in the Navigator, `NavigateToNodePresentation`
is emitted, which resolves the node's active presentation target from residency
state owned by runtime/workbench authority, resulting in workbench navigation
if the node is live and graph navigation if the node is cold.

**Behavior**:

1. The node is selected if it is not already selected.
2. If the node's view is already live/in memory, navigation goes to workbench.
3. If the node is cold, navigation goes to graph.
4. Focus moves to the resolved presentation target.

### 3.3 Selected Node Row Actions

Only selected node rows expose these actions:

#### Dismiss

When the user activates `Dismiss` on a selected node row, `DismissNode` is
emitted, which removes the node from its current frame/group container owned by
workbench arrangement authority, resulting in the node disappearing from that
container and being demoted to `Recent` / `Cold`.

If the node is already cold, dismiss deletes it from graph truth.

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
| Double click on cold node row navigates to graph | Test: node cold -> double-click row -> graph shown |
| Single click on frame/group row expands contents | Test: click structural row -> child rows revealed |
| Structural rows do not write graph selection on click | Test: click frame/group row -> no node selection write |
| Bare panes are absent from Navigator | Test: pane without container-backed node/tile row -> not listed |
| Dismiss removes selected node from current container | Test: dismiss selected node row -> row disappears from current frame/group |
| Dismiss demotes node to `Recent` / `Cold` | Test: dismiss selected live node -> node becomes cold/recent |
| Dismissing cold node deletes it | Test: dismiss cold node -> node removed from graph and Navigator |
| Sidebar and Toolbar Navigator use the same grammar | Test: same node/structural row interactions behave identically in both surfaces |
