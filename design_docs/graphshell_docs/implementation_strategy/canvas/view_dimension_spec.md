# View Dimension (TwoD/ThreeD) — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Blocked (see §7)

**Related**:

- `CANVAS.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-27_viewdimension_acceptance_contract.md`
- `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md`
- `../../TERMINOLOGY.md` — `ViewDimension`, `ThreeDMode`, `ZSource`, `Derived Z Positions`, `Dimension Degradation Rule`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **ViewDimension state** — ownership, persistence, and canonical values.
2. **TwoD ↔ ThreeD transition** — interaction continuity, selection continuity, state integrity.
3. **Degradation** — deterministic fallback when 3D is unavailable.
4. **ZSource** — how per-node `z` positions are derived.
5. **Observability** — required diagnostics and evidence for acceptance.

---

## 2. ViewDimension State Contract

`ViewDimension` is **per-Graph-View** state, not global. Each Graph View pane has its own `ViewDimension`.

```
ViewDimension =
  | TwoD
  | ThreeD { mode: ThreeDMode, z_source: ZSource }

ThreeDMode =
  | TwoPointFive
  | Isometric
  | Standard

ZSource = (per-node z-placement policy; see §5)
```

**Persistence**: `ViewDimension` is persisted as part of Graph View state in the frame snapshot. It is **not** a frame-level UI setting — it is durable intent.

**Ownership**: `ViewDimension` changes are explicit reducer-owned state transitions. They must not be triggered by implicit widget state or frame metadata.

---

## 3. TwoD ↔ ThreeD Transition Contracts

### 3.1 Interaction Continuity

1. Pan/zoom ownership remains deterministic before and after transition. No focus-repair clicks required after transition to resume standard camera interaction.
2. Camera commands (`fit`, `fit selection`, keyboard zoom) continue to target the active graph view through the transition.
3. Transition must not reset camera position silently. `(x, y)` camera state is preserved.

### 3.2 Selection Continuity

1. The selected-node set and primary selection are preserved across transition.
2. Selection visualization may change by mode (2D vs 3D rendering); selection truth must not reset silently.
3. Lasso/selection command routing remains valid for the active graph view after transition.

### 3.3 State Integrity

1. `ViewDimension` changes are explicit reducer state transitions. No implicit side-effects on unrelated graph topology state.
2. A transition failure or unsupported path must produce an explicit degraded/fallback outcome (§4), not a silent no-op.
3. `(x, y)` graph positions remain stable across transitions in both directions.

---

## 4. Degradation Contract

When persisted `ThreeD` state is restored and 3D rendering is unavailable, Graphshell **deterministically degrades** that view to `TwoD`.

**Degradation rules**:

1. `(x, y)` node positions are preserved; no position reset.
2. Ephemeral `z` values are discarded. They are never persisted independently and are recomputed on next 2D→3D entry.
3. The degradation reason is observable: the diagnostics system emits a channel event identifying the cause (unsupported capability, unavailable backend, blocked path, etc.).
4. Degradation must not affect other Graph View panes in the same workbench.
5. A degraded view may be manually re-elevated to `ThreeD` by the user if the capability becomes available; the system does not auto-elevate.

**Degradation mode values**: `full` (3D rendering active), `partial` (fallback active, some 3D features unavailable), `unavailable` (forced to TwoD).

---

## 5. ZSource Contract

`ZSource` is the policy for deriving per-node `z` placement when a Graph View is in `ThreeD` mode.

- `ZSource` configuration is persisted as part of `ViewDimension`.
- Derived per-node `z` positions are **ephemeral** runtime data. They are recomputed on 2D→3D entry and discarded on 3D→2D transition.
- `z` values must never be persisted independently of `ViewDimension`.

**ZSource derivation invariant**: Derived `z` positions are always computable from `ZSource` + current node metadata. Recalculating `z` for the same `ZSource` and the same node metadata must produce the same result (deterministic derivation).

---

## 6. Observability Requirements

1. Mode transitions must emit diagnosable events/channels for: success, fallback/degradation, and blocked paths.
2. The degradation reason must be observable (specific error or capability code), not just "3D unavailable."
3. Acceptance evidence must include targeted diagnostics and automated tests, not manual repro notes only.

---

## 7. Blocking Prerequisites

`#19` (ViewDimension hotswitch implementation) remains blocked until:

1. 3D rendering backend capability is confirmed.
2. Compositor pass-order correctness and GL-state diagnostics are hardened (`viewer/2026-02-26_composited_viewer_pass_contract.md`).
3. The `TileRenderMode` enum is set on every `NodePaneState` at viewer attachment time.

This spec defines the acceptance contract; implementation is not authorized until prerequisites are closed (see `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md`).

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Pan/zoom works immediately after transition | Test: transition TwoD↔ThreeD → camera commands respond without extra focus repair |
| Selected nodes preserved across transition | Test: select N nodes, transition → same N nodes still selected |
| `(x, y)` positions stable across transition | Test: transition TwoD→ThreeD→TwoD → all `(x, y)` positions within epsilon of original |
| `z` positions are not persisted | Test: snapshot roundtrip while in ThreeD → no per-node `z` field in serialized form |
| Degradation falls back to TwoD deterministically | Test: force 3D-unavailable condition → view degrades to TwoD, positions preserved |
| Degradation emits observable diagnostics channel event | Test: degradation path → diagnostics channel records event with reason field |
| Degradation does not affect sibling panes | Test: one pane degrades → other panes in same workbench unaffected |
| Reducer transition does not mutate graph topology | Test: transition intent → no `AddNode`/`RemoveNode`/`AddEdge`/`RemoveEdge` in intent side-effects |
