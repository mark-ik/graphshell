# Node Lifecycle and Runtime Reconcile Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `viewer_presentation_and_fallback_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../../TERMINOLOGY.md`

---

## 1. Purpose and Scope

This spec defines the lifecycle contract for node runtime state and reconcile behavior.

It governs:

- canonical lifecycle states (`Active`, `Warm`, `Cold`, `Tombstone`),
- transition rules and reconcile triggers,
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
- `Active|Warm|Cold -> Tombstone` (delete)
- `Tombstone -> Cold` (restore)

RuntimeBlocked transitions:

- Any activation failure may set `RuntimeBlocked` while preserving last stable lifecycle state.
- `RuntimeBlocked` clear requires successful retry/reconcile, then resume to `Active` or `Cold` per policy.

Forbidden:

- direct `Tombstone -> Active` (must restore to `Cold` first),
- hidden lifecycle mutation from UI rendering codepaths.

---

## 4. Reconcile Contract

Reconcile loop responsibilities:

1. align desired pane state with lifecycle state,
2. enforce warm-cache budget and eviction policy,
3. prevent pane/runtime desynchronization,
4. publish diagnostics for transition and failure causes.

Consistency invariants:

- active node pane must not reference a tombstoned node,
- lifecycle transition completion must leave pane state in a representable mode (`CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, `Placeholder`),
- reconcile failures must set explicit blocked/degraded state, never silent no-op.

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
- `reason`,
- timestamp/frame index.

---

## 7. Test Contract

Required coverage:

1. Active/Warm/Cold transition matrix.
2. Tombstone restore path (`Tombstone -> Cold`).
3. RuntimeBlocked set/clear behavior with recovery affordance visibility.
4. Pane/runtime desync prevention under rapid open/close/focus changes.
5. Warm-cache eviction policy under memory pressure.

---

## 8. Acceptance Criteria

- [ ] Lifecycle transitions follow §3 and reject forbidden paths.
- [ ] Reconcile invariants in §4 hold under integration tests.
- [ ] RuntimeBlocked behavior and recovery in §5 are user-visible and diagnosable.
- [ ] Diagnostics channels in §6 are emitted.
- [ ] Test suite in §7 is present and CI-gated.
