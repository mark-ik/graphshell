# ViewDimension Acceptance Contract (`#19` / R2)

**Date**: 2026-02-27  
**Status**: Draft (roadmap docs lane, implementation pending)  
**Lane**: `lane:roadmap`  
**Scope**: Acceptance contract for `TwoD` ↔ `ThreeD` mode transitions in Graph View panes.

---

## 1. Purpose

This contract defines the minimum acceptance behavior for `ViewDimension` transitions so `#19` can move from blocked planning into implementation-ready issue slices.

This document is **contract/spec only**. It does not authorize runtime hotspot edits by itself.

---

## 2. Canonical Terms and Ownership

- `ViewDimension` is the per-Graph-View mode state (`TwoD` or `ThreeD { mode, z_source }`).
- `z` positions are **derived/ephemeral** runtime data, not independently persisted state.
- Persistence ownership is the Graph View state snapshot boundary (not per-frame ad hoc UI metadata).
- Degradation mode must be explicit when 3D rendering is unavailable.

Terminology aligns with:
- `design_docs/TERMINOLOGY.md` (`Graph View`, `Camera`, `Degradation Mode`, `TileRenderMode`)
- Existing `ViewDimension`/`ZSource` code comments in `app.rs`.

---

## 3. Acceptance Contract (R2)

A `TwoD` ↔ `ThreeD` transition is accepted only if all criteria below hold.

### 3.1 Interaction continuity

1. Pan/zoom ownership remains deterministic before and after transition.
2. Camera commands (`fit`, `fit selection`, keyboard zoom) continue to target the active graph view.
3. Transition must not require extra focus-repair clicks for standard camera interaction.

### 3.2 Selection continuity

1. Selected-node set and primary selection are preserved across transition.
2. Selection visualization may change by mode, but selection truth may not reset silently.
3. Lasso/selection command routing remains valid for the active graph view.

### 3.3 State integrity (no silent corruption)

1. `ViewDimension` changes must be explicit reducer-owned state transitions.
2. Transition must not mutate unrelated graph topology state.
3. Failures or unsupported mode paths must produce explicit degraded/fallback outcome, not silent no-op.

### 3.4 Persistence and deterministic fallback

1. Persisted `ViewDimension` intent is restored when supported.
2. If 3D is unavailable on restore/runtime, behavior deterministically degrades to `TwoD`.
3. `(x, y)` graph positions remain stable across degrade/restore transitions.
4. Ephemeral `z` derivation is recomputed on 2D→3D entry and discarded on 3D→2D.

### 3.5 Observability and diagnostics

1. Mode transitions emit diagnosable events/channels for success/fallback/block paths.
2. Degradation reason is observable (unsupported capability, unavailable backend, blocked path, etc.).
3. Acceptance evidence must include targeted diagnostics and tests, not only manual repro notes.

---

## 4. Required Evidence for R2 Done Gate

R2 is considered complete when all evidence classes are linked:

1. **Contract reference links**
   - `PLANNING_REGISTER.md` readiness checklist references this contract.
   - `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md` references this contract under R2.

2. **Issue-stack linkage**
   - Future child issues under `#19` map to each contract area:
     - transition semantics,
     - render integration,
     - persistence/degradation tests,
     - UX feedback/shortcuts.

3. **Verification artifacts (implementation phase)**
   - Focused tests for transition continuity and persistence/degradation behavior.
   - Diagnostics proof for success/fallback/block outcomes.

---

## 5. Non-goals (R2)

- Implementing 3D rendering details.
- Altering compositor pass contract definitions.
- Runtime hotspot refactors in `app.rs`, `render/mod.rs`, `gui.rs`, or `shell/desktop/workbench/*` as part of this docs slice.

---

## 6. Exit Condition Contribution

This contract satisfies the roadmap requirement to define a single acceptance target for `ViewDimension` behavior. `#19` remains blocked until other prerequisites in the roadmap readiness checklist are also closed.
