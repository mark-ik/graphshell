# Node Audit Log — Implementation Spec

**Date**: 2026-03-18 (revised; originally 2026-02-28 stub)
**Status**: Implementation-ready spec
**Track**: History subsystem — NodeAuditHistory (§2.3 of unified architecture plan)

**Related**:

- `SUBSYSTEM_HISTORY.md`
- `2026-03-08_unified_history_architecture_plan.md` §2.3, §7.4
- `node_navigation_history_spec.md` — sibling NodeNavigationHistory track
- `history_timeline_and_temporal_navigation_spec.md` — Stage F temporal navigation
- `../../TERMINOLOGY.md` — `Node`, `EdgePayload`, `Tag`

---

## 1. What This Is

**NodeAuditHistory** records metadata and lifecycle changes to a node over its
lifetime. Each time a node is renamed, tagged, pinned, or has its URL changed
out-of-band, one `AppendNodeAuditEvent` entry is appended to the WAL.

This resolves the gap where the existing snapshot entries (`UpdateNodeTitle`,
`TagNode`, `PinNode`, etc.) carry no timestamp and produce no queryable event
trail.

### Relation to other tracks

| Track | Records | WAL entry | Surface |
|---|---|---|---|
| TraversalHistory | Inter-node navigation | `AppendTraversal` | History Manager |
| NodeNavigationHistory | Intra-node URL changes | `NavigateNode` | Node history panel |
| **NodeAuditHistory** | **Metadata/lifecycle changes** | **`AppendNodeAuditEvent`** | Node audit panel |

Audit events are **not** traversal events and must not appear in the traversal
archive keyspace or the traversal timeline rows.

---

## 2. WAL Entry Schema

### 2.1 `NodeAuditEventKind`

```rust
pub enum NodeAuditEventKind {
    TitleChanged { new_title: String },
    Tagged { tag: String },
    Untagged { tag: String },
    Pinned,
    Unpinned,
    UrlChanged { new_url: String },   // out-of-band URL set, not via NavigateNode
    Tombstoned,
    Restored,
}
```

Each variant records only the new value. To recover the previous value, query
the prior audit entry for the same field. Diffing is a query-time operation,
not a storage concern.

### 2.2 `LogEntry::AppendNodeAuditEvent`

```rust
LogEntry::AppendNodeAuditEvent {
    node_id: String,
    event: NodeAuditEventKind,
    timestamp_ms: u64,
}
```

### 2.3 Relationship to snapshot entries

Existing snapshot entries (`UpdateNodeTitle`, `TagNode`, `UntagNode`, `PinNode`,
`UpdateNodeUrl`) remain in the WAL. They are required for replay to reconstruct
current node state. `AppendNodeAuditEvent` is **additive** — emitted alongside
the snapshot entry, not instead of it.

Emit sequence for a title change:

1. Apply `GraphDelta::SetNodeTitle` to graph state
2. `log_mutation(&LogEntry::UpdateNodeTitle { ... })` — snapshot for replay
3. `log_audit_event(node_id, TitleChanged { new_title }, timestamp_ms)` — audit trail

---

## 3. Emit Sites

| Mutation | Snapshot entry | Audit event |
|---|---|---|
| Title change | `UpdateNodeTitle` | `TitleChanged` |
| Tag added | `TagNode` | `Tagged` |
| Tag removed | `UntagNode` | `Untagged` |
| Pin set | `PinNode { is_pinned: true }` | `Pinned` |
| Unpin | `PinNode { is_pinned: false }` | `Unpinned` |
| URL changed (out-of-band) | `UpdateNodeUrl` | `UrlChanged` |
| Node tombstoned | *(lifecycle intent)* | `Tombstoned` |
| Node restored | *(lifecycle intent)* | `Restored` |

All emits route through the reducer. Render and shell code must not call
`log_audit_event` directly.

**Note on `TagNode`/`UntagNode`**: Prior to this spec landing, the `TagNode` and
`UntagNode` `LogEntry` variants existed in the schema but were never actually
written to the WAL from the intent handler. This spec lands their first
real WAL writes, alongside the audit events.

---

## 4. Query Contract

### 4.1 `GraphStore::node_audit_history(node_id, limit)`

Scans the log keyspace in reverse (newest-first), collects
`AppendNodeAuditEvent` entries where `entry.node_id == node_id`, returns up to
`limit` entries. O(log_size) scan — no secondary index at prototype scale.

### 4.2 `GraphBrowserApp::node_audit_history_entries(node_id, limit)`

Delegates to `store.node_audit_history(&node_id.to_string(), limit)`.

---

## 5. Timeline Index Coverage

`timeline_index_entries` indexes `AppendNodeAuditEvent` entries alongside
`AppendTraversal`, `AddNode`, `RemoveNode`, and `NavigateNode`. The Stage F
scrubber therefore reflects metadata changes as timeline events.

---

## 6. Isolation Invariants

1. **WAL-only truth**: Audit history is derived from WAL entries. No in-memory
   audit log accumulates on `NodeState` or workspace fields.
2. **Not in traversal archive**: `AppendNodeAuditEvent` entries must not appear
   in `traversal_archive_keyspace` or `dissolved_archive_keyspace`.
3. **Preview isolation**: `AppendNodeAuditEvent` must not be emitted during
   history preview mode. The existing graph-mutation block gate covers all
   mutation intents that trigger audit events.
4. **Reducer boundary**: `log_audit_event` is called only from the reducer
   (`intent_phases.rs`, `graph_mutations.rs`), never from render or shell code.
5. **Replay no-op**: `AppendNodeAuditEvent` produces no graph state change
   during WAL replay — the snapshot entries handle that.

---

## 7. Open Questions (resolved)

- **Self-loop edge vs. WAL entry**: The original stub proposed self-loop edges
  as audit carriers. This spec uses WAL entries instead — cleaner separation
  from graph topology, no physics/rendering exclusion concerns, consistent with
  `NavigateNode` and `AppendTraversal` precedent.
- **Rolling window / eviction**: Deferred. No eviction policy at prototype
  scale; all audit entries are retained in the WAL.
- **Mixed history timeline**: Deferred per §6.2 of unified architecture plan.
  The audit query surface is per-node only until the mixed timeline contract is
  defined.

---

## 8. Acceptance Criteria

- [ ] `NodeAuditEventKind` enum present with rkyv derivation
- [ ] `LogEntry::AppendNodeAuditEvent` variant present
- [ ] `log_audit_event` helper on `GraphStore`
- [ ] Audit events emitted from: title change, tag add/remove, pin/unpin, URL change
- [ ] `Tombstoned`/`Restored` events emitted when lifecycle intents land those paths
- [ ] `node_audit_history` query returns newest-first `AppendNodeAuditEvent` entries
- [ ] `timeline_index_entries` indexes audit event timestamps
- [ ] Replay handler arm is no-op for `AppendNodeAuditEvent`
- [ ] No audit events emitted during history preview mode
- [ ] `TagNode`/`UntagNode` snapshot entries now actually written to WAL (were dead code)

---

## 9. Out of Scope

- Mixed-history timeline (traversal + navigation + audit interleaved)
- Node audit panel UI (surface pending after query layer is stable)
- `Tombstoned`/`Restored` event wiring (lifecycle intent paths not yet landed)
- Cross-system boundary events linking audit to `AWAL`/lineage DAGs
- Eviction/archive policy for audit entries
