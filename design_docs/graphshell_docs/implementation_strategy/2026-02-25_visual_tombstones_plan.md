# Visual Tombstones (Ghost Nodes / Ghost Edges) Plan (2026-02-25)

**Status**: Planned / Backlog — promoted from research
**Source research**: `design_docs/graphshell_docs/research/2026-02-24_visual_tombstones_research.md`
**Relates to**: `2026-02-24_immediate_priorities.md` §2 (Forgotten Concepts rank 1), `2026-02-23_udc_semantic_tagging_plan.md` (badge/dimmed rendering reuse), `2026-02-20_edge_traversal_impl_plan.md` (traversal history dependency)

## Context

When a user deletes a node, all edges to and from that node are removed, leaving a structural hole in the graph. Visual Tombstones preserve the topology of the deleted node as a lightweight, non-interactive placeholder ("ghost") that remains visible until explicitly cleared.

This is distinct from P2P "Ghost Nodes" (which are conflict artifacts in Verse sync). These tombstones are a user-facing "deleted but remembered" mechanism.

**Adoption trigger**: After traversal/history UI and deletion UX are stable (see `2026-02-20_edge_traversal_impl_plan.md` Stage E).

---

## Scope and Constraints

1. Tombstones are **read-only graph artifacts** — they cannot be navigated to or edited; only restored or permanently deleted.
2. Tombstones are **opt-in visible** — hidden by default behind a "Show Deleted" toggle in Graph View settings.
3. Tombstone data retains the minimum identity payload: `id`, `position`, `title` (optional). Content fields (`url`, `thumbnail`, `favicon`) are dropped on tombstone creation.
4. This plan targets **graph panes** only. Tombstone rendering is not relevant to viewer panes or tool panes.
5. Tombstone GC policy is **data-model only** in Phase 1 — no UI surface for bulk management until Phase 2.

---

## Phase 1: Data Model and Basic Rendering

**Prerequisite**: Deletion UX stable (confirm delete removes node and edges cleanly).

### 1.1 `NodeState::Tombstone` lifecycle state

- Add `Tombstone` variant to `NodeState` (alongside `Active` / `Warm` / `Cold`).
- On node delete: instead of removing the node from the graph, transition to `NodeState::Tombstone`.
  - Drop `url`, `thumbnail`, `favicon` fields.
  - Retain `id`, `position`, `title` (preserve last known title as `Option<String>`).
  - Retain all edges. Edge style flag set to `EdgeStyle::Ghost` for tombstone-connected edges.
- Tombstone nodes are excluded from all graph queries, search, layout physics, and traversal logic. They are inert structural records only.

### 1.2 `EdgeStyle::Ghost` variant

- Add `Ghost` variant to `EdgeStyle` (or equivalent rendering hint).
- Edges connected to a tombstone node render as dashed/faded lines.
- Ghost edges are non-interactive (no click-to-navigate, no traversal).

### 1.3 Toggle: "Show Deleted"

- Add a boolean setting `show_tombstones: bool` (default: `false`) to the Graph View settings surface.
- When `false` (default): tombstone nodes and their ghost edges are excluded from the render pass entirely.
- When `true`: tombstones render as a faint dashed outline or "×" marker with no fill. Ghost edges render as dashed/faded lines.
- This toggle mirrors the "Show Archived" pattern from the `#archive` tag rendering path.

### 1.4 Tombstone visual style

- **Node**: dashed outline (no fill), reduced opacity (~30%). A small "×" marker may overlay the center.
- **Edges**: dashed stroke, reduced opacity (~40%). Use a distinct color from active edges (e.g., muted gray).
- Tombstone nodes do not respond to hover, drag, or primary click. Right-click opens a minimal context menu: **Restore** or **Permanently Delete**.

### 1.5 Persistence

- Tombstones are persisted in the graph snapshot (fjall log + redb snapshot) like any other node state.
- Tombstone entries survive app restarts.
- Garbage collection: tombstones older than **N days** (default: 30 days, configurable via preferences) are eligible for silent GC on next snapshot write. This is enforced in the snapshot write path, not at delete time.

**Phase 1 done gate**:
- `NodeState::Tombstone` exists in the data model.
- Deleting a node produces a tombstone (not a hard removal).
- `show_tombstones` toggle controls render visibility.
- Tombstones persist and survive restart.
- `cargo check` green; targeted unit tests for tombstone state transitions pass.

---

## Phase 2: Interaction and Management

**Prerequisite**: Phase 1 done gate satisfied.

### 2.1 Restore flow

- Right-click on a tombstone node → "Restore": transitions `NodeState::Tombstone` → `NodeState::Cold` (or the state it held before deletion, if recorded).
- Restoration does not recover dropped content fields (`url`, `thumbnail`, `favicon`). The node is re-activated as a content-less placeholder; the user must re-navigate or re-attach content.
- Ghost edges revert to normal edge style on restore.

### 2.2 Permanent delete

- Right-click → "Permanently Delete": removes the tombstone node and all its ghost edges from the graph entirely. This is the only path to a hard structural removal.
- Emits a distinct `GraphIntent::PermanentlyDeleteTombstone` (separate from the standard soft-delete intent) for diagnostics/audit purposes.

### 2.3 Bulk tombstone management ("Clear Tombstones" command)

- Add `GraphIntent::ClearTombstones` that removes all tombstone nodes and ghost edges in one operation.
- Expose in Command Palette as "Clear deleted nodes" (only visible when at least one tombstone exists).
- Confirmation prompt before executing.

### 2.4 Retention preference UI

- Add a preference for tombstone GC age threshold (`tombstone_retention_days`, default: 30).
- Exposed in the app preferences panel alongside other graph lifecycle settings.
- Validate: 0 disables GC (tombstones never auto-expire), negative values are rejected.

**Phase 2 done gate**:
- Restore, permanent delete, and bulk clear are all reachable via Command Palette and right-click.
- Retention preference is visible and respected by snapshot GC path.
- Harness scenario covering tombstone lifecycle (delete → persist → restore / permanently delete) passes.

---

## Integration Notes

- **Badge/tag rendering reuse**: The `#archive` dimmed rendering path (`2026-02-23_udc_semantic_tagging_plan.md`) shares visual vocabulary with tombstone rendering. Prefer reusing the dim-render infrastructure rather than introducing a parallel rendering pass.
- **Traversal history**: Tombstones preserve the structural memory of edges that were traversed before deletion. Edge traversal queries should treat tombstone-connected edges as non-navigable but structurally present. See `2026-02-20_edge_traversal_impl_plan.md`.
- **Physics**: Tombstone nodes must be excluded from force-directed layout calculations. They hold a fixed position (last known position at delete time) and do not participate in repulsion/attraction.
- **Search/filter**: Tombstone nodes are excluded from all fuzzy search and graph filter results unless "Show Deleted" is active. Even with the toggle on, tombstones should be visually distinguishable in result sets.

---

## Non-Goals (Phase 1 + 2)

- No animated "deletion ghost" effect on delete action.
- No tombstone-aware undo/redo (soft-delete is the undo substitute for Phase 1).
- No content recovery from tombstone (URL/thumbnail/favicon are dropped on transition).
- No P2P tombstone sync (Verse sync treats tombstones as local state; cross-peer tombstone propagation is out of scope until Verse Tier 2).
