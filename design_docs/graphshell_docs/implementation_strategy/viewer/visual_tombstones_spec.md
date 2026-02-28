# Visual Tombstones (Ghost Nodes) — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Graph UX polish (non-critical path)

**Related**:

- `VIEWER.md`
- `viewer_presentation_and_fallback_spec.md`
- `2026-02-26_visual_tombstones_plan.md`
- `../canvas/CANVAS.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../subsystem_history/history_timeline_and_temporal_navigation_spec.md`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Tombstone data model** — `NodeState::Tombstone`, `TombstoneNodeData`, persistence.
2. **Tombstone rendering** — visual distinction, topology preservation.
3. **Restoration** — restoring a tombstoned node to active state.
4. **Garbage collection and retention policy** — expiry, threshold, user controls.
5. **Toggle and visibility** — per-view and global tombstone display settings.

---

## 2. Tombstone Data Model Contract

### 2.1 NodeState Extension

```
NodeState =
  | Active
  | Warm
  | Cold
  | Tombstone    -- deleted node with preserved structure
```

`Tombstone` is a lifecycle state, not a presentation mode. A tombstoned node is structurally present in the graph (retained in the data model) but marked as deleted.

### 2.2 TombstoneNodeData

```
TombstoneNodeData {
    id: NodeKey,
    position: Vec2,                          -- preserved spatial anchor
    title: Option<String>,                   -- optional memo label
    edges: Vec<(NodeKey, RelationshipKind)>, -- preserved topology
    deleted_at: DateTime,                    -- timestamp for GC
}
```

**Invariant**: A node transitions to `Tombstone` state only via an explicit `DeleteNode` intent with tombstone semantics. Silent removal (e.g., pane close) must not create tombstones.

### 2.3 Persistence

Tombstones are persisted in the graph persistence layer alongside active nodes. Queries **default to filtering out** `Tombstone` state. Explicit inclusion requires: `graph.nodes_with_state(NodeState::Tombstone)` or a `include_tombstones: true` query flag.

**Invariant**: Tombstone edges are preserved in the tombstone payload. The graph topology (live node edges) must not retain outgoing edges to `Tombstone` nodes unless tombstone display is active. When tombstone display is off, tombstoned nodes and their edges are invisible in graph queries and rendering.

---

## 3. Tombstone Rendering Contract

### 3.1 Visual Style

When tombstone display is active (`CanvasRegistry.tombstones_visible = true`):

- Tombstoned nodes render as **ghost nodes**: reduced opacity (0.25–0.35), dashed or dotted node border, distinct background fill (muted/greyed palette relative to active node theme).
- Tombstone label shows `title` if present, otherwise the original node URL truncated, with a strikethrough or ~~deleted~~ indicator.
- Ghost nodes do **not** show badges (badge system is for live nodes only), with the exception of a `Deleted` visual indicator in the node's primary slot.
- Ghost edges connecting two live nodes (where the path goes through a tombstone) are rendered as dashed lines.

### 3.2 Physics Behavior

- Tombstoned nodes are **excluded from physics simulation** by default. They occupy their preserved `position` but do not attract or repel live nodes.
- An optional `tombstone_gravity: bool` setting (default off) may include tombstones as passive attractors with reduced weight. This is off by default to avoid topology corruption from ghost-influenced layouts.

### 3.3 Interaction on Ghost Nodes

| Interaction | Behavior |
|-------------|----------|
| Click | Select ghost node; show restore affordance in context menu |
| Right-click | Context menu: "Restore Node", "Delete Permanently", "View History" |
| Hover | Tooltip shows original URL and deletion timestamp |
| Drag | Ghost nodes are not draggable unless explicitly enabled per `CanvasRegistry` setting |

---

## 4. Restoration Contract

**Restore action**: "Restore Node" from context menu or command palette.

1. Emit `RestoreNode { key }` intent.
2. Reducer transitions `NodeState::Tombstone → Cold` (not immediately `Active`; lifecycle reactivation follows normal cold-start path).
3. Node `position` from `TombstoneNodeData` is used as the initial position for the restored node.
4. Edges from `TombstoneNodeData` are re-evaluated: edges to live nodes are restored; edges to other tombstones remain tombstoned until those nodes are also restored.
5. The restoration is undoable via the history system.

**Invariant**: Restoration must not overwrite a different node that was created at the same `NodeKey` since the deletion. If key conflict exists, restoration is blocked with an explicit error.

---

## 5. Garbage Collection and Retention Policy

### 5.1 GC Trigger Conditions

Tombstones are eligible for permanent deletion (GC) when:

- Age exceeds the configured retention threshold (default: 30 days from `deleted_at`).
- Manual GC is triggered by the user ("Purge deleted nodes" action).
- The workspace is explicitly exported without tombstones (`include_tombstones: false` in export settings).

### 5.2 GC Behavior

GC permanently removes `TombstoneNodeData` from the persistence layer. It is **not reversible** after GC completes (unless a snapshot backup exists). The user is warned before manual GC if tombstones are present.

### 5.3 Retention Threshold Configuration

The retention threshold is configurable in settings (`tombstone_retention_days: u32`; default 30). Setting to `0` disables automatic GC (tombstones are retained indefinitely until manual GC).

---

## 6. Toggle and Visibility Contract

### 6.1 Per-View Toggle

Graph View has a "Show deleted nodes" toggle (keyboard shortcut or graph view toolbar). This controls `CanvasRegistry.tombstones_visible` for the active pane only.

When toggled off: tombstoned nodes and ghost edges are removed from the render pass. They remain in the data model.

### 6.2 Global Setting

`AppPreferences.tombstones_default_visible: bool` (default false) — controls the initial visibility for new and restored views. Per-view toggle overrides the global default for the lifetime of the view.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| `DeleteNode` with tombstone flag creates `NodeState::Tombstone` | Test: emit `DeleteNode { tombstone: true }` → node state is `Tombstone` |
| Tombstone queries excluded by default | Test: `graph.nodes()` → no tombstone nodes returned |
| Ghost node renders at reduced opacity and distinct style | Test: tombstone display on → node rendered with opacity ≤ 0.35 and dashed border |
| Ghost nodes excluded from physics | Test: tombstone in physics step → no force vectors applied to/from tombstone |
| Restore transitions to `Cold` | Test: `RestoreNode` → node state is `Cold`, not `Active` |
| Restored node uses preserved position | Test: restore → node position matches `TombstoneNodeData.position` |
| Restore is undoable | Test: restore → undo → node state returns to `Tombstone` |
| GC removes tombstone from persistence | Test: trigger GC → tombstone not present in reloaded graph |
| Per-view toggle does not affect sibling panes | Test: toggle in pane A → pane B tombstone visibility unchanged |
| Tombstone with no connected live nodes renders in isolation | Test: tombstone with all edges to other tombstones → renders as isolated ghost |
