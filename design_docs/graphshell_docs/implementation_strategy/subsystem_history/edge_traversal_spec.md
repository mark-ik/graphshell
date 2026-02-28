# Edge Traversal — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: In progress (Stages 0–E complete)

**Related**:

- `SUBSYSTEM_HISTORY.md`
- `history_timeline_and_temporal_navigation_spec.md`
- `2026-02-20_edge_traversal_impl_plan.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../../TERMINOLOGY.md` — `Traversal`, `Edge Traversal History`, `EdgePayload`, `EdgeType`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Edge semantic model** — `EdgePayload`, `Traversal`, `NavigationTrigger`.
2. **Traversal recording** — append rules, trigger classification, WAL entry format.
3. **Edge visual presentation** — traversal-aware rendering.
4. **History Manager surface** — Timeline and Dissolved tabs, archive queries.
5. **Temporal navigation** — preview mode, scrubber contract (planned).

---

## 2. Edge Semantic Model Contract

### 2.1 EdgePayload

`EdgePayload` encodes both structural and temporal data for an edge. It replaces the deprecated `EdgeType`.

```
EdgePayload {
    kind: EdgeKind,
    traversals: Vec<Traversal>,
}

EdgeKind =
  | UserGrouped       -- explicit user-created connection
  | TraversalDerived  -- implicit; created by navigation event
```

**Invariant**: Display-only computations (dominant direction, stroke width) are derived from `EdgePayload` at render time. They must not be stored in `EdgePayload`.

### 2.2 Traversal Record

```
Traversal {
    timestamp: DateTime,
    trigger: NavigationTrigger,
    direction: TraversalDirection,   -- Forward | Backward
}

NavigationTrigger =
  | LinkClick
  | BackButton
  | ForwardButton
  | AddressBarEntry
  | Programmatic
  | Unknown
```

Each navigation event between two nodes appends a `Traversal` record to the edge's `traversals` list. Repeated traversals are recorded (not deduplicated). The full traversal list is the history of all navigation over that edge.

### 2.3 Traversal Append Rules

All traversal append logic lives in a single `push_traversal` function (reducer layer).

Skip rules — a traversal is **not** recorded when:

- Source and destination nodes are the same (self-loop navigation).
- The destination node is unknown (not yet in the graph).
- The navigation event has `#nohistory` tag on the source or destination node.

**Invariant**: UI and render code must not mutate traversal state directly. All mutations route through the reducer via `AppendTraversal` intent or its WAL equivalent.

---

## 3. WAL Integration Contract

### 3.1 LogEntry Extensions

The WAL includes traversal-aware entries:

```
LogEntry =
  | AppendTraversal { edge_id, traversal: Traversal }
  | AssertEdge { ... }
  | RetractEdge { ... }
  | … (existing)
```

**Replay invariant**: Replaying WAL entries must produce the same `traversals` list as the original append sequence. The replay path reuses the same `push_traversal` append semantics.

### 3.2 Archive Keyspaces

The persistence layer maintains two dedicated archive keyspaces:

- `traversal_archive` — hot traversal records for the History Manager Timeline tab.
- `dissolved_archive` — dissolved/collapsed traversal records for the History Manager Dissolved tab.

Archive operations:
- `archive_append_traversal(edge_id, traversal)` — append to `traversal_archive`.
- `archive_dissolved_traversal(edge_id, dissolved_record)` — append to `dissolved_archive`.

**Invariant**: Archive append order and in-memory mutation order must match. Crash/recovery semantics for archival are treated as persistence work (Storage subsystem), not UI work.

---

## 4. Edge Visual Presentation Contract

The render layer derives edge visuals from `EdgePayload`. It does not define traversal truth.

### 4.1 Traversal-Aware Edge Rendering

| EdgePayload state | Visual |
|-------------------|--------|
| `TraversalDerived`, 1 traversal | Thin solid line; direction arrow |
| `TraversalDerived`, N traversals | Stroke width proportional to log(N); direction arrow toward dominant direction |
| `UserGrouped` | Distinct style (e.g., dashed or colored); no direction arrow |
| Both kinds on same pair | Combined rendering; `UserGrouped` style dominates with traversal weight overlay |

**Dominant direction**: The direction with the majority of traversal records. Computed at render time from `traversals`; not stored.

### 4.2 Edge Tooltip / Inspection

On edge hover: tooltip shows:
- Edge kind (`UserGrouped` / `TraversalDerived`)
- Total traversal count
- Most recent traversal timestamp and trigger

This is a read-only inspection surface. No mutations from tooltip interaction.

---

## 5. History Manager Surface Contract

### 5.1 Panel Structure

The History Manager is a non-modal tool pane with two tabs:

- **Timeline**: ordered list of recent traversal events across all edges, newest first.
- **Dissolved**: dissolved/collapsed traversal records for edges that have been archived.

### 5.2 Timeline Tab

- Shows the N most recent traversal records (configurable; default 50).
- Each entry: source node title, destination node title, relative timestamp, `NavigationTrigger` indicator.
- Click on an entry: emit `SelectNode` + `RequestZoomToSelected` intents for the destination node.
- Timeline is read-only from this surface. No delete/edit from the timeline tab.

### 5.3 Dissolved Tab

- Shows dissolved/collapsed traversal records from the `dissolved_archive` keyspace.
- Layout TBD in a subsequent plan; this spec records that the tab exists and is backed by `dissolved_archive`.

### 5.4 Panel Open/Close

- Keyboard shortcut (configurable; default unbound).
- Settings menu entry.
- Command palette: "Open History Manager".

---

## 6. Temporal Navigation (Planned)

Timeline scrubber and preview mode are planned but not yet in scope for current implementation stages. When implemented, they must satisfy:

- Preview mode is read-only and isolated; it must not mutate graph state.
- "Return to present" must restore exactly the state that was active before preview entered.
- Preview isolation aligns with `history_timeline_and_temporal_navigation_spec.md` replay/preview contracts.

This section is a placeholder for future spec expansion.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Self-loop navigation is not recorded | Test: navigate A → A → no traversal appended to any edge |
| `#nohistory` node suppresses traversal | Test: navigate to node with `#nohistory` → no traversal recorded |
| Repeated traversal A → B appends multiple records | Test: navigate A → B three times → edge has 3 traversal records |
| WAL replay produces identical traversal list | Test: replay WAL from empty state → `traversals` list matches original |
| Stroke width reflects traversal count | Test: 1 traversal vs 10 traversals on same edge → measurable width difference |
| Dominant direction computed at render time | Test: `EdgePayload` has no `dominant_direction` field |
| Timeline shows newest entry first | Test: navigate A→B then C→D → C→D appears above A→B in timeline |
| Timeline click emits `SelectNode` and `RequestZoomToSelected` | Test: click timeline entry → both intents in intent queue |
| `traversal_archive` and `dissolved_archive` are separate keyspaces | Test: append to each → query confirms entries in respective keyspace only |
| UI cannot mutate traversal state directly | Architecture invariant: no `push_traversal` call from render or UI layer |
