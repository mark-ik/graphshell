<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# TwoD / TwoPointFive / Isometric Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the projection-mode lane from `2026-02-24_physics_engine_extensibility_plan.md` into a narrower execution plan for `TwoD`, `TwoPointFive`, and `Isometric`, without assuming full reorientable 3D.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-04-03_layout_backend_state_ownership_plan.md`
- `2026-04-03_wasm_layout_runtime_plan.md`
- `2026-04-03_layout_transition_and_history_plan.md`
- `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`
- `view_dimension_spec.md`
- `2026-02-27_viewdimension_acceptance_contract.md`
- `2026-04-02_scene_mode_ux_plan.md`
- `../aspect_render/render_backend_contract_spec.md`

---

## Context

The canonical `ViewDimension` contract currently covers `TwoD` and generalized `ThreeD`, but the
most achievable next lane is narrower:

- preserve 2D layout truth
- derive ephemeral `z` placement from `ZSource`
- add projection/render behavior for `TwoPointFive` and `Isometric`
- keep full free-camera `Standard` 3D as a later, separate problem

This lane depends on the shared persisted carrier described in
`2026-04-03_layout_backend_state_ownership_plan.md`. It does **not** depend on the runtime-loaded
guest layout work in `2026-04-03_wasm_layout_runtime_plan.md`, though projection modes should be
able to consume either built-in or guest-computed 2D layout truth later.

**Architecture alignment (2026-04-10)**:

- the multi-layer scene/render substrate is now defined in
  `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`,
- `TwoPointFive` and `Isometric` remain projection modes over canonical 2D
  layout truth,
- those projection modes now target the shared projected-scene + Vello
  world-render path,
- `Standard` remains architecture-only and is not part of this milestone.

---

## Non-Goals

- a hard requirement to move to wgpu before `TwoPointFive` or `Isometric` can exist
- turning projection modes into a new source of graph truth
- using this plan to solve full free-camera 3D or arbitrary scene navigation
- making runtime-loaded WASM layouts a prerequisite for projection work

---

## Feature Target 1: Narrow the Mode Contract

### Target 1 Context

`TwoPointFive` and `Isometric` should behave as projection modes over 2D layout truth, not as a
different source of graph truth.

### Target 1 Tasks

1. Define `TwoPointFive` and `Isometric` as modes that preserve `(x, y)` layout state.
2. Keep `z` derivation ephemeral and recomputable from `ZSource` plus node metadata.
3. Preserve the per-`GraphViewId` ownership model from `view_dimension_spec.md`.
4. Retain deterministic degradation to `TwoD` if the projection path is unavailable.
5. Route mode-switch animation/history through `2026-04-03_layout_transition_and_history_plan.md`
   rather than inventing projection-specific one-offs.

### Target 1 Validation Tests

- Unsupported projection path degrades to `TwoD` with diagnostics and no cross-pane impact.
- Switching projection modes preserves the underlying 2D layout truth.
- Projection-mode selection/restoration survives snapshot roundtrip through the shared carrier.

---

## Feature Target 2: Land the TwoPointFive Projection Path

### Target 2 Context

`TwoPointFive` is the lowest-risk projection mode because it keeps a fixed camera and a mostly 2D
interaction model while adding depth cues.

### Target 2 Tasks

1. Implement a fixed-camera projection pass for node/edge rendering through the
   shared projected-scene + Vello world-render path.
2. Keep pan/zoom ownership aligned with existing 2D camera behavior.
3. Ensure selection, hover, and hit testing remain continuous under projection.
4. Expose diagnostic evidence for mode enter, mode exit, and degraded fallback.

### Target 2 Validation Tests

- `TwoPointFive` preserves selection and hover continuity.
- Pan/zoom behavior remains aligned with 2D expectations.
- Entering and leaving `TwoPointFive` emits diagnosable mode-resolution evidence.

---

## Feature Target 3: Land the Isometric Layering Path

### Target 3 Context

`Isometric` is more semantically ambitious than `TwoPointFive` but still bounded because layer
placement can remain a deterministic projection of 2D positions plus quantized depth.

### Target 3 Tasks

1. Define layer quantization rules for `ZSource` inputs such as BFS depth, UDC level, or recency.
2. Render the same underlying graph with a fixed isometric projection on the
   shared projected-scene + Vello world-render path.
3. Preserve label, hover, and selection semantics across layer separation.
4. Keep the mode within the current graph-view interaction contract rather than requiring a
   separate 3D navigation model.

### Target 3 Validation Tests

- Layer assignment is deterministic for the same graph state and `ZSource`.
- Isometric mode preserves active selection and focus-routing behavior.
- Returning to `TwoD` discards derived `z` without disturbing `(x, y)` positions.

---

## Feature Target 4: Keep Full 3D as a Separate Escalation Gate

### Target 4 Context

This plan should not quietly grow into a full 3D renderer effort.

### Target 4 Tasks

1. Define the explicit boundary between projected 2.5D / isometric rendering and full 3D scene
   navigation.
2. Keep all mode transitions lossless with respect to underlying 2D layout truth.
3. Require a separate authority before adding orbit, tilt, true 3D camera
   persistence, or a non-projected `Standard` renderer path.

### Target 4 Validation Tests

- Projection modes do not require full 3D navigation semantics.
- Degradation to `TwoD` is explicit and diagnosable.
- Full 3D work can remain deferred without blocking `TwoPointFive` and `Isometric`.

---

## Exit Condition

This plan is complete when Graphshell can render `TwoPointFive` and `Isometric` as stable,
per-view projection modes over the existing 2D layout truth, with deterministic degradation to
`TwoD`, targeting the shared projected-scene + Vello world-render path, and
without pretending that full reorientable 3D is already solved.
