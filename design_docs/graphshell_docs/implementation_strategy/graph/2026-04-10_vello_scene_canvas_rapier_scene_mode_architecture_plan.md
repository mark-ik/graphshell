<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Vello Scene Canvas + Rapier Scene Mode Architecture Plan (2026-04-10)

**Status**: Active architecture and execution anchor
**Scope**: Define the canonical scene-substrate architecture for projected graph rendering, authored scene composition, `Simulate`-mode physics, and capability-based scene scripting.
**Related**:

- `../../technical_architecture/graph_canvas_spec.md`
- `2026-04-02_scene_mode_ux_plan.md`
- `2026-04-02_parry2d_scene_enrichment_plan.md`
- `2026-04-03_twod_twopointfive_isometric_plan.md`
- `2026-04-11_graph_canvas_crate_plan.md`
- `2026-02-24_physics_engine_extensibility_plan.md`
- `view_dimension_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `GRAPH.md`
- `../../research/scene_customization.md`
- `../../research/2026-03-01_webrender_wgpu_renderer_research.md`
- `../../research/2026-03-01_servo_script_engine_alternatives.md`

---

## 1. Purpose

Graphshell now has enough neighboring plans that the intended scene direction
needs one explicit architectural anchor.

This document defines that anchor.

For naming clarity, this plan treats the future custom-canvas subsystem/crate
as **`graph-canvas`**.

`graph-canvas` is the product-owned canvas layer that should carry:

- scene derivation,
- camera/projection rules,
- interaction and hit-testing contracts,
- render-packet derivation,
- backend selection,
- and canvas diagnostics.

It is intentionally broader than a hypothetical `graph-render` crate name,
which would understate the interaction and projection responsibilities already
called for by the custom-canvas research.

The canonical direction is:

- `TwoPointFive` and `Isometric` are first-class projected view modes over
  canonical 2D layout truth.
- `Vello` is the intended primary world renderer for projected graph and scene
  rendering.
- `Parry2D` is the geometry/query/editor layer.
- `Rapier2D` is activated only for `SceneMode::Simulate`.
- `Wasmtime` is the scripting substrate through an explicit capability/event
  API.
- scene packages are part of the first scene-program milestone.

This plan does **not** authorize full `Standard` 3D in the current milestone.
`Standard` remains architecture-only until a separate 3D renderer/camera
program is written.

---

## 2. Authority Model

Graphshell should keep the graph/scene/style/runtime split explicit.

### 2.1 Graph truth

Graph remains canonical for:

- node and edge identity,
- topology and relation semantics,
- tags, memberships, provenance, and semantic metadata,
- canonical `(x, y)` graph layout truth.

Scene work must not create a parallel graph authority.

### 2.2 View-owned scene composition

Scene composition is per-view state.

It includes:

- scene mode,
- view dimension,
- scene package references,
- node-avatar bindings,
- authored scene objects,
- routes, triggers, and scene-local configuration.

This state belongs with `GraphViewId`-scoped interpretation, not with the graph
itself.

### 2.3 Style and rendering

Style remains representational.

It includes:

- node avatar visuals,
- scene prop visuals,
- shadows, depth cues, and world labels,
- background/environment presentation.

Changing style must not change graph truth.

### 2.4 Runtime-only state

Derived runtime state stays non-canonical.

It includes:

- derived `z` positions from `ZSource`,
- projected draw packets,
- Parry query structures,
- Rapier body state and contacts,
- live script instance state.

This state may be rebuilt from persisted inputs and must not silently become
the durable truth of the graph.

---

## 3. Canonical Scene Stack

The scene substrate should be expressed through a small set of explicit types.

### 3.1 Persisted view-owned state

```rust
struct SceneViewState {
    mode: SceneMode,
    renderer: SceneRendererMode,
    physics: ScenePhysicsMode,
    environment: SceneEnvironment,
    packages: Vec<ScenePackageRef>,
    node_avatars: HashMap<NodeKey, NodeAvatarBinding>,
    scene_objects: HashMap<SceneObjectId, SceneObject>,
    scripts: HashMap<SceneScriptId, SceneScriptBinding>,
}
```

This carrier is persisted with the owning graph view snapshot.

`graph-canvas` should consume this persisted scene/view composition state rather
than owning a second app model.

### 3.2 Runtime-owned state

```rust
struct SceneRuntimeState {
    projected_scene: ProjectedScene,
    parry_queries: SceneQueryState,
    rapier_world: Option<RapierSceneWorld>,
    script_runtime: SceneScriptRuntime,
    event_buffer: Vec<SceneEvent>,
}
```

This carrier is runtime-only unless a later snapshot policy explicitly scopes
some subset of it.

This runtime carrier is the natural core state for the future `graph-canvas`
subsystem/crate.

### 3.3 Scene composition types

```rust
struct NodeAvatarBinding {
    node_key: NodeKey,
    preset: AvatarPresetId,
    visual: VisualAssetRef,
    collider: ColliderSpec,
    material: PhysicsMaterial,
    body_kind: SceneBodyKind,
    script: Option<SceneScriptId>,
}

struct SceneObject {
    id: SceneObjectId,
    role: SceneObjectRole,
    transform: SceneTransform,
    visual: VisualAssetRef,
    collider: Option<ColliderSpec>,
    material: Option<PhysicsMaterial>,
    body_kind: SceneBodyKind,
    trigger: Option<TriggerSpec>,
    route: Option<RouteSpec>,
    script: Option<SceneScriptId>,
}

struct AvatarPreset {
    id: AvatarPresetId,
    visual: VisualAssetRef,
    collider: ColliderSpec,
    material: PhysicsMaterial,
    default_body_kind: SceneBodyKind,
}

enum ColliderSpec {
    Circle { radius: f32 },
    Rect { size: Vec2 },
    Capsule { half_height: f32, radius: f32 },
    ConvexHull { points: Vec<Pos2> },
    Compound(Vec<ColliderSpec>),
    FromAlphaMask { texture: TextureAssetId, threshold: f32 },
}

struct PhysicsMaterial {
    density: f32,
    friction: f32,
    restitution: f32,
    linear_damping: f32,
    angular_damping: f32,
    gravity_scale: f32,
}

struct SceneScriptBinding {
    target: ScriptTarget,
    module: SceneScriptModuleId,
    capabilities: Vec<SceneCapability>,
}

struct ProjectedScene {
    background: Vec<SceneDrawItem>,
    world: Vec<SceneDrawItem>,
    overlays: Vec<SceneDrawItem>,
    hit_proxies: Vec<HitProxy>,
}
```

The exact field set may evolve, but the boundary between persisted scene
composition and runtime projection/physics/script state should not.

### 3.4 Scene packages

Scene packages are part of v1.

They should provide stable ids for reusable:

- textures and sprite sheets,
- vector or layered visual assets,
- collider assets,
- avatar presets,
- prop prefabs,
- route/path assets,
- scene scripts,
- optional environment presets.

Scene package import/export should be a scene-asset concern, not a graph
topology concern.

---

## 4. Renderer Ownership

### 4.1 Primary renderer decision

`Vello` is the intended primary world renderer for:

- projected graph nodes and edges,
- node avatar visuals,
- scene props,
- scene backgrounds and environments,
- projected shadows and depth cues,
- world-space labels and decorative affordances.

### 4.2 egui ownership

`egui` remains the owner of:

- app chrome,
- inspectors and panels,
- editor gizmos,
- diagnostics overlays,
- fallback placeholders and low-risk debug surfaces.

`egui` should not remain the long-term owner of the world-render layer once the
Vello path is live.

### 4.3 Shared render packet

Both projected graph rendering and authored scenes should render from a shared
`ProjectedScene` packet rather than directly from raw widget-local state.

That packet is the seam that keeps:

- graph truth,
- projected view modes,
- scene composition,
- and backend selection

decoupled from any one rendering framework.

In crate terms, this packet should be owned by `graph-canvas`, while Vello
should appear either as an internal backend module or a later `graph-canvas-vello`
backend crate if the separation becomes useful.

---

## 5. View Dimension Alignment

`ViewDimension` remains authoritative for `TwoD` vs projected 3D-like modes.

### 5.1 TwoPointFive and Isometric

`TwoPointFive` and `Isometric` are in-scope implementation targets for this
program.

They must:

- preserve canonical `(x, y)` layout truth,
- derive `z` ephemerally from `ZSource`,
- preserve selection/camera continuity,
- degrade deterministically to `TwoD` when unavailable.

They should target the shared projected scene plus Vello world-render path.

### 5.2 Standard 3D

`Standard` remains architecture-only.

This program should define the future seam for a true 3D renderer/camera path
without implementing:

- orbit or arcball camera,
- free reorientation,
- `rapier3d`,
- full 3D mesh/scene rendering.

---

## 6. Physics Ownership

### 6.1 Parry2D responsibilities

`Parry2D` is the geometry/query/editor layer.

It owns:

- picking and hit testing,
- collider authoring/editing,
- projected hit proxies for `TwoPointFive` and `Isometric`,
- region and route geometry queries,
- lightweight overlap/containment helpers,
- spatial queries when no live physics world is active.

### 6.2 Rapier2D responsibilities

`Rapier2D` is the live rigid-body world for `SceneMode::Simulate`.

It owns:

- body simulation,
- contacts and triggers,
- restitution, friction, damping, and gravity behavior,
- dynamic/static/kinematic scene props,
- physically responsive node avatars.

### 6.3 Mode activation boundary

- `Browse`: no Rapier world allocation required
- `Arrange`: no Rapier world allocation required by default
- `Simulate`: Rapier world is active for opted-in scene views

This keeps scene authoring and projected browsing possible without forcing the
entire graph canvas through a rigid-body simulation stack.

---

## 7. Scripting Ownership

`Wasmtime` is the canonical scripting substrate.

v1 should use an explicit capability/event API rather than unrestricted script
execution.

### 7.1 Script targets

Scripts may attach to:

- a node avatar,
- a scene object,
- a scene view.

### 7.2 Event surface

The first event surface should support:

- `Tick`,
- `ContactBegin` / `ContactEnd`,
- `TriggerEnter` / `TriggerExit`,
- pointer/select/focus events,
- route/waypoint events.

### 7.3 Capability surface

The first capability surface should support:

- reading self state,
- reading nearby/query results,
- applying impulses or bounded motion changes,
- setting animation/presentation state,
- emitting UI events,
- following a route.

There is no Boa/JS-first path in this milestone.

---

## 8. Persistence And Degradation

### 8.1 Persisted inputs

Persist with the graph view snapshot:

- `SceneMode`,
- `ViewDimension`,
- scene package refs,
- scene composition,
- node-avatar bindings,
- authored routes/triggers/object configuration.

### 8.2 Runtime-only state

Remain runtime-only by default:

- derived `z`,
- Rapier body state,
- contacts,
- script instance internals,
- generated render packets.

### 8.3 Degradation

Missing scene-package assets, unavailable projection capabilities, or blocked
scene-script capabilities should degrade safely:

- placeholder visuals instead of silent disappearance,
- diagnostics instead of silent failure,
- no cross-pane corruption,
- no mutation of graph topology.

---

## 9. Delivery Phases

### Phase 1: Substrate

- add the canonical scene-state and runtime-state carriers,
- define scene package manifests and ids,
- define the projected-scene render seam,
- publish diagnostics for projection/backend/script degradation.

### Phase 2: Projected Rendering

- land `TwoPointFive` and `Isometric` on the shared projected scene,
- route world rendering through Vello,
- keep selection, camera, and `(x, y)` continuity intact.

### Phase 3: Authoring

- add package-backed avatar and prop assignment,
- add collider authoring/editing,
- add route editing and projected hit proxies,
- keep `Browse` and `Arrange` usable without Rapier.

### Phase 4: Rapier Simulation

- add `Simulate`-mode Rapier world activation,
- map node avatars and scene props into bodies/colliders/materials,
- add triggers and contact-driven behavior.

### Phase 5: Scripting

- add Wasmtime module loading,
- bind scripts to scene targets,
- expose the first event/capability surface for scene objects and avatars.

---

## 10. Shared Acceptance Shape

The neighboring scene/projection plans should all align on the following
acceptance shape:

- projection transitions preserve selection, camera continuity, and canonical
  `(x, y)` positions,
- `TwoPointFive` and `Isometric` derive depth deterministically from `ZSource`,
- missing scene-package assets degrade to placeholders with diagnostics,
- `Browse` and `Arrange` do not require Rapier world allocation,
- `Simulate` enables Rapier behavior without mutating graph topology,
- Wasm scene scripts operate only through explicit capabilities/events,
- scene composition roundtrips through view snapshots while derived runtime
  state does not.

---

## 11. Crate/Subsytem Direction

The intended portable split now looks like:

- `graph-tree`: graphlet-native tree/workbench/navigator structure
- `graph-canvas`: graph-view scene derivation, camera/projection, interaction,
  hit testing, render-packet derivation, backend seam, and diagnostics
- Vello backend: inside `graph-canvas` initially, with a later split to
  `graph-canvas-vello` only if the backend seam becomes independently valuable
- scene/physics/script integrations: plugged into `graph-canvas` through scene
  composition state and runtime contracts rather than replacing it

`graph-canvas` must not become the owner of graph truth, tile-tree truth, or
global application state.

The technical crate API design now lives in
`../../technical_architecture/graph_canvas_spec.md`, and the concrete extraction
strategy now lives in `2026-04-11_graph_canvas_crate_plan.md`.

---

## 12. Exit Condition

This architecture plan is in effect when the neighboring implementation plans
for scene mode, Parry scene enrichment, and projected view dimensions all defer
to it for the multi-layer scene substrate and milestone ordering.
