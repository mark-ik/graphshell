<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# GraphCanvas Crate Plan (2026-04-11)

**Status**: Strategy — pre-implementation
**Scope**: Phased extraction of Graphshell's product-owned graph canvas into a
`graph-canvas` crate, aligned with the Vello scene canvas architecture and the
existing custom-canvas migration direction.

**Related**:

- `../../technical_architecture/graph_canvas_spec.md` — crate API design
- `../../technical_architecture/graph_tree_spec.md` — sibling portable tree subsystem
- `../../research/2026-02-27_egui_wgpu_custom_canvas_migration_requirements.md`
- `../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`
- `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`
- `2026-04-03_twod_twopointfive_isometric_plan.md`
- `2026-04-02_scene_mode_ux_plan.md`
- `2026-04-02_parry2d_scene_enrichment_plan.md`
- `GRAPH.md`

---

## 1. Why Now

Graphshell now has:

- explicit per-view `SceneMode`,
- explicit `ViewDimension` with `TwoPointFive` and `Isometric`,
- a growing Parry-backed scene runtime,
- `egui_wgpu` in place,
- and an active Vello-first scene/world rendering direction.

At the same time, the graph canvas is still spread across:

- `render/mod.rs`,
- `render/canvas_camera.rs`,
- `render/canvas_input.rs`,
- `render/canvas_visuals.rs`,
- `render/canvas_overlays.rs`,
- `render/spatial_index.rs`,
- `graph/scene_runtime.rs`.

That is enough product-owned logic that the canvas now deserves the same kind
of subsystem extraction that `graph-tree` is pursuing for the tile/tree layer.

---

## 2. What the Crate Should Absorb

### Phase-in targets from the current codebase

`graph-canvas` should absorb:

- camera state transforms and projection math from the current camera path,
- hover/selection/lasso and graph-surface click grammar from the current input path,
- viewport culling, visible-node filtering, and packet-derivation logic from
  the current visuals path,
- hit-test proxy and spatial-query logic from the current spatial helpers,
- scene packet derivation and scene-runtime bridging from the current graph/scene path.

### What stays outside

- reducer-owned app mutations,
- graph truth,
- workbench/tree layout,
- viewer composition,
- host-specific egui panel/chrome code.

The crate should emit typed packets and actions, not mutate app state directly.

---

## 3. Target Crate Shape

Create a new workspace member:

```text
crates/graph-canvas
```

Initial module intent:

- `scene`
- `camera`
- `projection`
- `interaction`
- `hit_test`
- `lod`
- `packet`
- `diagnostics`
- `backend`
- optional `host_egui`
- optional `backend_vello`

The first public seam should be:

- derived `CanvasSceneInput`
- derived `ProjectedScene`
- typed `CanvasAction`
- backend trait / capability model

---

## 4. Phased Implementation

### Phase 0: Crate scaffold + pure types

**Goal**: `graph-canvas` compiles with portable types and no host/backend lock-in.

- add workspace member
- define core geometry, viewport, camera, projection, packet, and action types
- add serde and construction tests where appropriate
- keep zero egui dependency in the portable core if feasible

**Done gate**: the crate builds and can serialize/construct the core packet and
action types.

### Phase 1: Packet derivation and projection

**Goal**: current graph-view rendering inputs can be converted into a portable
scene packet.

- extract scene derivation from the current `render/canvas_visuals.rs` +
  `render/mod.rs` path
- add `TwoD`, `TwoPointFive`, and `Isometric` projection functions
- preserve canonical `(x, y)` positions and derived `z` policy
- publish degraded-mode diagnostics for unsupported projection paths

**Done gate**: the crate can derive a deterministic `ProjectedScene` for the
same graph/view inputs.

### Phase 2: Interaction and hit testing

**Goal**: the current graph-surface interaction grammar is available through the
crate rather than the widget.

- extract hover/select/drag/lasso logic from `render/canvas_input.rs`
- extract camera gesture logic from `render/canvas_camera.rs`
- move hit-proxy and spatial-query contracts into the crate
- keep reducer mutation outside the crate; emit typed `CanvasAction`s

**Done gate**: host code can feed pointer/keyboard/wheel events into
`graph-canvas` and receive typed canvas actions plus updated interaction state.

### Phase 3: egui host bridge

**Goal**: Graphshell can host the new canvas in the current egui shell without
rewriting chrome.

- add an egui-facing adapter module or thin in-repo bridge
- wire pane rects, viewport input, and paint callback integration
- keep existing graph panels and overlays working around the new canvas seam

**Done gate**: a graph view can be hosted through the new canvas seam while the
rest of the shell remains egui-driven.

### Phase 4: Vello backend

**Goal**: Vello becomes the world renderer for the custom canvas.

- add backend trait implementation for Vello
- route projected graph and scene world rendering through the Vello backend
- preserve egui ownership for chrome and debug/editor overlays
- add diagnostics for backend capability and degraded fallback

**Done gate**: `TwoD`, `TwoPointFive`, and `Isometric` world rendering can all
run through Vello without changing graph semantics.

### Phase 5: Scene integration

**Goal**: authored scene composition and package-backed assets flow cleanly
through the canvas crate.

- consume scene-view composition state from the anchored scene program
- integrate scene objects, node-avatar bindings, and scene-package assets into
  packet derivation
- integrate Parry-backed projected hit proxies and editor geometry queries

**Done gate**: `Browse` and `Arrange` can use scene composition and projected
hit testing without requiring Rapier.

### Phase 6: Rapier `Simulate`

**Goal**: `Simulate` mode can layer Rapier onto the same canvas seam.

- ingest Rapier-derived body transforms and contacts as runtime inputs
- map node avatars and scene props into the projected packet
- preserve graph truth and view isolation

**Done gate**: `Simulate` activates Rapier through scene/runtime inputs, not by
replacing the canvas contract.

### Phase 7: Wasmtime scripting hooks

**Goal**: scene objects and avatars can react through capability-based scripts.

- expose script-driven presentation and interaction hooks through packet/runtime inputs
- do not let the scripting layer mutate graph truth directly
- publish capability-blocked diagnostics

**Done gate**: Wasmtime scene objects can participate in the same canvas packet
and interaction model.

---

## 5. Code Impact Map

### Primary extraction sources

- [mod.rs](C:/Users/mark_/Code/source/repos/graphshell/render/mod.rs)
- [canvas_camera.rs](C:/Users/mark_/Code/source/repos/graphshell/render/canvas_camera.rs)
- [canvas_input.rs](C:/Users/mark_/Code/source/repos/graphshell/render/canvas_input.rs)
- [canvas_visuals.rs](C:/Users/mark_/Code/source/repos/graphshell/render/canvas_visuals.rs)
- [canvas_overlays.rs](C:/Users/mark_/Code/source/repos/graphshell/render/canvas_overlays.rs)
- [spatial_index.rs](C:/Users/mark_/Code/source/repos/graphshell/render/spatial_index.rs)
- [scene_runtime.rs](C:/Users/mark_/Code/source/repos/graphshell/graph/scene_runtime.rs)

### Thin host bridges after extraction

- Graphshell UI code should become a host/adapter layer that:
  - provides view rects and input events
  - converts `CanvasAction` into reducer-owned intents
  - provides overlay/chrome rendering around the canvas

### Sibling subsystem alignment

- `graph-tree` owns tree/workbench/navigator structure
- `graph-canvas` owns graph-view scene rendering and interaction
- neither crate should own the other's truth

---

## 6. Test Strategy

Add crate-local tests for:

- projection determinism for `TwoD`, `TwoPointFive`, and `Isometric`
- packet derivation stability for the same graph/view inputs
- hover/select/lasso hit-testing behavior
- camera transform roundtrips and zoom/pan continuity
- degradation to `TwoD` when projected paths/backend capabilities are unavailable
- no cross-view contamination in scene-runtime-derived packets

Add host-level integration tests for:

- selection continuity across projection changes
- `Browse`/`Arrange` without Rapier world allocation
- Vello backend activation and fallback diagnostics
- `Simulate` packet derivation from Rapier runtime state

---

## 7. Exit Condition

This plan is in effect when Graphshell has:

- a written `graph-canvas` crate spec,
- an agreed extraction path from the current `render/canvas_*` and
  `graph/scene_runtime.rs` modules,
- and a stable understanding that `graph-canvas` is the portable sibling to
  `graph-tree`, not a catch-all replacement for graph truth or scene-state
  ownership.
