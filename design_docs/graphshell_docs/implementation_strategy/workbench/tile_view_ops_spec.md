# Tile View Operations — Interaction Spec

**Date**: 2026-03-12
**Status**: Canonical interaction contract
**Priority**: Implementation-ready (documents existing implementation)

**Related**:

- `WORKBENCH.md`
- `workbench_frame_tile_interaction_spec.md`
- `pane_chrome_and_promotion_spec.md`
- `focus_and_region_navigation_spec.md` (FocusCycle / FocusCycleRegion)
- `shell/desktop/workbench/tile_view_ops.rs`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **`TileOpenMode`** — how panes are inserted relative to existing layout.
2. **Pane open/focus semantics** — focus-existing-or-create for Graph, Node,
   and Tool panes.
3. **Pane close semantics** — removal and post-removal tree normalization.
4. **`normalize_parent_after_child_removal`** — the recursive cleanup
   invariant.
5. **`ensure_active_tile`** — fallback activation after active tile loss.
6. **`FocusCycle` / `FocusCycleRegion`** — region cycling semantics.
7. **Multi-tile selection** — `group_selected_tiles`, `ordered_selected_pane_tile_ids`.
8. **`toggle_tile_view`** — node pane toggle contract.

---

## 2. TileOpenMode

```
TileOpenMode =
  | Tab
  | SplitHorizontal
```

All open operations accept a `TileOpenMode`. The mode controls insertion
shape, not pane identity.

| Mode | Effect |
|---|---|
| `Tab` | New pane added to an existing Tabs container at root, or root wrapped into a new Tabs container |
| `SplitHorizontal` | New pane inserted into an existing horizontal Linear container at root, or root split into a new horizontal Linear container |

**Leaf-wrapping invariant**: A raw leaf pane (Pane node, not a container) must
never be directly split. Before any split operation, the target pane is
wrapped in a Tabs container. This prevents malformed trees where a pane is the
direct child of a Linear/Grid container.

---

## 3. Graph Pane Operations

### 3.1 `open_or_focus_graph_pane`

Default mode: `TileOpenMode::Tab`.

### 3.2 `open_or_focus_graph_pane_with_mode`

If a Graph pane with the requested `GraphViewId` already exists in the tree,
activates it without inserting a new pane.

If no such pane exists, inserts a new Graph pane:

- `Tab`: if root is a Tabs container, adds to it; otherwise wraps root in
  a new Tabs container.
- `SplitHorizontal`: if root is a Tabs container, wraps root in a Tab first
  (respecting the leaf-wrapping invariant), then inserts into or creates a
  horizontal Linear container.

After insertion, the new pane is activated.

### 3.3 `split_pane_with_new_graph_view`

Inserts a new Graph pane adjacent to a specific source pane, using
`wrap_pane_in_split_container`. The source pane and new pane are each wrapped
in Tabs before being joined in a split container.

---

## 4. Node Pane Operations

### 4.1 `open_or_focus_node_pane`

Default mode: `TileOpenMode::SplitHorizontal`.

### 4.2 `open_or_focus_node_pane_with_mode`

If a Node pane for `node_key` already exists, activates it and calls
`refresh_node_pane_render_modes`. No new pane is inserted.

If no pane exists, inserts a new Node pane:

- In both modes, the new node tile is always wrapped in a Tabs container
  (`split_leaf_tile_id`) before being placed in the tree.
- `Tab`: adds to existing root Tabs, or wraps root in new Tabs container with
  the new pane alongside existing root.
- `SplitHorizontal`: adds to existing root Linear, or creates a horizontal
  Linear container. The leaf-wrapping invariant applies: a bare root pane is
  wrapped in a Tabs container before splitting.

After any operation, `refresh_node_pane_render_modes` is called.

### 4.3 `detach_node_pane_to_split`

Removes an existing Node pane for `node_key` (if present) via
`remove_recursively`, then calls `open_or_focus_node_pane_with_mode` in
`SplitHorizontal` mode. This moves an existing pane to a new split position.

### 4.4 `preferred_detail_node`

Returns the node to use as the target for node pane operations when no
explicit `node_key` is provided:

1. If exactly one node is selected in the graph, returns that node.
2. Otherwise, returns the first graph node (by graph node order).
3. Returns `None` if the graph has no nodes.

---

## 5. Tool Pane Operations

Tool panes are gated on the `diagnostics` feature flag. On non-diagnostics
builds, `open_tool_pane` and `close_tool_pane` are no-ops that return `false`.

---

## 6. `close_pane`

Removes the tile for `pane_id` from the tree using `remove_recursively`
(provided by `egui_tiles`), then calls `ensure_active_tile` to restore a
valid active tile.

---

## 7. `normalize_parent_after_child_removal`

Called after any `remove_child_from_container` operation. Recursively
normalizes the tree to prevent malformed states.

```
normalize_parent_after_child_removal(tiles_tree, parent_id) -> bool
```

Resolution rules by remaining child count:

| Remaining children | Action |
|---|---|
| 0 | Remove `parent_id` from its grandparent via `remove_child_from_container`, then recurse on grandparent |
| 1 | Elevate the single remaining child to replace `parent_id` in the grandparent (or as root) |
| 2+ | No structural change needed; return `true` |

**Root handling**: when the parent is the root node:

- 0 children: sets `root = None`.
- 1 child: sets `root = only_child` (the container is replaced by its sole
  remaining child).

**Invariant**: After normalization, no container in the tree has 0 or 1
children, except the root when the root is a single pane (non-container).

---

## 8. `remove_child_from_container`

Removes `child_id` from the specified container:

| Container type | Removal behavior |
|---|---|
| `Tabs` | Finds by position; if removed child was active, activates the successor tab (next index, or previous if at end) |
| `Linear` | Finds and removes; updates size `shares` to remove the child's share slot |
| `Grid` | No-op (returns false) — Grid child removal is not supported |
| `Pane` | No-op (returns false) |

---

## 9. `ensure_active_tile`

Called after close/removal to guarantee a valid active tile exists.

```
ensure_active_tile(tiles_tree) -> bool
```

- Returns `false` (no-op) if the tree already has an active tile.
- If no active tile: attempts to activate in this order:
  1. Any Graph pane
  2. Any Node pane
  3. Any Tool pane (diagnostics feature only)
- Returns `true` if an active tile was successfully set.

---

## 10. `active_graph_view_id`

```
active_graph_view_id(tiles_tree) -> Option<GraphViewId>
```

Returns the `GraphViewId` of the currently active Graph pane, or `None`.
Uses `egui_tiles::Tree::active_tiles()` and returns the last-wins match
(consistent with how `egui_tiles` tracks activation in multi-active scenarios).

---

## 11. Focus Cycling Contract

### 11.1 `FocusCycleRegion` / `FocusCycle`

```
FocusCycleRegion =
  | Graph
  | Node
  | Tool      -- only when diagnostics feature enabled

FocusCycle =
  | Tabs
  | Panes
  | Both
```

`cycle_focus_region(tiles_tree) -> bool` cycles the active tile through
available pane regions in order: `Graph → Node → Tool → Graph` (wrapping).

- Only regions with at least one pane in the tree are included in the cycle.
- If the current active pane is not one of the recognized region types, the
  cycle starts at `Graph`.
- Returns `true` if the active tile changed; `false` if only one region type
  is present and it was already active.

### 11.2 `cycle_focus_region_with_policy`

Applies `FocusCycle` policy to scope the cycling behavior. The current
implementation routes through `cycle_focus_region`; the policy is reserved for
future fine-grained cycling (e.g., cycling only within panes vs. tabs).

---

## 12. Multi-Tile Selection Operations

### 12.1 `ordered_selected_pane_tile_ids`

```
ordered_selected_pane_tile_ids(
    tiles_tree,
    selected_tile_ids: &HashSet<TileId>,
    primary_tile_id: Option<TileId>,
) -> Vec<TileId>
```

Returns the subset of `selected_tile_ids` that are Pane tiles, in tree
traversal order, with the primary tile moved to the front.

**Use**: provides stable, deterministic ordering for group operations.

### 12.2 `group_selected_tiles`

```
group_selected_tiles(
    tiles_tree,
    selected_tile_ids: &HashSet<TileId>,
    primary_tile_id: Option<TileId>,
) -> Option<(Vec<TileId>, TileId)>
```

Groups the selected pane tiles into a new Tabs container:

1. Requires at least 2 selected Pane tiles; returns `None` if fewer.
2. All selected tiles must be Pane tiles; returns `None` if any is a container.
3. Detaches each selected tile from its current parent via
   `detach_tile_for_reparent` (which calls `normalize_parent_after_child_removal`
   on each parent).
4. Creates a new Tabs container with the selected tiles.
5. Places the new container: if a root exists, wraps root + new group in a
   horizontal split; otherwise, the new group becomes root.
6. Activates the primary tile within the new group.
7. Returns `(ordered_tile_ids, primary_tile_id)`.

**Invariant**: `group_selected_tiles` must not be called if any selected tile
is a container. All structural operations are reversible by undo of the emitted
intents.

---

## 13. `toggle_tile_view`

Toggles the node pane presence:

- If any Node panes exist in the tree: removes all Node panes and releases
  their runtime (webview backpressure cleanup).
- If no Node panes exist: opens a node pane for `preferred_detail_node`. If the
  opened pane uses a composited runtime, ensures a webview is allocated via
  `webview_backpressure::ensure_webview_for_node`.

---

## 14. Acceptance Criteria

| Criterion | Verification |
|---|---|
| `open_or_focus_graph_pane` focuses existing graph, no new pane | Test: tree has graph_a → call → count unchanged, active == graph_a |
| `open_or_focus_graph_pane` inserts new graph tab | Test: tree has graph_a + node → insert graph_b → count = 2, active == graph_b |
| `SplitHorizontal` creates horizontal linear with new pane | Test: single graph pane → split with graph_b → root is Linear with 2 children |
| `open_or_focus_node_pane` in `SplitHorizontal` wraps leaf root before split | Test: bare leaf root → split → root is Linear; each child is Tabs container |
| `cycle_focus_region` rotates deterministically through present regions | Test: Graph+Node+Tool → cycle: graph→node→tool→graph |
| `cycle_focus_region` skips absent regions | Test: Graph+Tool (no Node) → cycle: graph→tool→graph |
| `ensure_active_tile` is no-op when active tile exists | Test: tree with active tile → `ensure_active_tile` returns false |
| `ensure_active_tile` recovers after active node tile removed | Test: activate node; remove it → `ensure_active_tile` activates graph |
| `normalize_parent_after_child_removal` collapses empty container | Test: container with 1 child; remove child → container removed from tree |
| `normalize_parent_after_child_removal` promotes sole remaining child | Test: container with 2 children; remove one → container replaced by remaining child |
| `group_selected_tiles` creates Tabs group with primary tile first | Test: 3 selected panes → group → new Tabs container, primary pane active |
| `group_selected_tiles` returns None for fewer than 2 panes | Test: 1 selected pane → returns None |
| `detach_tile_for_reparent` normalizes vacated parent | Test: detach from 2-child container → parent normalized (1 child elevated) |
| `active_graph_view_id` returns active graph view | Test: active graph tile → returns its `GraphViewId` |
