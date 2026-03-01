# Node Audit Log — Deferred Spec Stub

**Date**: 2026-02-28
**Status**: Deferred — not in current implementation scope
**Priority**: Prospective

**Related**:

- `edge_traversal_spec.md` — §2.6 physics exclusion invariant (self-loops must be excluded from physics)
- `../canvas/graph_node_edge_interaction_spec.md` — §6.4 (self-loop edge as audit log carrier)
- `SUBSYSTEM_HISTORY.md` — history subsystem contracts
- `../../TERMINOLOGY.md` — `Node`, `EdgePayload`, `Tag`

---

## Why This Stub Exists

The v2.1 edge traversal report proposed using logical self-loop edges (edges where `source == target`) as per-node event carriers for `MetadataChange` and workbench-action events. This was excluded from `edge_traversal_spec.md` because node audit events are a distinct concern from edge traversal history.

This stub records where that design belongs and the constraints it must satisfy when implemented.

---

## Deferred Design Concept

A **Node Audit Log** is a per-node ordered record of non-navigation events that affected the node:

- Metadata property changes (title, tags, address, `mime_hint`, notes).
- Workbench-action events (node opened in a pane, pane closed, node tombstoned/restored).

The proposed carrier mechanism is a logical self-loop edge on the node — an `EdgePayload` with `source == target` — holding an ordered list of audit event records rather than `Traversal` records.

---

## Required Constraints (Must Satisfy When Implemented)

1. **Physics exclusion**: Self-loop edges must not participate in force-directed physics simulation, regardless of content.
2. **No canvas rendering**: Self-loop edges must not render as circular lines on the canvas. They are logical-only.
3. **Separate event type**: Audit event records are not `Traversal` records. They must not be mixed into the `traversals` list or the `traversal_archive` keyspace. A separate audit event type and archive keyspace are required.
4. **History Manager scope**: Node-scoped audit log viewing in the History Manager timeline (filtering to a selected node's events) is an anticipated surface. The audit log must integrate with History Manager query semantics, not replace the traversal timeline.
5. **WAL entry**: Audit log appends must be WAL-durable (separate `LogEntry` variant, not `AppendTraversal`).
6. **Reducer authority**: Audit log mutation must route through the reducer, not from render or UI code directly.

---

## Open Questions (Pre-Design)

- Is a self-loop edge the right carrier, or is a separate per-node log structure cleaner?
- What is the `EdgeKind` for an audit self-loop? It must not be `TraversalDerived`, `UserGrouped`, or `AgentDerived`.
- How does the rolling window / eviction contract from `edge_traversal_spec.md §2.4` apply to audit events?
- What is the History Manager query contract for mixed traversal + audit timelines?

---

## When to Design This

Design this spec when:

1. The traversal model (Stage E) is stable and test-covered.
2. There is a clear product need for per-node metadata change history (e.g., for undo, audit export, or node-scoped timeline views).

Do not implement any self-loop edge logic or audit event types until this spec is written and reviewed.
