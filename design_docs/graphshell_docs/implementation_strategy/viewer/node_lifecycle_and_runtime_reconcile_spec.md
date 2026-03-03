# Node Lifecycle and Runtime Reconcile Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `viewer_presentation_and_fallback_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../workbench/2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md`
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../../../TERMINOLOGY.md`

---

## 1. Purpose and Scope

This spec defines the lifecycle contract for node runtime state and reconcile behavior.

It governs:

- canonical lifecycle states (`Active`, `Warm`, `Cold`, `Tombstone`),
- transition rules and reconcile triggers,
- graph-backed vs ephemeral pane boundaries for lifecycle ownership,
- `RuntimeBlocked` handling and recovery affordances,
- pane/runtime consistency guarantees during lifecycle transitions,
- diagnostics and test contracts.

It does not govern:

- command-surface semantics,
- detailed renderer pass ordering,
- history timeline UI behavior.

---

## 2. Canonical Lifecycle Model

Lifecycle states:

- `Active`: node has a live runtime attachment and may render interactively.
- `Warm`: node runtime attachment may remain alive but is not foreground-visible.
- `Cold`: node has no live runtime attachment; metadata-only representation.
- `Tombstone`: node is deleted-but-preserved in graph structure (`Ghost Node` user-facing concept).

Lifecycle ownership rule:

- Only graph-backed nodes participate in the full node lifecycle model.
- Ephemeral panes may have runtime state, but they do not become `Tombstone` because they are not graph nodes.
- When an ephemeral pane is collapsed, closed, or simplified away, that is a pane-lifecycle event, not a node-lifecycle mutation.

Transient runtime condition:

- `RuntimeBlocked`: runtime activation failed and requires explicit recovery path.

---

## 3. Transition Contract

Allowed transitions:

- `Cold -> Active` (open/activate)
- `Active -> Warm` (deactivate but retain warm cache)
- `Warm -> Active` (resume)
- `Warm -> Cold` (eviction/dehydrate)
- `Active -> Cold` (hard teardown)
- `Active|Warm|Cold -> Tombstone` (delete or preserve/remove a graph-backed tile while keeping ghostable graph structure; see §3.1 for the collapse-driven preserving path)
- `Tombstone -> Cold` (restore)

RuntimeBlocked transitions:

- Any activation failure may set `RuntimeBlocked` while preserving last stable lifecycle state.
- `RuntimeBlocked` clear requires successful retry/reconcile, then resume to `Active` or `Cold` per policy.

Forbidden:

- direct `Tombstone -> Active` (must restore to `Cold` first),
- hidden `Tombstone` entry for non-graph-backed ephemeral panes,
- hidden lifecycle mutation from UI rendering codepaths.

### 3.1 Collapse-driven `Tombstone` entry

When a graph-backed pane is intentionally removed from live runtime in a way that preserves graph structure rather than deleting identity outright, the lifecycle may enter `Tombstone` through a collapse-driven path.

This path is valid when:

- the pane is already graph-backed (`Tile` / address-resolving node),
- user intent or reconcile policy preserves the node as ghostable graph structure,
- runtime attachment is being removed but semantic graph presence must remain observable.

This path is not valid when:

- the pane is still ephemeral and has never crossed into graph citizenship,
- the operation is only a presentation-mode change (`Docked <-> Tiled`),
- the operation is a temporary deactivation that should remain `Warm` or `Cold`.

Reconcile expectation:

- collapse-driven `Tombstone` must record an explicit reason classification distinct from destructive delete,
- pane state must drop references to live runtime attachments,
- graph identity remains restorable through the existing `Tombstone -> Cold` path.

---

## 4. Reconcile Contract

Reconcile loop responsibilities:

1. align desired pane state with lifecycle state,
2. enforce warm-cache budget and eviction policy,
3. prevent pane/runtime desynchronization,
4. publish diagnostics for transition and failure causes.

Consistency invariants:

- active node pane must not reference a tombstoned node,
- reconcile must distinguish pane-rest collapse from node-lifecycle mutation based on graph citizenship,
- lifecycle transition completion must leave pane state in a representable mode (`CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, `Placeholder`),
- reconcile failures must set explicit blocked/degraded state, never silent no-op.

Opening-mode / lifecycle invariant:

- `PaneOpeningMode` is consulted before lifecycle mutation.
- If the pane is ephemeral, reconcile may tear down runtime but must not enter `Tombstone`.
- If the pane is graph-backed, reconcile may enter `Cold` or `Tombstone` according to the preserving-vs-destructive policy.

---

## 5. RuntimeBlocked and Recovery

When `RuntimeBlocked` is set:

- user-visible recovery affordance is required (retry/reload or equivalent),
- diagnostics event must include reason classification,
- repeated retries are rate-limited by policy,
- successful recovery clears blocked state and emits success diagnostic.

---

## 6. Diagnostics Contract

Required channels:

- `lifecycle:transition` (Info)
- `lifecycle:runtime_blocked` (Warn/Error by cause)
- `lifecycle:recovery_attempt` (Info)
- `lifecycle:recovery_failed` (Warn/Error)
- `lifecycle:recovery_succeeded` (Info)

Minimum event payload:

- `node_key`,
- `from_state`, `to_state`,
- `graph_backed: bool`,
- `reason`,
- timestamp/frame index.

---

## 7. Test Contract

Required coverage:

1. Active/Warm/Cold transition matrix.
2. Tombstone restore path (`Tombstone -> Cold`).
3. Collapse-driven `Tombstone` entry for graph-backed panes only.
4. Ephemeral pane close/collapse does not create `Tombstone`.
5. RuntimeBlocked set/clear behavior with recovery affordance visibility.
6. Pane/runtime desync prevention under rapid open/close/focus changes.
7. Warm-cache eviction policy under memory pressure.

---

## 8. Acceptance Criteria

- [ ] Lifecycle transitions follow §3 and reject forbidden paths.
- [ ] Reconcile invariants in §4 hold under integration tests.
- [ ] Collapse-driven `Tombstone` entry is explicit for graph-backed panes and excluded for ephemeral panes.
- [ ] RuntimeBlocked behavior and recovery in §5 are user-visible and diagnosable.
- [ ] Diagnostics channels in §6 are emitted.
- [ ] Test suite in §7 is present and CI-gated.
