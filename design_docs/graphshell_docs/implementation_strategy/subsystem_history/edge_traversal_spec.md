# Edge Traversal — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: In progress (Stages 0–E complete)

**Related**:

- `SUBSYSTEM_HISTORY.md`
- `history_timeline_and_temporal_navigation_spec.md`
- `2026-02-20_edge_traversal_impl_plan.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../../TERMINOLOGY.md` — `Traversal`, `Edge Traversal History`, `EdgePayload`, `EdgeType`, `AgentRegistry`

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
    kinds: EdgeKindSet,            -- set of active kinds (see §2.3)
    traversals: Vec<Traversal>,    -- rolling window (see §2.4)
    metrics: EdgeMetrics,          -- rolled-up aggregates (see §2.4)
}

EdgeKindSet = one or more of:
  | UserGrouped       -- explicit user-created connection
  | TraversalDerived  -- implicit; created by navigation event
  | AgentDerived      -- implicit; created by an AgentRegistry agent recommendation
```

**Invariant**: Display-only computations (dominant direction, stroke width) are derived from `EdgePayload` at render time. They must not be stored in `EdgePayload`.

**Multi-kind invariant**: `UserGrouped` and `TraversalDerived` may coexist on the same node pair. The union represents an edge that is both user-asserted and traversal-active. Rendering priority when both are present is defined in §4.1.

### 2.2 EdgeKind Rules

- `UserGrouped` is asserted by an explicit user action and retracted only by an explicit user action.
- `TraversalDerived` is asserted when the first `Traversal` record is appended to the edge and cannot be retracted independently (it persists as long as traversal records exist).
- `AgentDerived` is asserted by an `AgentRegistry` agent emit and is subject to time-decay and eviction rules (§2.5). It is promoted to `TraversalDerived` the first time a user navigates the edge (§2.5).

### 2.3 Traversal Record

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

Each navigation event between two nodes appends a `Traversal` record to the edge's `traversals` list. Repeated traversals are recorded (not deduplicated). The full traversal list within the rolling window is the recent history; older records are flushed to the archive and reflected in `metrics` (§2.4).

### 2.4 EdgeMetrics and Rolling Window

To bound in-memory size on heavily traversed edges, `EdgePayload` separates a bounded recent-events window from rolled-up aggregate metrics.

```
EdgeMetrics {
    total_navigations: u64,         -- incremented on every Traversal append; never decremented
    last_navigated_at: Option<DateTime>,
    agent_asserted_at: Option<DateTime>,  -- when AgentDerived was last set
    agent_confidence: Option<f32>,        -- last confidence score from asserting agent
}
```

**Rolling window contract**:

- `traversals` holds at most N recent records (configurable; default 100).
- When the window is full and a new `Traversal` is appended, the oldest record is evicted from memory and written to `traversal_archive` (§3.2) before appending the new record.
- `metrics.total_navigations` is incremented on every append, including evicted records. It reflects the true total, not the window size.
- `metrics.last_navigated_at` is always the timestamp of the most recently appended `Traversal`.

**Invariant**: Display-only computations (dominant direction, stroke width) must be derived from the rolling window or metrics — never from a full unbounded scan. The render layer must not assume `traversals` contains all historical records.

**Archive invariant**: Eviction from the rolling window must write to archive before the in-memory record is dropped. Crash-order guarantee is the Storage subsystem's responsibility (see `SUBSYSTEM_STORAGE.md`).

### 2.5 AgentDerived Decay and Promotion

`AgentDerived` edges are ephemeral suggestions from `AgentRegistry` agents. They are subject to time-decay and user-driven promotion.

**Decay rule**: An edge whose `kinds` set contains only `AgentDerived` (no `TraversalDerived`, no `UserGrouped`) will have its visual opacity faded over time. If no `Traversal` append occurs within the configured decay window (default: 72 hours), the `AgentDerived` kind is removed. If the `kinds` set becomes empty as a result, the edge is evicted from the active graph entirely.

**Promotion rule**: When a user navigates an `AgentDerived` edge, a `Traversal` record is appended normally via `push_traversal`. This asserts `TraversalDerived` on the edge's `kinds` set. Once `TraversalDerived` is present, decay is halted and the `AgentDerived` kind may be retained for provenance or removed; the edge is permanently part of the traversal-derived graph.

**Eviction is not history loss**: An evicted `AgentDerived` edge with zero traversals has no entries in `traversal_archive`. Eviction is the correct outcome. An edge promoted to `TraversalDerived` before eviction retains its full traversal history in archive as normal.

### 2.6 Traversal Append Rules

All traversal append logic lives in a single `push_traversal` function (reducer layer). Appending a traversal also updates `metrics.total_navigations` and `metrics.last_navigated_at` (§2.4).

Skip rules — a traversal is **not** recorded when:

- Source and destination nodes are the same (self-loop navigation).
- The destination node is unknown (not yet in the graph).
- The navigation event has `#nohistory` tag on the source or destination node.

**Invariant**: UI and render code must not mutate traversal state directly. All mutations route through the reducer via `AppendTraversal` intent or its WAL equivalent.

**Physics exclusion invariant**: Any edge whose `source == target` (self-loop, however created) must not participate in force-directed physics simulation and must not render as a literal circular line on the canvas. This applies regardless of how a self-loop edge came to exist.

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
| `TraversalDerived` only, 1 traversal | Thin solid line; direction arrow |
| `TraversalDerived` only, N traversals | Stroke width proportional to log(N); direction arrow toward dominant direction |
| `UserGrouped` only | Distinct style (e.g., dashed or colored); no direction arrow |
| `UserGrouped` + `TraversalDerived` | `UserGrouped` base style dominates; traversal stroke-width and direction arrow applied as modifiers on top |
| `AgentDerived` only | Low-opacity style (fading with time elapsed since assert); no direction arrow |
| `AgentDerived` + `TraversalDerived` | `TraversalDerived` style takes over fully; opacity restored; decay halted |

**Dominant direction**: The direction with the majority of traversal records. Computed at render time from `traversals` or `metrics`; not stored.

**Multi-kind rendering priority**: When multiple kinds are present, the base visual style is determined by the highest-priority kind present: `UserGrouped` > `TraversalDerived` > `AgentDerived`. Traversal-derived modifiers (stroke width, direction arrow) are applied on top of the base style whenever `TraversalDerived` is in the set, regardless of what the base style is.

### 4.2 Edge Tooltip / Inspection

On edge hover: tooltip shows:

- Edge kinds present (`UserGrouped`, `TraversalDerived`, `AgentDerived` — whichever are active)
- Total traversal count (`metrics.total_navigations`)
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
| Rolling window is bounded | Test: append 1,000 traversals → `traversals.len()` ≤ N (window size); `metrics.total_navigations` == 1,000 |
| Evicted records reach archive before memory drop | Test: fill window + 1 → oldest record present in `traversal_archive` before in-memory list shrinks |
| `AgentDerived` edge decays after threshold | Test: assert `AgentDerived` edge; advance clock past decay window → edge evicted from active graph |
| `AgentDerived` promoted on navigation | Test: assert `AgentDerived` edge; navigate it → `TraversalDerived` present in `kinds`; decay halted |
| Self-loop edges excluded from physics | Test: graph with a self-loop edge → layout simulation produces stable positions; no circular line rendered |
| Multi-kind rendering priority | Test: `UserGrouped` + `TraversalDerived` edge → `UserGrouped` base style present; traversal stroke-width modifier applied on top |
