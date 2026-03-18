# NodeNavigationHistory — Implementation Spec

**Date**: 2026-03-18
**Status**: Implementation-ready spec
**Track**: History subsystem — NodeNavigationHistory (§2.2 of unified architecture plan)

**Related**:
- `SUBSYSTEM_HISTORY.md`
- `2026-03-08_unified_history_architecture_plan.md` §2.2, §3, §7.3
- `../../technical_architecture/2026-02-18_universal_node_content_model.md` §5
- `edge_traversal_spec.md` — TraversalHistory (separate track)
- `history_timeline_and_temporal_navigation_spec.md` — Stage F temporal navigation

---

## 1. What This Is

**NodeNavigationHistory** records the address evolution of a single node over
its lifetime. Each time a node navigates to a new URL (same-tab navigation,
redirect, Back/Forward), one `NavigateNode` entry is appended to the WAL.

This is **not** the same as TraversalHistory:

| | TraversalHistory | NodeNavigationHistory |
|---|---|---|
| **What moves** | User navigates *between* nodes | URL changes *within* a single node |
| **WAL entry** | `AppendTraversal` | `NavigateNode` |
| **Truth carrier** | Edge payload traversal records | Per-node WAL log |
| **Surface** | History Manager timeline | Node history panel |
| **Archive** | Traversal archive keyspace | Node-scoped query over WAL |

---

## 2. WAL Entry Schema

### 2.1 New LogEntry variant

```rust
LogEntry::NavigateNode {
    node_id: String,         // UUID string of the node
    from_url: String,        // URL the node held before this navigation
    to_url: String,          // URL the node navigated to
    trigger: PersistedNavigationTrigger,  // reuse existing enum
    timestamp_ms: u64,       // wall-clock ms since UNIX epoch
}
```

This is a new variant alongside (not replacing) `UpdateNodeUrl`.

**Relationship to `UpdateNodeUrl`**: `UpdateNodeUrl` is a snapshot-style
mutation entry that moves the node's canonical URL forward. `NavigateNode`
is a history-style append that additionally records the `from_url` so the
full address lineage is reconstructable from the WAL alone. Both may be
emitted for the same navigation event:

- `NavigateNode` is emitted first (to record the transition)
- `UpdateNodeUrl` is emitted after (to update the canonical URL field)

A future migration step may retire `UpdateNodeUrl` once WAL replay correctly
derives node state from `NavigateNode` entries. Until then both coexist.

### 2.2 Wire format compatibility

`LogEntry` uses rkyv for serialization. Adding a new variant to the enum
extends the wire format. Existing WAL files written without `NavigateNode`
remain readable — rkyv's archived enum uses discriminant-based dispatch and
unknown discriminants should be handled at read sites with `_ => continue`
(already the pattern in `timeline_index_entries` and replay code).

The prototype accepts the schema change without a versioned migration.

---

## 3. Emit Path

### 3.1 Where NavigateNode is emitted

`NavigateNode` is emitted from the same code path that currently emits
`UpdateNodeUrl` — the URL-change handler in `graph_mutations.rs`
(`update_node_url` or equivalent). The emit sequence is:

```rust
// 1. Record the navigation history entry
store.log_mutation(&LogEntry::NavigateNode {
    node_id: node_id.to_string(),
    from_url: current_url.to_string(),
    to_url: new_url.to_string(),
    trigger,                          // from the intent or Unknown if not available
    timestamp_ms: Self::unix_timestamp_ms_now(),
});

// 2. Update canonical URL (existing path — unchanged)
store.log_mutation(&LogEntry::UpdateNodeUrl {
    node_id: node_id.to_string(),
    new_url: new_url.to_string(),
});
```

The `trigger` field reuses `PersistedNavigationTrigger`. At the
`UpdateNodeUrl` call sites that don't have trigger context, use
`PersistedNavigationTrigger::Unknown`.

### 3.2 Reducer authority

`NavigateNode` emission must route through the reducer. Render code and
direct field writes must not call `log_mutation` directly for navigation
events. The existing `UpdateNodeUrl` intent is the mutation boundary.

---

## 4. Query Contract

### 4.1 `node_navigation_history(node_id, limit)`

Add to `GraphStore`:

```rust
pub fn node_navigation_history(
    &self,
    node_id: &str,
    limit: usize,
) -> Vec<LogEntry>
```

Scans the log keyspace in reverse (newest-first), collects `NavigateNode`
entries where `entry.node_id == node_id`, and returns up to `limit` entries.

This is an O(log_size) scan with no secondary index. Acceptable for prototype
panel display (typical limit: 50–200 entries). If it becomes a performance
concern, add a secondary index keyspace `node_nav_history:{node_id}:{seq}` in
a follow-on step.

### 4.2 GraphBrowserApp accessor

```rust
pub fn node_navigation_history_entries(
    &self,
    node_id: Uuid,
    limit: usize,
) -> Vec<LogEntry>
```

Delegates to `store.node_navigation_history(&node_id.to_string(), limit)`.

---

## 5. Timeline Index Coverage

The `timeline_index_entries` function (already extended in task A to cover
`AddNode` / `RemoveNode`) should also index `NavigateNode` entries:

```rust
ArchivedLogEntry::NavigateNode { timestamp_ms, .. } => (*timestamp_ms).into(),
```

This means the Stage F scrubber will include node URL changes as timeline
events alongside structural mutations and traversal events.

---

## 6. Surface: Node History Panel

### 6.1 Panel trigger

The node history panel is accessible from:
- The node pane context menu: "History"
- The History Manager pane: clicking a traversal row's "from" node opens that
  node's history panel in a split or overlay (future)

### 6.2 Rendered content

Each row shows one `NavigateNode` entry:

```
[time_label]  [trigger_icon]  [from_url short] → [to_url short]
```

- `time_label`: human-relative (same format as History Manager rows)
- `trigger_icon`: same trigger icons as History Manager row rendering
- `from_url short` / `to_url short`: hostname or path-truncated display

### 6.3 Row click behavior

Clicking a row navigates the node to that historical URL:
- Emits `GraphIntent::NavigateNodeToUrl { key, url: entry.to_url }`
- Does NOT enter preview mode (this is live navigation to a historical URL,
  not temporal graph preview)

### 6.4 Panel location

The node history panel renders inside the node pane as a collapsible section
or via a tab-like affordance within the existing node pane UI. It does **not**
require a new ToolPaneState variant for the initial implementation.

---

## 7. Isolation and Invariants

1. **WAL-only truth**: Node navigation history is derived entirely from WAL
   replay. It must not be stored as a separate in-memory `Vec` on `NodeState`
   or any workspace field — this would create a second mutable store that drifts
   from WAL truth.
2. **Not in TraversalHistory**: `NavigateNode` entries must not appear in the
   traversal archive keyspace. They are separate from `AppendTraversal`.
3. **Not in dissolved archive**: `NavigateNode` entries are not dissolved. They
   are permanent WAL records tied to the node's lifetime.
4. **Preview isolation**: `NavigateNode` must not be emitted during history
   preview mode (`history_preview_mode_active = true`). The existing
   `as_graph_mutation()` block gate already covers `UpdateNodeUrl` intents —
   the `NavigateNodeToUrl` intent must also be classified as a graph mutation
   so it is blocked in preview.
5. **Reducer boundary**: `NavigateNode` log writes occur only inside the
   reducer, not from the render or shell layer.

---

## 8. Relationship to TraversalHistory

TraversalHistory and NodeNavigationHistory are complementary, not competing:

- TraversalHistory answers: "What path did the user take across nodes?"
- NodeNavigationHistory answers: "What URLs has this specific node visited?"

A user can navigate from Node A to Node B (one `AppendTraversal` entry on the
A→B edge) and then navigate Node B's URL three times within the same node
(three `NavigateNode` entries on Node B, no new traversal entries).

The mixed-history timeline (showing both tracks interleaved by timestamp) is
explicitly deferred per `2026-03-08_unified_history_architecture_plan.md §6.2`.

---

## 9. Acceptance Criteria

### WAL schema (prerequisite)

- [ ] `LogEntry::NavigateNode` variant added with `node_id`, `from_url`,
      `to_url`, `trigger`, `timestamp_ms` fields
- [ ] rkyv Archive/Serialize/Deserialize derived for new variant
- [ ] `timeline_index_entries` indexes `NavigateNode` entries

### Emit path

- [ ] `NavigateNode` emitted by `update_node_url` mutation path before
      `UpdateNodeUrl`
- [ ] `NavigateNode` NOT emitted during history preview mode
- [ ] `NavigateNode` uses `unix_timestamp_ms_now()` (not zero)

### Query

- [ ] `GraphStore::node_navigation_history` returns `NavigateNode` entries
      for a given `node_id`, newest-first, up to `limit`
- [ ] Empty result for nodes with no navigation history (no entries ≠ error)

### WAL replay safety

- [ ] `replay_to_timestamp` handles `NavigateNode` entries correctly
      (applies `UpdateNodeUrl`-equivalent state update if needed, or skips
      if navigation history is replay-read-only)
- [ ] Pattern match sites with `_ => continue` do not panic on `NavigateNode`

### Surface

- [ ] Node history panel renders `NavigateNode` entries in newest-first order
- [ ] Each row shows time, trigger icon, from→to URL display
- [ ] Row click emits `NavigateNodeToUrl` intent (live navigation, not preview)

---

## 10. Out of Scope

- Mixed-history timeline (TraversalHistory + NodeNavigationHistory interleaved)
- Per-node address history stored on `NodeState` (WAL-only authority)
- Node audit log (metadata change history) — see `node_audit_log_spec.md`
- Back/Forward in-session state — this is in-memory renderer state, not
  NodeNavigationHistory (which is durable WAL-based per-node address history)
- Archive/eviction policy for `NavigateNode` entries (deferred until volume
  makes it necessary)
