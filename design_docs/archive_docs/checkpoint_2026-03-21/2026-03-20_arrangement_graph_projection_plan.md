# Arrangement Graph Projection Plan

**Date**: 2026-03-20
**Revised**: 2026-03-21
**Status**: Implementation complete — all phases shipped (2026-03-21)
**Priority**: Architecture — foundational

**Related**:

- `WORKBENCH.md` — workbench owns arrangement interaction/session mutation truth
- `graph_first_frame_semantics_spec.md` — Frame as graph-first organizational object; handle model
- `frame_persistence_format_spec.md` — FrameSnapshot bundle shape and restore contract
- `../canvas/2026-03-14_graph_relation_families.md` — ArrangementRelation family; FamilyPhysicsPolicy
- `../canvas/multi_view_pane_spec.md` — hosted surface contract; GraphViewId identity
- `workbench_frame_tile_interaction_spec.md` — arrangement interaction contracts; §4.2 routing priority
- `../navigator/navigator_interaction_contract.md` — Navigator projection over relation families
- `../PLANNING_REGISTER.md` — execution control-plane
- `../../TERMINOLOGY.md` — Frame, TileGroup, HostedSurface, ArrangementRelation, Graphlet

---

## 1. Problem

The current architecture treats the tile tree as the **primary truth** for
workbench arrangement: splits, tab groups, frame membership, tab order, and
active-tab markers all live in `egui_tiles::Tree<TileKind>` and survive only
as a `FrameSnapshot`. This creates four concrete problems:

1. **Navigator and workbench can diverge.** Navigator projects tile-tree
   snapshots; workbench projects live `egui_tiles` state. Any drift between
   them produces ordering inconsistencies that are undetectable at the contract
   level.

2. **Arrangement state is not graph-backed.** `ArrangementRelation` is defined
   as a forthcoming `EdgeKind` family in `graph_relation_families.md §2.4`
   (sub-kinds: `frame-member`, `tile-group`, `split-pair`), but no arrangement
   state is read back from the graph to drive the tile tree. Frame membership,
   tab ordering, and active-tab markers exist only in the tile tree.

3. **Workbench invocation is ad hoc.** Opening a node in a split, adding it to
   a tab group, or promoting an overlay are all tile-tree-direct mutations with
   no graph-backed audit trail and no clear routing authority.

4. **Tile grouping is not graphlet-causative.** The edges that connect nodes in
   the graph — hyperlinks, history traversal, explicit user groupings, frame
   membership — do not drive which nodes appear together in the workbench tile
   tree. A user who filters edges to isolate a connected component (graphlet)
   and then opens a member node gets no automatic grouping of that graphlet's
   other members. The graph's structure and the workbench's structure are
   semantically unrelated.

The insight from `graph_relation_families.md §1.1`: if arrangement belongs to
one relation family, the Navigator, the workbench, the physics engine, and the
persistence layer can all share one model rather than maintaining parallel
structures.

---

## 2. Target Model

**Authority split:**

- **Graph carries membership truth.** The canonical graph (nodes, edges,
  `NodeLifecycle`) is the authority for *which* nodes belong together and
  whether each is currently presented. `UserGrouped` and `FrameMember` edges
  are the durable record; lifecycle state is the presence record. There is no
  separate in-memory arrangement graph.
- **Workspace state carries presentation truth.** *How* those nodes are
  arranged — split geometry, tab order, active-tab identity — lives in
  session-local workbench objects (`SplitContainer`, `SetGroupActiveMember`
  history) and is captured in the `FrameSnapshot` for workspace restore. This
  state is not in the graph edge layer.

The workbench tile tree is a **projection** of both layers. The Navigator reads
the same sources. They agree structurally, not incidentally.

### 2.1 Node Lifecycle

Every node carries a lifecycle state:

| State | Meaning | Tile presence |
|-------|---------|---------------|
| `Active` | Live renderer (WebView / viewer) running | Has a tile; renderer is live |
| `Warm` | Tile exists; renderer pending or suspended | Has a tile |
| `Cold` | In graph with edges intact; no tile | No tile; visible in omnibar and Navigator with ○ badge |

**Dismiss gesture** (`DismissTile`): close a tile without removing any edges.
The node's lifecycle moves to `Cold`. The node stays in every graphlet it
belonged to; its edges are entirely intact.

**Delete gesture**: remove the node from the graph. Separate action; more
permanent; edges are retracted; the node leaves all graphlets.

These two gestures must never be aliased to the same UI action.

### 2.2 Graphlet as Tile Group Roster

A **graphlet** is the set of nodes reachable from a given node by traversing
edges that pass the active lens filter — the weakly connected component
containing that node under the filtered edge set.

The graphlet is the **primary arrangement unit**. The tile tree shows the
**warm/active slice** of each graphlet. The omnibar shows the **full roster**
(warm and cold alike).

```
Graphlet G (under active filter):
  N  — Active (has live tile)
  M1 — Cold   (in graphlet, no tile)
  M2 — Cold   (in graphlet, no tile)
  M3 — Warm   (has live tile)

Tile tree:  Container::Tabs { N tile | M3 tile }
Omnibar:    N ●  M3 ●  |  M1 ○  M2 ○
Navigator:  graphlet row → N ●  M3 ●  M1 ○  M2 ○
```

When the filter changes, the graphlet boundary changes. Nodes may join or leave
a visible graphlet. Their lifecycle state and edge connections are not affected
by filter changes — only the grouping is recalculated.

### 2.3 Edge Families That Form Graphlets

Any edge that passes the active lens filter contributes to graphlet
connectivity:

| Family | Durability | Graphlet character |
|--------|-----------|-------------------|
| `UserGrouped` | Durable | Explicit user-created connection; filter-independent |
| `ArrangementRelation(FrameMember)` | Durable | Named frame membership; filter-independent |
| `Hyperlink` | Filter-derived | Navigated-between pages |
| `History` | Filter-derived | Browsing session traversal chain |
| `ContainmentRelation` | Filter-derived | Same domain / URL path |
| `AgentDerived` | Session (decay) | Agent-inferred similarity |

**Durable graphlets**: formed by `UserGrouped` and `FrameMember` edges.
Persist across sessions regardless of filter. These are the graphlets the user
explicitly creates or that a named frame creates.

**Circumstantial graphlets**: formed by filter-included `Hyperlink`, `History`,
or `ContainmentRelation` edges. Exist when those edges pass the filter; change
as the filter changes.

**Long-chain graphlets**: `History` traversal edges chain nodes into browsing
session paths. A History-inclusive filter makes the whole traversal chain one
graphlet.

### 2.4 Workbench-Local Session Objects

For implementation purposes the workbench tracks these session-local objects.
They are not graph nodes and have no `verso://` addresses.

| Object | Identifier | Purpose |
|--------|-----------|---------|
| `HostedSurface` | `HostedSurfaceId` | Binds a warm/active node to its live renderer; `presents_node: NodeKey` is a struct field, not an edge |
| `SplitContainer` | `SplitContainerId` | Carries split axis, share proportions, and ordered child references; relates tile groups spatially |

**SplitContainer persistence:** `SplitContainer` identity and parent/child
structure are captured in the `FrameSnapshot` for named frames (debounced
autosave). The snapshot records the ordered sequence of split children and the
committed share proportions. For unnamed session contexts (no `FrameMember`
edges to a named frame anchor), `SplitContainer` state is purely ephemeral and
is not persisted. Split geometry is **not** in the graph edge layer.

**Ephemeral session state** — does not enter arrangement truth:

- Focus and hover state
- Drag target and drag preview
- Split resize geometry during an active drag (committed on drag-end)
- Ephemeral panes (QuarterPane / HalfPane / FullPane) before enrollment into
  the arrangement

### 2.5 Active-Tab Persistence

`SetGroupActiveMember` records which node is the active tab within a tile
group. Persistence rules:

- **Named frames**: the active-tab node identity is written into the
  `FrameSnapshot` on each debounced autosave. On workspace restore, the saved
  active-tab node is made active (if it is still warm/active); otherwise the
  most-recently-activated warm member is used.
- **Unnamed session contexts**: active-tab is session-only. It is not
  persisted; on restart the default ordering (most-recently-activated) applies.

`SetGroupActiveMember` is independent of keyboard/accessibility focus routing.
Moving focus to the Navigator does not change the active-tab marker.

### 2.6 Multi-Presence

A node may have `FrameMember` edges to **multiple** named frames
simultaneously. In a live session:

- The node's tile resides in the most-recently-active frame context for that
  node (last frame in which the node was warm or active).
- In all other frames, the node appears as cold (○ badge) in the omnibar and
  Navigator roster for that frame.
- `RemoveFromGraphlet` in one frame retracts only the `FrameMember` edge to
  that frame's anchor; membership in other frames is unaffected.

A node has exactly one `NodeLifecycle` state at any time. Being a member of
multiple frames does not create multiple lifecycle states.

---

## 3. Persistence

### 3.1 Graph edges are the durable format

Durable edges (`UserGrouped`, `ArrangementRelation(FrameMember)`) persist
naturally in the graph store (redb WAL). Graphlet membership for named frames
survives restarts because `FrameMember` edges survive restarts.

`NodeLifecycle::Cold` is stored on the node. Cold graphlet members persist
across sessions. On startup, the graph's edges reconstruct graphlet membership
directly; no separate bootstrap step is required.

### 3.2 FrameSnapshot as workspace-restore format

The `FrameSnapshot` bundle (per `frame_persistence_format_spec.md`) captures
**presentation state at save time**: which nodes were warm or active, the
active-tab identity for each tile group, and split structure/proportions. Its
role is **workspace restore** — re-opening the saved tiles and arrangement
shape when a frame is loaded — not carrying membership truth. Graph edges carry
that truth.

The bundle and the graph are mutually consistent for named frames. If they
diverge (e.g. a graph edge was added without a snapshot update), graph edges
take precedence for membership; the snapshot's presentation shape is applied on
top.

Named frames autosave (debounced, 1 s quiescence) to keep `FrameSnapshot`
current for workspace restore. Unnamed session contexts (no `FrameMember` edges
to a named anchor) evaporate on close — their transient tiles are not
persisted.

Significant events trigger an immediate autosave write for named frames: frame
naming, node added to or removed from a named frame (edge asserted or
retracted).

### 3.3 Data portability

`FrameSnapshot` bundles reference nodes by stable UUID (per
`frame_persistence_format_spec.md §4.2`), making them portable across
instances. Graphlet membership for durable graphlets is carried by graph edges,
which are also UUID-stable.

### 3.4 Write authority

- **Durable `FrameMember` edges** for named frames → `GraphIntent::AssertArrangementRelation`
  / `RetractArrangementRelation`. These produce durable `ArrangementRelation`
  edges in the graph store.
- **UserGrouped edges** (e.g. from growing a graphlet via new tile) →
  `GraphIntent::CreateUserGroupedEdge`.
- **Lifecycle changes** → `GraphIntent::PromoteNodeToActive`,
  `DemoteNodeToWarm`, `DemoteNodeToCold`.
- **Active-tab identity** → `SetGroupActiveMember` (session state); for named
  frames, autosave captures the active-tab node in `FrameSnapshot`.
- **Split geometry** → `WorkbenchIntent` updating `SplitContainer.shares` and
  child ordering (session state); autosave captures the committed split
  structure in `FrameSnapshot` for named frames.

---

## 4. Workbench as Projection

The workbench tile tree is built by:

1. Reading the active lens filter to determine the visible edge set.
2. Computing graphlets: weakly connected components over the filtered graph
   (`graph.weakly_connected_components()` applied to the filter-projected
   edge set).
3. For each graphlet with at least one warm/active member: ensure the tile
   tree contains a `Container::Tabs` holding those warm members.
4. For each warm node with no graphlet peers: ensure a single tile (no tab
   container wrapper).
5. Applying saved `SplitContainer` geometry to arrange tile groups spatially.
6. Overlaying ephemeral session state (focus ring, drag preview, hover).

**Filter stability for durable groups**: filter changes only add or remove
nodes at the boundary of a durable graphlet — they do not split or merge
existing durable groups. A durable graphlet (formed by `UserGrouped` or
`FrameMember` edges) remains intact regardless of filter state. A filter change
may cause a circumstantially connected node to join or leave the durable group's
graphlet boundary; nodes that leave become standalone graphlets or join other
reachable components, but their lifecycle is not changed and their tiles (if
warm) remain live.

**Bidirectional binding**: changes flow in both directions.

- **Graph → workbench**: edge added or removed → graphlet recomputed →
  reconciler updates tile tree. Filter change → graphlets recomputed →
  reconciler updates tile tree. Node promoted/demoted → reconciler adds or
  removes tile.
- **Workbench → graph**: open tile → `PromoteNodeToActive`; dismiss tile →
  `DemoteNodeToCold`; grow graphlet by opening new tile → edge created;
  remove from graphlet → edges retracted.

**Invariant**: the tile tree is never semantic arrangement truth. If it drifts
from the expected projection, the graph (edges + lifecycle) wins.

---

## 5. Workbench Invocation via Graph

### 5.1 Opening a node (`OpenNode`)

1. Compute N's graphlet G under the active filter.
2. Determine the destination tile group:
   a. If a tile group for G already exists (any warm member present) → route N
      into that group.
   b. If no tile group exists → create a new `Container::Tabs` for G.
3. `PromoteNodeToActive(N)` — N gets a live tile.
4. Graphlet peers (M1, M2, …) remain cold. They appear in the omnibar roster
   with ○ badges but do not get tiles automatically.
5. If G has `FrameMember` edges to a named frame anchor → frame routing applies
   per `workbench_frame_tile_interaction_spec.md §4.2`: prefer last-active
   frame, then deterministic fallback.

### 5.2 Growing a graphlet: open new tile in group

When the user opens a new tile within an existing tile group:

1. A new node is created (or an existing node is chosen from the omnibar).
2. A `UserGrouped` edge is created from the new node to any existing graphlet
   member (or to the frame anchor if one exists). This makes the new node a
   **durable** graphlet member regardless of the active filter.
3. `PromoteNodeToActive(new_node)` — the new node gets a live tile in the same
   tab group.

This is the mechanism by which explicit tile-opening grows the graphlet. The
edge ensures that even if the filter later changes to exclude other
circumstantial edges, the explicitly added node stays in the graphlet.

### 5.3 Dismissing a tile (`DismissTile`)

1. `DemoteNodeToCold(N)` — N's lifecycle becomes `Cold`.
2. Close N's `HostedSurface` (tile closed; renderer released).
3. All edges connecting N to its graphlet remain intact.
4. N remains in the omnibar roster with ○ badge.
5. If all members of the graphlet become cold: the tile group is removed from
   the tile tree. The graphlet is intact in the graph and will reappear in the
   tile tree when any member is next activated.

`DismissTile` is the standard "close a tile" gesture. It is not destructive.

### 5.4 Removing from graphlet (`RemoveFromGraphlet`)

`RemoveFromGraphlet` retracts only the **durable arrangement edges** connecting
N to graphlet G — `UserGrouped` and `ArrangementRelation(FrameMember)` edges.
Circumstantial edges (Hyperlink, History, ContainmentRelation) are **not**
retracted; semantic relationships the user navigated are not erased.

1. Retract all `UserGrouped` edges between N and any member of G.
2. If G is a named frame: emit `GraphIntent::RetractArrangementRelation` for
   the `FrameMember` edges connecting N to G's anchor.
3. N is no longer a durable graphlet member and does not appear in the omnibar
   roster or Navigator row for G.
4. N's lifecycle is unchanged. If N had a live tile, it is now a standalone
   tile (or part of another graphlet if N retains durable edges to other nodes).
5. N may still appear in circumstantial graphlets if Hyperlink/History edges to
   G's members pass the active filter — this is correct behavior, not a bug.

`RemoveFromGraphlet` is the "leave this arranged cohort" gesture. It is more
permanent than dismiss (which only changes lifecycle), but it does not erase
semantic history. Calling `RemoveFromGraphlet` on a node that is connected to
G only via circumstantial edges has no effect on the graph (no durable edges
to retract).

### 5.5 Activating a cold node

**From omnibar**: The omnibar lists all graphlet members (warm ● and cold ○).
Selecting a cold entry triggers `OpenNode(N)` with the current graphlet's tile
group as the routing target. N gets a tile in the existing tab group.

**From canvas**: Multiselect cold nodes on the graph canvas → "Warm Select"
action → `OpenNode` for each selected node, routed into their respective
graphlets' tile groups.

### 5.6 Other invocation actions

| Action | Arrangement effect |
|--------|-------------------|
| `OpenInSplit` | Creates a `SplitContainer`; places existing tile group and new tile group as split children |
| `SetGroupActiveMember` | Updates the active-tab marker for a tile group (session state; debounced autosave for named frames) |
| `ActivateSurface` | Routes keyboard/accessibility focus via Focus Subsystem; independent from `SetGroupActiveMember` |
| `EnrollOverlayInArrangement` | Converts an ephemeral pane into a warm graphlet member; creates `UserGrouped` or `FrameMember` edge |
| `CommitSplitShares` | Writes final split share values on drag-end; triggers debounced autosave |

---

## 6. Cold/Warm Roster and Omnibar

### 6.1 Roster definition

A graphlet's **roster** = all nodes connected to any warm member via
filter-visible edges. The roster has two slices:

- **Warm slice**: nodes with `NodeLifecycle::Active` or `Warm` — have live
  tiles in the tile tree.
- **Cold slice**: nodes with `NodeLifecycle::Cold` — in the graphlet, no tile.

The tile tree shows the warm slice. The omnibar and Navigator show the full
roster.

### 6.2 Omnibar

The omnibar within a workbench context shows the active tile group's graphlet
roster:

- Warm members: ● indicator; click to focus their tile.
- Cold members: ○ indicator; click to activate (opens tile in same tab group).
- Ordered: warm members first (by last-activation order), then cold members
  (by last-activation recency).

The omnibar is the primary discovery surface for cold graphlet members. It
answers "what else belongs here that I'm not looking at right now?"

### 6.3 Cold node display in Navigator

Cold graphlet members appear in the Navigator with a `cold` residency badge
(○). They are not hidden. This is a deliberate update to
`navigator_interaction_contract.md §2.1`, which previously suppressed nodes
without a live tile representation.

Under the graphlet model, graph membership (edges) is sufficient for Navigator
projection. The tile tree is not the authority.

- Single-click cold node: select the node in the graph.
- Double-click cold node: `OpenNode(N)` — activates the node, opens a tile in
  the graphlet's tab group.
- Right-click → "Remove from graphlet": `RemoveFromGraphlet(N, G)`.

---

## 7. Navigator Faithfulness Contract

The Navigator reads graphlet membership (edge connectivity + filter) and
lifecycle state directly from the graph — the same source the workbench
projection reads. Agreement is structural, not incidental.

| Navigator concern | Source |
|-------------------|--------|
| Which nodes are in a tile group | Graphlet connectivity (edges under active filter) |
| Warm / cold status | `NodeLifecycle` field on each node |
| Active tab within group | `SetGroupActiveMember` history / last-activation order |
| Frame membership | `ArrangementRelation(FrameMember)` edges |
| Split structure | `SplitContainer` session layout preferences |

### 7.1 Example: five-node graphlet with filter change

```
Graph state:
  N  —[UserGrouped]→  M1
  N  —[UserGrouped]→  M2
  N  —[Hyperlink]→    M3     (Hyperlink visible under current filter)
  N  —[FrameMember]→  FrameAnchor "Research"

Lifecycle:
  N:  Active    M1: Cold    M2: Cold    M3: Warm

Graphlet G = { N, M1, M2, M3 }

Tile tree:   Container::Tabs { N tile | M3 tile }
Omnibar:     N ●  M3 ●  |  M1 ○  M2 ○
Navigator:   Frame "Research" → G: N ●  M3 ●  M1 ○  M2 ○
```

User removes Hyperlink edges from the active filter:

```
Graphlet G' = { N, M1, M2 }    (M3 is now its own singleton graphlet)

Tile tree:   Container::Tabs { N tile }    (M3 tile still live, now standalone)
Omnibar:     N ●  |  M1 ○  M2 ○
Navigator:   Frame "Research" → G': N ●  M1 ○  M2 ○
             (M3 appears in its own row as a singleton)
```

M3's tile is not destroyed by the filter change — lifecycle is not changed by
filter changes. M3's `NodeLifecycle` remains `Warm`. M3 simply belongs to a
different (singleton) graphlet now.

---

## 8. Reconciliation Layer

The reconciler runs when:

- A node's lifecycle changes (`Active` / `Warm` / `Cold`)
- An edge is added or removed (graphlet boundary change)
- The active filter changes (graphlet recomputation)
- The tile tree drifts from its expected projection state

### 8.1 Reconciler algorithm

**Scoping**: the reconciler is scoped to the affected graphlet(s), not the
full graph. An edge change between nodes N and M only triggers recomputation
for the connected component containing N and M. A lifecycle change on node N
only triggers recomputation for N's graphlet. A filter change triggers full
recomputation (all graphlets are potentially affected). This keeps
per-interaction cost O(component size), not O(V+E) for the whole graph.

Steps for each affected graphlet:

1. Compute graphlet membership under the active filter (weakly connected
   components restricted to the affected component set).
2. For each graphlet with ≥ 1 warm/active member:
   a. Ensure a `Container::Tabs` exists for those warm members.
   b. Ensure tab order reflects `SetGroupActiveMember` history or
      most-recent-activation order.
3. For each warm/active node with no warm graphlet peers: single tile, no tab
   container.
4. Apply `SplitContainer` geometry from saved layout preferences.
5. Remove empty containers.

### 8.2 Lifecycle preservation

The reconciler never changes `NodeLifecycle`. Lifecycle changes are driven
exclusively by explicit user actions (`OpenNode`, `DismissTile`,
`RemoveFromGraphlet`) and the `PromoteNodeToActive` / `DemoteNodeToCold`
intent path. The reconciler only mutates the tile tree.

### 8.3 Ephemeral bypass

Drag preview, split preview, and resize drag bypass the reconciler for their
duration. On confirm or cancel, the reconciler resumes.

### 8.4 Empty container cleanup

- A `Container::Tabs` whose entire warm slice has been dismissed is removed
  from the tile tree. The graphlet remains intact in the graph.
- A `SplitContainer` reduced to one child is collapsed: the sole child is
  promoted to the parent container.
- A `SplitContainer` with zero children is removed unconditionally.

Cleanup runs at the end of each reconcile pass. No empty container rows appear
in the Navigator.

### 8.5 Cycle prevention

Arrangement edge writes that would create a cycle (e.g. a `FrameMember` loop)
are rejected as a precondition check before the write. Not repaired after.

### 8.6 Split resize commit

`SplitContainer.shares` are not updated during an active resize drag. Live
resize geometry is ephemeral session state for the drag's duration — the
reconciler is bypassed and the tile tree holds the live geometry. On drag-end
(mouse release), `CommitSplitShares` writes the final share values and triggers
debounced autosave for named frames.

---

## 9. Diagnostics Channels

| Channel | Severity | Emitted when |
|---------|----------|-------------|
| `arrangement:graphlet_computed` | Info | Filter change triggers graphlet recomputation |
| `arrangement:membership_changed` | Info | Node joins or leaves a graphlet (edge add/remove or filter change) |
| `arrangement:lifecycle_transition` | Info | Node lifecycle changes (Active / Warm / Cold) |
| `arrangement:mutation_failure` | Error | Arrangement edge write fails (graph intent rejected, validation error) |
| `arrangement:cycle_detected` | Error | Arrangement mutation would create a cycle; write rejected |
| `arrangement:reconciliation_drift` | Warn | Tile tree shape differs from expected projection at reconcile time |
| `arrangement:autosave_failure` | Error | Autosave write of FrameSnapshot to redb fails |
| `arrangement:autosave_write` | Info | Autosave successfully refreshed a named frame's FrameSnapshot |
| `arrangement:bootstrap_populated` | Info | Graphlet membership populated from graph edges at startup |

---

## 10. Migration Plan

Phased. Each phase is independently shippable.

### Phase 1 — Graphlet computation ✓ Complete (2026-03-21)

- Implement `compute_graphlets(graph, filter) -> Vec<Vec<NodeKey>>` using
  `graph.weakly_connected_components()` applied to the filter-projected edge
  set. This is a pure query; no graph mutations.
- Wire: filter change signal → graphlet recomputation →
  `WorkbenchProjectionRefreshRequested`.
- No workbench behavior change yet.
- Gate: `compute_graphlets` returns correct components for test graphs with
  mixed edge families and filter configurations.

### Phase 2 — Lifecycle-driven tile tree projection ✓ Complete (2026-03-21)

- Reconciler reads graphlets + lifecycle → drives tile tree.
- `DismissTile` → `DemoteNodeToCold` + close tile; edges preserved.
- `OpenNode` → compute graphlet, route to correct tab group.
- Gate: tile tree reflects exactly the warm/active members of each graphlet.
- Gate: `DismissTile` does not remove any edges from the graph.

### Phase 3 — Omnibar graphlet roster ✓ Complete (2026-03-21)

- Omnibar lists full graphlet roster (warm ● + cold ○) for the active tile
  group.
- `OmnibarMatch::ColdGraphletMember(NodeKey)` variant added; `TabsLocal`
  empty-query branch extended to append cold peers of all warm nodes in the
  tree; `apply_omnibar_match` routes via `SelectNode` + `ToolbarOpenMode::Tab`.
- Cold nodes activatable from omnibar.
- Gate met: cold members visible in omnibar; activating opens tile via graphlet
  routing in same tab group.

### Phase 4 — Navigator cold-node display ✓ Complete (2026-03-21)

- Update Navigator to show cold graphlet members with ○ badge.
- `WorkbenchNavigatorMember.is_cold` field already carried `is_cold`; Navigator
  render loop updated to prepend "○ " prefix for cold members.
- `arrangement_navigator_groups` already extended with cold `UserGrouped` peers.
- `SidebarAction::ActivateNode` already calls `open_node_with_graphlet_routing`
  for cold nodes — no change needed.
- Gate met: cold nodes appear in Navigator with ○ badge; double-click opens
  tile in graphlet tab group.

### Phase 5 — Graphlet growth via new tile ✓ Complete (2026-03-21)

- "Open new tile in group" creates new node + `UserGrouped` edge → grows
  graphlet durably.
- `CreateNodeNearCenterAndOpen { mode: Tab }` handler captures the active
  primary selection **before** calling `create_new_node_near_center()` (which
  overwrites the selection with the new node's key), then creates the
  `UserGrouped` edge and enqueues `ReconcileGraphletTiles`.
- Gate met: new node appears as permanent graphlet member; new-tile edge
  survives filter changes that would exclude other circumstantial edges.

### Phase 6 — Canvas multiselect warm-select ✓ Complete (2026-03-21)

- Multiselect cold nodes on graph canvas → "Warm Select" activates them into
  their graphlets' tab groups.
- `ACTION_GRAPH_SELECTION_WARM_SELECT` dispatches `OpenNodeInPane` for each
  cold selected node; cold nodes are opened via graphlet routing.
- Gate met: selected cold nodes get tiles in correct tab groups.

### Phase 7 — Remove tile-tree-as-truth code paths ✓ Substantially complete (2026-03-21)

Audit (2026-03-21) confirmed no routing or invocation callsites remain that use
the tile tree as arrangement authority. Findings:

- **`tile_grouping.rs` + `tile_post_render.rs`** — drag-detection that fires
  `GraphIntent::CreateUserGroupedEdge` on tab-drop. This is the correct
  **workbench → graph** direction: it *writes to* arrangement truth in response
  to a gesture. Intentionally kept; not a tile-tree-as-truth read.
- **`toolbar_omnibar.rs` (`tab_node_keys_in_tree`)** — lists open node-pane
  keys for omnibar tab suggestions. Display/search callsite only; does not
  influence arrangement routing or invocation decisions. Intentionally kept.
- **`FrameTabSemantics`** — was never implemented in code. No removal needed.

`FrameSnapshot` is workspace-restore format only; graph edges (`UserGrouped`,
`FrameMember`) and `NodeLifecycle` are arrangement membership truth. This
invariant holds throughout the codebase.

**Intentional tile-tree reads (not in scope for removal)**:

- Layout reads (split geometry, active-tile focus ring) — tile tree *is* the
  layout authority; these are correct by design.
- Display/search queries (omnibar tab listing) — read-only; not arrangement
  authority callsites.
- Drag-detection edge writes — write *to* graph from tile-tree events; correct
  direction.

Gate met: no direct tile-tree reads for arrangement ordering from routing or
invocation callsites.

---

## 11. Key Invariants

1. **Graph carries membership truth; workspace state carries presentation
   truth.** Graph edges (`UserGrouped`, `FrameMember`) and `NodeLifecycle` are
   the authority for which nodes belong together and whether each is presented.
   Split geometry, tab order, and active-tab identity are workspace-local
   presentation state captured in `FrameSnapshot` for named frames. Neither
   layer is reducible to the other.
2. **Graphlet membership is determined by edges + active filter.** Different
   filters → different graphlets. Lifecycle state is not changed by filter
   changes.
3. **Durable graphlets are not split or merged by filter changes.** A
   `UserGrouped` or `FrameMember` group survives filter changes intact. Filter
   changes only affect circumstantial boundary nodes.
4. **The tile tree shows the warm/active slice of each graphlet.** Cold members
   are in the omnibar and Navigator but not the tile tree.
5. **Dismiss → cold. Delete → remove.** `DismissTile` preserves edges and
   graphlet membership; delete retracts edges and removes the node. These
   gestures must never be aliased.
6. **`RemoveFromGraphlet` retracts only durable edges.** `UserGrouped` and
   `FrameMember` edges are retracted; circumstantial edges (Hyperlink, History)
   are not touched.
7. **Growing a graphlet via new tile creates a durable `UserGrouped` edge.**
   The new node's graphlet membership is filter-independent.
8. **Filter change recalculates graphlets but does not change lifecycle.** A
   node that exits a graphlet due to a filter change retains its
   `NodeLifecycle::Warm` or `Active`; its tile remains live.
9. **Navigator and tile tree agree because they read the same graph.** Agreement
   is structural, not incidental.
10. **`GraphViewId` remains graph-owned** even when hosted in a tile surface.
11. **`HostedSurface` is workbench-local.** No `verso://surface/` graph node;
    `HostedSurfaceId` is a session-only binding; `presents_node` is a struct
    field, not a graph edge.
12. **Closing a tile does not remove edges.** `DemoteNodeToCold` + tile close;
    edges intact; graphlet membership unchanged.
13. **Cycle detection is a precondition.** Arrangement edge writes that would
    create cycles are rejected before write, not repaired after.
14. **Split resize geometry is ephemeral during drag.** `SplitContainer.shares`
    committed on drag-end only. Reconciler bypassed during resize.
15. **`FrameSnapshot` is workspace-restore format, not membership truth.** Graph
    edges reconstruct durable membership at startup without a separate
    bootstrap. On conflict, graph edges win for membership; snapshot wins for
    presentation shape.
16. **Active-tab persistence is scoped to named frames.** `SetGroupActiveMember`
    is session-only for unnamed contexts; written to `FrameSnapshot` for named
    frames.
17. **Multi-presence is permitted.** A node may hold `FrameMember` edges to
    multiple named frames. Its tile lives in the most-recently-active frame
    context; other frames show it as cold.
18. **The omnibar is the primary cold-node discovery surface.** Cold graphlet
    members are always accessible via omnibar or Navigator; they are not hidden
    or lost.
19. **Reconciler cost is scoped to affected components.** Only the connected
    component(s) containing changed nodes are recomputed per edge/lifecycle
    event. Full recompute only on filter change.

---

## 12. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Graphlet computed from edges + filter | Test: add `Hyperlink` edge A→B; filter includes Hyperlink; A and B in same graphlet |
| Filter change updates graphlet | Test: remove Hyperlink from filter; A and B in separate graphlets; lifecycle unchanged |
| Warm members appear in tile tree | Test: A active, B cold; tile tree contains A tile, no B tile |
| Cold members appear in omnibar with ○ | Test: A active, B cold; omnibar shows A ● B ○ |
| Activating cold node from omnibar opens tile in same group | Test: click B ○ in omnibar; B tile opens in same `Container::Tabs` as A |
| Canvas multiselect warm-select | Test: select B, C cold on canvas; warm-select → B and C tiles open in graphlet tab group |
| `DismissTile` → cold, edges preserved | Test: close A tile; `NodeLifecycle::Cold`; A's edges intact; A in graphlet; A in omnibar with ○ |
| `DismissTile` does not affect graphlet peers | Test: close A tile; B tile and B lifecycle unchanged |
| All members cold → tile group removed from tile tree | Test: dismiss all tiles in graphlet; no `Container::Tabs` in tile tree; graphlet intact in graph |
| Cold member activated → tile group recreated | Test: dismiss all tiles, then activate one cold member; `Container::Tabs` appears in tile tree |
| Open new tile in group → `UserGrouped` edge created | Test: open new tile in tile group; new node has `UserGrouped` edge to an existing graphlet member |
| New-tile node is durable graphlet member | Test: open new tile; change filter to exclude other edge families; new node still in graphlet (UserGrouped edge survives filter) |
| `RemoveFromGraphlet` retracts edges | Test: remove N from graphlet; N's graphlet edges retracted; N absent from tab group, omnibar roster, and Navigator graphlet row |
| Named-frame `FrameMember` edges persist across restart | Test: create named frame (assert `FrameMember` edges); restart; edges restored from graph store |
| `FrameSnapshot` captures warm members for workspace restore | Test: named frame with A warm, B cold; autosave; restart; A re-opened as warm; B remains cold (NodeLifecycle::Cold) |
| Cold nodes appear in Navigator with ○ badge | Test: dismiss A tile; Navigator shows A with cold badge in graphlet row |
| Double-click cold Navigator node opens tile | Test: double-click cold A in Navigator; `OpenNode` fires; A tile opens in graphlet tab group |
| Filter change does not change lifecycle | Test: remove Hyperlink from filter; B exits graphlet; B's `NodeLifecycle` unchanged; B tile (if warm) still live |
| Single warm node = single tile, no tab container | Test: graphlet with exactly 1 warm node; tile tree has no `Container::Tabs` wrapper |
| Split shares committed on drag-end | Test: resize drag; reconciler bypassed mid-drag; mouse release fires `CommitSplitShares`; shares updated; debounced autosave triggered |
| Reconciler does not change lifecycle | Architecture invariant: reconciler only mutates tile tree structure; no `PromoteNodeToActive` or `DemoteNodeToCold` from reconciler |
| `SetGroupActiveMember` and focus routing are independent | Test: move keyboard focus to Navigator; active-tab marker in tile group unchanged |
| Cycle write rejected | Test: write `FrameMember` edge that would create cycle; write rejected; `arrangement:cycle_detected` emitted |
| `EnrollOverlayInArrangement` makes ephemeral pane a graphlet member | Test: promote ephemeral pane; `UserGrouped` or `FrameMember` edge asserted; node joins graphlet |

---

## 13. Test Coverage (as of 2026-03-21)

Automated tests covering the §12 acceptance criteria live in
`shell/desktop/tests/scenarios/grouping.rs` and
`shell/desktop/ui/workbench_sidebar.rs` (test module).

| Test | File | Criterion covered |
|------|------|-------------------|
| `create_user_grouped_edge_from_primary_selection_creates_grouped_edge` | `grouping.rs` | `UserGrouped` edge creation |
| `dismiss_tile_demotes_lifecycle_and_preserves_edges` | `grouping.rs` | `DismissTile` → cold, edges preserved |
| `dismissed_node_remains_in_durable_graphlet` | `grouping.rs` | Dismissed node remains graphlet peer |
| `open_node_with_graphlet_routing_joins_warm_peer_tab_container` | `grouping.rs` | Activating cold node routes into existing tab group |
| `cold_node_reactivated_joins_existing_tab_group` | `grouping.rs` | Cold member activated → joins existing tab group |
| `reconcile_graphlet_merges_tiles_from_different_tab_containers` | `grouping.rs` | `ReconcileGraphletTiles` merges separate containers |
| `remove_from_graphlet_action_retracts_durable_edges_only` | `grouping.rs` | `RemoveFromGraphlet` retracts only durable edges |
| `warm_select_action_dispatches_open_intent_for_cold_selected_nodes` | `grouping.rs` | Canvas multiselect warm-select |
| `new_tile_as_tab_creates_durable_graphlet_edge` | `grouping.rs` | New-tile-in-group creates `UserGrouped` edge (Phase 5) |
| `active_graphlet_roster_marks_cold_peers_as_cold` | `workbench_sidebar.rs` | Cold roster entry `is_cold = true` (Phase 3) |
| `arrangement_navigator_member_marks_dismissed_cold_peer_as_cold` | `workbench_sidebar.rs` | Navigator cold badge `is_cold = true` (Phase 4) |
