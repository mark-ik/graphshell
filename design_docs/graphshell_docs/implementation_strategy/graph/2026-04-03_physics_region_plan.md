<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Physics Region Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the speculative `PhysicsRegion` / spatial-rule-authoring lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan for authored regions, interaction semantics, persistence boundaries, and view-scope commit behavior.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `layout_behaviors_and_physics_spec.md`
- `2026-04-03_physics_preferences_surface_plan.md`
- `multi_view_pane_spec.md`
- `../workbench/graph_first_frame_semantics_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`

---

## Context

The umbrella physics note contains two different "region" ideas that must not be collapsed:

- **Frame-affinity regions** are already canonical, derived from frame membership, and remain a
  visual/layout projection of `Frame` authority.
- **Physics regions** are a separate speculative authoring lane where a user draws spatial rule
  volumes that affect simulation directly.

That distinction matters. If `PhysicsRegion` revives, it cannot drift back into the old zone model
or compete with frame-affinity as a second organization authority. It needs its own narrow contract:
an authored spatial-rule object with explicit scope, persistence, and commit semantics.

This plan exists to extract that lane before the settings surface or canvas editor start implying
that region authoring is already settled.

---

## Non-Goals

- replacing frame-affinity behavior or rebranding frames as regions
- turning physics regions into graph-truth or arrangement-truth authorities
- requiring rapier as a prerequisite for every first-slice region behavior
- inventing a full general-purpose scene editor before the data and reducer contracts exist
- solving the broader paint-callback or custom-canvas rendering pipeline here

---

## Feature Target 1: Define The PhysicsRegion Authority Boundary

### Target 1 Context

The first missing decision is conceptual: what a physics region is, and what it is not.

### Target 1 Tasks

1. Define `PhysicsRegion` as an authored spatial-rule object, separate from frame-affinity and from
   graph topology.
2. Explicitly contrast authored `PhysicsRegion` objects with derived frame-affinity regions from
   `graph_first_frame_semantics_spec.md`.
3. Decide the first-slice rule vocabulary: gravity well, repulsion field, friction, boundary, and
   filtered variants only if their runtime semantics are concrete.
4. Keep rule semantics reducer-addressable and inspectable instead of storing opaque widget-local
   state.

### Target 1 Validation Tests

- A doc reader can distinguish frame-affinity from `PhysicsRegion` without ambiguity.
- Region rules are named and serializable rather than implied by panel state.
- A region can be disabled or hidden without losing its identity or mutating graph truth.

---

## Feature Target 2: Define The Interaction And Authoring Model

### Target 2 Context

The umbrella note has a draft data model but leaves authoring semantics unresolved.

### Target 2 Tasks

1. Define how a user creates a region: palette tool, draw gesture, or command-driven creation with
   explicit shape parameters.
2. Define the minimum editable shape set for the first slice: circle and rectangle before polygon.
3. Define overlap rules and precedence policy for conflicting regions rather than leaving runtime
   composition to accident.
4. Route all create/update/delete actions through explicit `GraphIntent` variants instead of direct
   canvas mutation.

### Target 2 Validation Tests

- Creating a region produces a durable identity and an inspectable rule payload.
- Overlapping regions compose according to a documented policy.
- Cancelling region creation does not leave behind partial runtime state.

---

## Feature Target 3: Define Scope, Persistence, And Commit Semantics

### Target 3 Context

The hardest unresolved question is where regions live. The umbrella note floated view-local,
workspace, and lens-bound possibilities, but the canonical graph-view model already requires
per-view isolation and explicit commit behavior.

### Target 3 Tasks

1. Define a scope model for regions in terms of `GraphViewId`, workspace snapshot, and optional
   lens-provided defaults.
2. Keep the first slice conservative: decide whether regions are view-local only, explicitly
   persistable, or both.
3. If a divergent view authors regions, define the same explicit commit boundary expected for other
   divergent spatial state.
4. Persist explicit `physics_regions` records if the feature crosses the snapshot boundary; do not
   revive the legacy `zones` carrier.

### Target 3 Validation Tests

- A region authored in one graph view does not leak into a sibling view by default.
- Persisted region state round-trips through snapshot restore without relying on a legacy zone model.
- Divergent-view region edits do not become global until the documented commit path is invoked.

---

## Feature Target 4: Define Runtime Consumption And Degradation Policy

### Target 4 Context

The region model is only worth shipping if its runtime effect is deterministic, bounded, and safe
to degrade when advanced backends are unavailable.

### Target 4 Tasks

1. Define how each first-slice region rule affects the active layout/physics step without requiring
   a full separate simulation engine.
2. Decide whether the first slice is implemented via current post-physics helper injection, a
   rapier-backed lane, or a bounded hybrid.
3. Define visibility separately from simulation effect so a hidden region can still influence
   layout if the user chooses.
4. Define degradation rules when a backend cannot execute a region type: disable with diagnostics,
   downgrade to a simpler helper, or omit from playback/restore explicitly.

### Target 4 Validation Tests

- Region effects are deterministic for identical state and inputs.
- Hidden regions do not silently disable their rule unless the user explicitly turns them off.
- Unsupported region rules degrade with explicit diagnostics rather than silent no-ops.

---

## Feature Target 5: Integrate With Settings And Canvas Editing Without Reopening Ownership

### Target 5 Context

The physics preferences plan should consume this lane, not define it. Likewise, any future canvas
editor should author regions against this contract rather than inventing a second scene model.

### Target 5 Tasks

1. Feed region visibility, scope, and persistence controls back into
   `2026-04-03_physics_preferences_surface_plan.md` as consumers of this plan.
2. Keep the settings page limited to control/inspection flows; direct geometry authoring belongs to
   a dedicated canvas tool or editor path.
3. Define the minimum diagnostics surface for region state: active count, scope, enabled/visible
   status, and unsupported-rule failures.
4. Preserve the distinction between authored physics regions and derived frame-affinity backdrops in
   all user-facing wording.

### Target 5 Validation Tests

- The settings page can inspect and toggle region state without becoming the geometry editor.
- Diagnostics distinguish authored physics regions from frame-affinity behavior.
- User-facing labels never present frame-affinity and `PhysicsRegion` as the same feature.

---

## Exit Condition

This plan is complete when Graphshell has a documented `PhysicsRegion` contract that separates
authored spatial rules from derived frame-affinity regions, defines how regions are created and
composed, establishes `GraphViewId`-aware scope and snapshot semantics, and gives the settings/editor
surfaces a single canonical model to consume.
