<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Layout Transition And History Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the layout-morphing and persistence-aware spatial history lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan for bounded position snapshots, layout transitions, and view-owned spatial undo/redo.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-04-03_layout_backend_state_ownership_plan.md`
- `2026-04-03_layout_variant_follow_on_plan.md`
- `2026-04-03_twod_twopointfive_isometric_plan.md`
- `layout_behaviors_and_physics_spec.md`
- `multi_view_pane_spec.md`

---

## Context

The umbrella physics note identified two adjacent follow-ons that are not yet owned anywhere else:

- layout morphing / interpolation between layout states
- persistence-aware layout history for spatial undo/redo

There is already one important guardrail in active docs: `layout_behaviors_and_physics_spec.md`
keeps progressive lens switching threshold-based and explicitly defers continuous interpolation
until a per-field contract exists. This plan is the right place to define that contract for the
graph layout/view lane instead of leaving transitions as ad hoc UI effects.

This lane is about **spatial state only**. It is not graph-topology undo, not command-history
replacement, and not a general animation framework for the whole app.

---

## Non-Goals

- replacing reducer-owned graph mutation history
- making all lens switching continuously interpolated by default
- inventing a separate persistence model outside the shared layout state carrier
- building full cinematic camera choreography or free-camera 3D animation

---

## Feature Target 1: Define the Position Snapshot Model

### Target 1 Context

Morphing and spatial undo both need one canonical, view-owned position snapshot format. Without
that, every layout switch or bulk move will invent its own transient buffer shape.

### Target 1 Tasks

1. Define a bounded `PositionSnapshot` model keyed by node identity and scoped to `GraphViewId`.
2. Separate the spatial snapshot from graph topology mutations and other non-spatial view state.
3. Keep the snapshot carrier compatible with the widened persisted state model from
   `2026-04-03_layout_backend_state_ownership_plan.md`.
4. Establish deterministic capture ordering so snapshots can be compared, restored, and replayed.

### Target 1 Validation Tests

- The same spatial state produces the same snapshot ordering across runs.
- Capturing/restoring a snapshot does not mutate graph truth.
- Per-view snapshots remain isolated across multiple graph views.

---

## Feature Target 2: Land the Transition Engine

### Target 2 Context

The source note sketched the right abstraction: a transition state that can interpolate from one
layout snapshot to another until completion. That behavior should become a first-class view-owned
lane rather than a one-off animation attached to individual layout switches.

### Target 2 Tasks

1. Define a transition carrier that can interpolate between a source `PositionSnapshot` and a
   resolved destination layout state.
2. Restrict the first slice to position interpolation over compatible node sets.
3. Define the completion rule: when the transition ends, the destination layout becomes canonical
   and the transient source snapshot is discarded.
4. Keep transition triggering explicit for layout switches, projection-mode switches, and other
   spatially disruptive operations.

### Target 2 Validation Tests

- Transition playback converges deterministically to the destination state.
- Cancelled or interrupted transitions degrade to a safe resolved state.
- Transition state does not become the long-term source of truth after completion.

---

## Feature Target 3: Define Spatial Undo/Redo Semantics

### Target 3 Context

The source note also called out a separate bounded ring buffer for layout-destructive operations.
That is distinct from graph mutation undo and should stay view-owned.

### Target 3 Tasks

1. Define which operations push a spatial-history entry: layout switches, bulk moves, commit-divergent style operations, and other explicitly spatial actions.
2. Keep the first slice as a bounded ring buffer with predictable memory cost.
3. Separate spatial undo from graph-topology undo so the user can restore positions without
   rewinding graph truth.
4. Decide which parts of the history buffer survive snapshot persistence and which degrade to the
   current view state only.

### Target 3 Validation Tests

- Undoing a spatial operation restores node positions without rewinding graph mutations.
- History capacity stays bounded and old entries evict deterministically.
- Restoring from spatial history remains view-scoped rather than global.

---

## Feature Target 4: Integrate With Variant And Projection Lanes

### Target 4 Context

This lane is only useful if other extracted plans can depend on it instead of reinventing their own
transition behavior.

### Target 4 Tasks

1. Make `2026-04-03_layout_variant_follow_on_plan.md` depend on this lane for layout-switch
   transitions.
2. Make `2026-04-03_twod_twopointfive_isometric_plan.md` depend on this lane for projection-mode
   continuity.
3. Keep the contract narrow enough that guest layouts from
   `2026-04-03_wasm_layout_runtime_plan.md` can opt in later without changing the host-owned
   transition/history model.
4. Expose diagnostics that distinguish instantaneous switches from animated transitions and from
   spatial-history restore operations.

### Target 4 Validation Tests

- Native layout switches can use the shared transition path.
- Projection-mode transitions preserve selection continuity and 2D truth.
- Diagnostics can report whether the current spatial state came from a live layout step, a
  transition, or a history restore.

---

## Exit Condition

This plan is complete when Graphshell has a bounded, per-view spatial snapshot model that supports
deterministic layout/projection transitions and view-owned spatial undo/redo without conflating any
of that behavior with graph-topology history.
