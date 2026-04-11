# Parry2D Scene Enrichment Plan (2026-04-02)

**Status**: In progress
**Priority**: Small focused execution slice
**Goal**: Introduce `parry2d` as Graphshell's first geometry-backed scene-enrichment layer for collision-aware layout, authored static regions, and bounded canvas behavior without introducing a full rigid-body world.

**Execution update (2026-04-02)**:

- `parry2d` is now added to the desktop graph-canvas dependency surface.
- `graph/scene_runtime.rs` exists and currently applies:
  - node-overlap resolution,
  - viewport containment,
  - runtime region effects for `Attractor`, `Repulsor`, `Dampener`, and `Wall`.
- The scene runtime pass is wired after the existing post-physics helper layer in `render/mod.rs`.
- Authored runtime regions and bounds overrides now render as canvas backdrops in `render/canvas_overlays.rs`.
- `GraphViewRuntimeState` now carries per-view `scene_runtimes`.
- `PhysicsProfile` now exposes a first collision policy surface (`node_separation`, `viewport_containment`) used by the new pass.
- `GraphBrowserApp` now exposes view-scoped scene-runtime helpers for:
  - setting/clearing bounds overrides,
  - setting/replacing regions,
  - appending regions,
  - clearing all scene runtime for a view.
- `render/graph_info.rs` now exposes a lightweight scene authoring surface:
  - add `Attractor`, `Repulsor`, `Dampener`, or `Wall Box` regions around the current selection,
  - adopt the current view bounds as a scene boundary,
  - clear the active view's runtime scene.
- Authored scene regions can now be manipulated directly on-canvas:
  - pointer hit-testing prefers the smallest visible authored region under the cursor,
  - clicking a region selects it with view-scoped runtime state,
  - dragging a region reuses the existing graph interaction lifecycle (`set_interacting`) so physics pause/resume behavior stays coherent,
  - background clicks clear authored-region selection just like they clear node selection,
  - selected regions now expose first-pass resize handles:
    - circle regions expose a radius handle,
    - rect regions expose corner resize handles,
    - resize interactions share the same runtime drag lifecycle as region movement.
- The scene quick-actions overlay now includes a first selected-region inspector:
  - rename or clear a region label,
  - toggle visibility,
  - switch effect family between `Attractor`, `Repulsor`, `Dampener`, and `Wall`,
  - tune effect strength/factor for non-wall regions,
  - delete the selected region.
- Scene authoring now has a dedicated summoned surface instead of living only in the tiny graph overlay:
  - `render/panels.rs` exposes a floating `Scene` window that reuses the app's existing transient overlay pattern,
  - the panel is graph-view scoped and retargetable across graph views,
  - the lightweight graph overlay remains as the launcher plus fast authoring shortcuts.
- `GraphViewState` now carries a persisted per-view `SceneMode` (`Browse`, `Arrange`, `Simulate`) scaffold.
- `Arrange` is now the first mode that meaningfully changes scene behavior:
  - scene authoring affordances are foregrounded in the graph overlay and Scene panel,
  - on-canvas authored-region hover/select/drag/resize interactions are gated to `Arrange`,
  - selected regions now expose a canvas-local action strip for the most common gather commands and panel access,
  - `Browse` and `Simulate` currently retain backdrop visibility while keeping authoring quieter.
- `Simulate` now has a first concrete object-legibility slice:
  - per-view `Reveal Nodes` and `Relation X-Ray` toggles persist with `GraphViewState`,
  - the graph overlay and summoned Scene panel expose those controls directly,
  - the canvas now renders node-object halos/labels and a scoped relation x-ray overlay for the hovered or primary selected node.
- `Simulate` now also has first behavior presets:
  - per-view `Float`, `Packed`, and `Magnetic` presets persist with `GraphViewState`,
  - the graph overlay and summoned Scene panel expose those presets directly,
  - the scene-runtime pass now biases node separation, containment feel, and region-effect strength from the active preset without introducing a separate physics world,
  - preset personalities are now visibly distinct: `Float` glides longer and responds more loosely to bounds, `Packed` settles faster with firmer containment, and `Magnetic` keeps moderate coast while letting regions feel strongest.
- `Simulate` now also has a first object-motion slice:
  - active drag deltas for the focused simulate view are captured as short-lived release impulses,
  - released node-objects coast briefly and decay through the existing post-drag release window,
  - the effect stays per-view, runtime-only, and non-canonical.
- Selected regions now expose first semantic `Gather Here` actions in `Arrange`:
  - gather the current view selection into the region,
  - gather the current projection-aware graphlet into the region,
  - gather nodes matching selection-derived classification, tag, registrable-domain, or frame candidates,
  - gather the active graph search result set into the region,
  - gather the current filtered-view node set into the region,
  - packing is stable and pinned nodes are respected.
- The first scene authoring bug surfaced during this pass is fixed:
  - quick-action region creation now uses `get_single_selected_node_for_view(view_id)` instead of the globally focused selection, so multi-view scene authoring targets the correct view.

Still open in this plan:

- persistence,
- richer geometry beyond rect/circle,
- richer region editing/manipulation UI beyond first-pass drag/select/resize/inspect,
- broader semantic gather/sort actions inside `Arrange` beyond the current classification/tag/domain/frame/search/filter set,
- fuller `Simulate`-mode behavior beyond the current legibility/preset/release-coast slice.

**Relates to**:

- `../../research/scene_customization.md`
- `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`
- `2026-02-24_physics_engine_extensibility_plan.md`
- `layout_behaviors_and_physics_spec.md`
- `graph_node_edge_interaction_spec.md`
- `multi_view_pane_spec.md`
- `../system/register/canvas_registry_spec.md`

---

## 1. Purpose

Graphshell's current graph canvas already supports:

- built-in layout dispatch via `ActiveLayout`,
- post-physics helper passes in `graph/physics.rs`,
- frame-affinity backdrops and soft organizational behavior,
- semantic and domain clustering,
- per-view layout ownership.

The next scene-enrichment slice should deepen that system without yet adopting `rapier2d`.

The purpose of this plan is to add a **geometry-aware middle tier**:

- no-overlap node resolution,
- static walls / bounded canvas behavior,
- authored regions with geometry and simple scene effects,
- collision-aware scene queries,
- optional label/region geometry support later,

while preserving current authority rules:

- graph remains canonical,
- view state remains explicit,
- scene state remains per-view,
- style remains representational.

**Architecture alignment (2026-04-10)**:

- the broader scene program is now anchored in
  `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`,
- `parry2d` remains the geometry/query/editor layer within that architecture,
- `rapier2d` is explicitly a later `SceneMode::Simulate` physics-world layer,
  not a replacement for this slice,
- the current runtime-region work should be treated as the foundation for
  future collider authoring, projected hit proxies, route editing, and
  package-backed geometry assets.

---

## 2. Scope and Non-Goals

### In scope

This plan covers a first `parry2d`-backed runtime scene layer for graph views:

1. Add `parry2d` as a geometry/query dependency.
2. Introduce per-view runtime scene helpers for authored static regions and collision policy.
3. Add collision-aware post-layout resolution for node overlap and simple wall containment.
4. Add geometry-backed authored scene regions with soft behavioral effects.
5. Route all of the above through existing canvas/layout ownership rather than a new physics engine.

### Explicit non-goals

This slice does **not** include:

- `rapier2d`,
- rigid-body simulation,
- bounce,
- sliding friction,
- springs or joints,
- scene-file persistence,
- external import/export,
- a new `ActiveLayoutKind`,
- a separate full canvas-editor mode or render stack,
- fluid simulation (`salva2d`),
- graph-canonical scene authority.

This is a runtime-only, geometry-assisted enrichment slice.

It is also explicitly **not** the owner of:

- live rigid-body simulation,
- bounce/sliding/joint behavior,
- scene-script execution,
- or the world-render stack itself.

Those responsibilities belong to later layers in the anchored scene program.

---

## 3. Canonical Integration Strategy

### 3.1 Architectural position

`parry2d` should be integrated as a **geometry and scene-query helper**, not as a replacement layout engine.

The canonical first-slice architecture is:

1. Existing `ActiveLayout` variant runs (`ForceDirected` or `BarnesHut`).
2. `render/mod.rs` reads back the updated layout state as it does today.
3. Graphshell applies a new ordered set of `parry2d`-backed post-layout scene helpers.
4. Those helpers update node projected positions in graph/runtime state.
5. `egui_graphs` remains the render surface.

That means `parry2d` is introduced through the existing `apply_graph_physics_extensions(...)` seam or a sibling helper layer immediately adjacent to it.

### 3.2 Why this is the right first step

This keeps the change small and aligned with current code:

- no new render stack,
- no new world/substrate ownership model,
- no `ActiveLayoutState` redesign,
- no simulation-vs-layout branching yet,
- no second scene persistence system yet.

It also gives Graphshell immediate high-value behavior:

- packed, stable "solid" layouts,
- no-overlap node resolution,
- bounded/walled scene behavior,
- region geometry for future scene tools.

---

## 4. First-Slice Runtime Model

### 4.1 Per-view runtime scene state

Add a runtime-only scene helper state owned per `GraphViewId`.

Conceptually:

```rust
struct GraphViewSceneRuntime {
    regions: Vec<SceneRegionRuntime>,
    collision_policy: SceneCollisionPolicy,
}
```

This state is:

- per-view,
- runtime-only in this slice,
- not graph-canonical,
- not serialized in snapshots yet.

### 4.2 Runtime region model

The first region type should be intentionally narrow:

```rust
enum SceneRegionShape {
    Circle { center: Pos2, radius: f32 },
    Rect { rect: egui::Rect },
}

enum SceneRegionEffect {
    Attractor { strength: f32 },
    Repulsor { strength: f32 },
    Dampener { factor: f32 },
    Wall,
}

struct SceneRegionRuntime {
    id: SceneRegionId,
    label: Option<String>,
    shape: SceneRegionShape,
    effect: SceneRegionEffect,
    visible: bool,
}
```

This slice should **not** attempt polygons, arbitrary paths, or editor-authored splines yet.

### 4.3 Collision policy

Use one explicit runtime policy object:

```rust
struct SceneCollisionPolicy {
    node_separation_enabled: bool,
    wall_containment_enabled: bool,
    node_padding: f32,
    region_effect_scale: f32,
}
```

Keep the first slice intentionally simple:

- node-vs-node overlap resolution,
- node-vs-wall containment,
- region-effect strength scaling,
- pinned nodes opt out of displacement.

---

## 5. Behavior Contracts

### 5.1 Node-overlap resolution

After the primary layout step:

- build simple collision proxies for visible graph nodes using projected positions and display radius,
- use `parry2d` queries to detect overlapping node proxies,
- resolve overlaps iteratively by pushing non-pinned nodes apart,
- skip pinned nodes as movable participants,
- clamp displacement per frame to avoid explosive jumps.

Expected result:

- layouts retain their current topology-aware shape,
- but nodes settle into clearer packed arrangements instead of visually overlapping.

### 5.2 Wall / bounded scene containment

Add a bounded-scene option using simple static geometry:

- viewport or authored wall regions are treated as containment geometry,
- nodes that drift outside are projected back inside,
- the correction runs as a post-layout scene rule, not a camera rule.

Expected result:

- graphs can feel physically bounded without requiring a rigid-body engine,
- the "solid" / "atlas" style presets gain a stronger sense of place.

### 5.3 Authored static region effects

Scene regions participate as simple query-backed effects:

- `Attractor`: nudges nodes inward when inside or near the region,
- `Repulsor`: pushes nodes outward,
- `Dampener`: reduces displacement for nodes within the region,
- `Wall`: acts as static geometry for containment/exclusion.

These are **scene rules**, not graph truth and not rigid-body simulation.

### 5.4 Ordering

The first-slice execution order must be deterministic:

1. primary layout (`ActiveLayout`)
2. current post-layout semantic/domain/frame-affinity extensions
3. `parry2d` collision / wall / region passes
4. final position writeback

This prevents the new scene layer from silently replacing the current physics semantics.

---

## 6. File and Module Plan

### 6.1 Dependency

Add `parry2d` to the same target/dependency tier that already owns graph-canvas desktop behavior, alongside the existing `egui_graphs` / `petgraph` integration surface.

### 6.2 New module boundary

Introduce a dedicated graph-side scene helper module, for example:

- `graph/scene_runtime.rs`

Responsibilities:

- runtime scene-region and collision-policy types,
- `parry2d` collider/query helpers,
- overlap resolution,
- wall containment,
- region effect application.

### 6.3 Existing integration points

Expected touched integration points:

- `graph/physics.rs`
  - wire the new post-layout scene helper pass
- `app/workspace_state.rs`
  - add per-view runtime scene state carrier
- `render/mod.rs`
  - invoke the scene helper pass in the existing layout/render orchestration

No new layout enum variant is added in this slice.

---

## 7. State and Authority Rules

### 7.1 Graph authority

Graph remains canonical for:

- node identity,
- edge identity,
- tags,
- memberships,
- relations,
- semantic metadata.

### 7.2 View/runtime authority

The new region and collision state is view-owned runtime state only.

It may affect:

- projected positions,
- scene backdrops,
- per-view layout interpretation.

It must **not**:

- mutate graph-canonical semantics,
- introduce a parallel membership system,
- redefine frame/workbench authority.

### 7.3 Persistence

There is **no persistence in this slice**.

If users can create runtime regions during experimentation, those regions are ephemeral and vanish on restart until a later persistence plan is written.

---

## 8. Testing and Acceptance Criteria

### 8.1 Core tests

Add focused unit/headless tests for:

1. **Node separation**
   - two overlapping nodes end a pass farther apart than they began
2. **Pinned node behavior**
   - pinned nodes do not move during overlap resolution
3. **Wall containment**
   - nodes outside a bounded rect are moved back inside
4. **Region attractor**
   - a node inside an attractor region moves toward the intended target area
5. **Region repulsor**
   - a node inside a repulsor region moves away from the region center
6. **Per-view isolation**
   - scene runtime state for one graph view does not affect another view

### 8.2 Acceptance behavior

The slice is complete when:

- `parry2d` is integrated without disturbing the existing layout dispatch model,
- graph nodes no longer visually overlap under the new collision policy,
- bounded scenes/walls work in a deterministic post-layout pass,
- simple runtime-authored regions affect node motion as specified,
- graph-canonical state remains unchanged except for ordinary projected-position updates,
- no persistence or second scene authority is accidentally introduced.

---

## 9. Follow-On Boundary

If this slice lands cleanly, the next decision is **not** automatically "add Rapier."

The next checkpoint should evaluate:

- whether `parry2d` + existing layout/runtime logic already satisfies the desired "solid" and scene-organization experience,
- whether users now need true physical response such as bounce, sliding, or joints,
- whether authored region UX is compelling enough to justify persistence and editor surfaces.

Only if those needs are concrete should Graphshell advance to the `rapier2d` scene-mode plan.

When that follow-on does occur, this plan's intended carry-forward value is:

- Parry-backed collider/query infrastructure for editor tooling,
- projected hit proxies for `TwoPointFive` and `Isometric`,
- route/path geometry editing,
- and package-backed geometry assets that remain useful even when Rapier is
  active.

**Shared acceptance shape with the anchored scene program**:

- missing scene-package geometry assets degrade safely with diagnostics,
- `Browse` and `Arrange` can rely on Parry/Vello without Rapier world
  allocation,
- `Simulate` may later layer Rapier on top of Parry-owned authoring/query
  structures without mutating graph topology.
