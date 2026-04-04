<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Damping Profile Follow-On Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the damping-profile lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan for named damping curves, registry-owned profile references, and explicit settle-shape policy.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `force_layout_and_barnes_hut_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `../system/register/physics_profile_registry_spec.md`
- `2026-04-03_layout_transition_and_history_plan.md`

---

## Context

The umbrella physics note already calls out a missing layer between numeric force presets and the
actual feel of convergence: named damping curves.

That gap matters because Graphshell currently has only one coarse damping parameter surface, while
the desired product behavior is richer:

- some layouts should snap to rest quickly
- some should settle softly
- some should permit a controlled oscillation window before rest

This should not be solved by multiplying named physics presets without structure. Damping curves
are a separate semantic dimension from repulsion/attraction/gravity magnitudes, so they need their
own authority.

---

## Non-Goals

- replacing the current baseline force model with an unrelated animation system
- silently changing the feel of existing presets without explicit profile identity or diagnostics
- using damping curves as a proxy for layout algorithm selection
- expanding this lane into a general render-motion or camera-easing policy

---

## Feature Target 1: Define the Damping Curve Contract

### Target 1 Context

The first missing decision is what a damping profile actually is: a stable, serializable named
curve shape distinct from raw parameter magnitude.

### Target 1 Tasks

1. Define the initial damping curve family set: linear, exponential, spring, and critically damped.
2. Separate curve identity from raw `damping` magnitude so profiles can express both shape and
   strength without ambiguity.
3. Define a serializable curve carrier that can be referenced by `PhysicsProfile.damping_profile_id`.
4. Keep curve IDs stable and diagnosable rather than tied to Rust enum names.

### Target 1 Validation Tests

- Damping curve definitions roundtrip through serialization without losing identity.
- Different curve IDs can share the same raw magnitude while producing distinct settle behavior.
- Missing or unknown curve IDs degrade through a documented fallback path.

---

## Feature Target 2: Integrate With Physics Profile Registry Semantics

### Target 2 Context

Named profile selection already belongs to `physics_profile_registry`. Damping curves should extend
that semantic surface rather than becoming widget-local hidden state.

### Target 2 Tasks

1. Define how `PhysicsProfile` references an optional damping curve ID.
2. Keep registry lookup deterministic and overrideable under the existing profile-registry rules.
3. Ensure damping-curve selection is visible to shared settings and diagnostics surfaces, not just
   the canvas.
4. Preserve fallback safety when a user or mod-supplied profile references an unavailable curve.

### Target 2 Validation Tests

- Registry lookup resolves the same damping curve deterministically for the same profile ID.
- An unavailable damping curve falls back without breaking profile resolution.
- Shared settings and diagnostics can report the active damping curve identity.

---

## Feature Target 3: Define Execution And Settle Semantics

### Target 3 Context

Damping curves only matter if their runtime effect is explicit. This lane needs to define what
they actually influence: convergence feel, oscillation budget, and rest behavior under the
existing force-layout contract.

### Target 3 Tasks

1. Define how each curve shape affects energy dissipation over time within the baseline force
   layout path.
2. Keep the influence bounded so damping curves do not silently invalidate existing reheat or
   stability guarantees.
3. Define how damping curves interact with auto-pause, convergence thresholds, and repeated
   stepping.
4. Keep damping curves compatible with later layout transitions rather than fighting them.

### Target 3 Validation Tests

- Different damping profiles produce measurably different settle trajectories.
- Damping curves remain bounded under repeated stepping and do not destabilize pinned-node
  invariants.
- Convergence and auto-pause behavior remain explicit and testable.

---

## Feature Target 4: Make The Results User-Visible And Explainable

### Target 4 Context

If damping curves change how the graph feels, that difference must be visible and attributable. A
user should not need to infer curve shape from motion alone.

### Target 4 Tasks

1. Expose active damping curve identity through diagnostics alongside the active physics profile.
2. Provide clear semantic descriptions for the first shipped curves such as snap-to-rest, soft
   settle, or oscillate-to-rest.
3. Ensure lens/profile binding surfaces can explain when a damping curve changed because the active
   profile changed.
4. Keep advanced curve selection optional rather than forcing raw control surfaces into the first
   product slice.

### Target 4 Validation Tests

- Diagnostics distinguish raw damping magnitude from damping curve identity.
- Profile changes that alter the damping curve are inspectable and explainable.
- Users can reason about why one preset settles differently from another without reading code.

---

## Exit Condition

This plan is complete when Graphshell has a named, serializable damping-curve surface that is
referenced by physics profiles, resolved deterministically by registry policy, applied explicitly in
the force-layout engine, and exposed through diagnostics and user-facing profile semantics.
