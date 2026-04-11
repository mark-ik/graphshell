# GraphCanvas Specification

**Date**: 2026-04-11
**Status**: Architecture design — pre-implementation
**Scope**: API design for the `graph-canvas` crate — a framework-agnostic,
Graphshell-owned custom canvas subsystem for scene derivation, camera and
projection, interaction and hit testing, render-packet derivation, and backend
selection.

**Related**:

- `graph_tree_spec.md` — graphlet-native tree/workbench structure
- `unified_view_model.md` — domain ownership model
- `../implementation_strategy/graph/GRAPH.md` — Graph domain canvas ownership
- `../implementation_strategy/graph/view_dimension_spec.md` — `ViewDimension`,
  `ThreeDMode`, `ZSource`
- `../implementation_strategy/graph/2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`
- `../implementation_strategy/graph/2026-04-11_graph_canvas_crate_plan.md`
- `../implementation_strategy/graph/2026-04-02_scene_mode_ux_plan.md`
- `../implementation_strategy/graph/2026-04-02_parry2d_scene_enrichment_plan.md`
- `../research/2026-02-27_egui_wgpu_custom_canvas_migration_requirements.md`
- `../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`

---

## 1. Why `graph-canvas`

Graphshell's future canvas is not merely a renderer.

It must own:

- scene derivation,
- camera and projection rules,
- interaction and hit testing,
- LOD and culling policy,
- render-packet derivation,
- backend selection and degraded-mode diagnostics.

That is why the subsystem should be named **`graph-canvas`**, not
`graph-render`.

`graph-render` would imply a narrower responsibility centered on drawing.
Graphshell needs a portable canvas core that can describe what should be drawn,
how it is interacted with, and how it degrades, before any backend-specific
painting happens.

---

## 2. What GraphCanvas Replaces

`graph-canvas` replaces the current `egui_graphs`-centric graph widget path as
the product-owned graph surface.

It should absorb the current mixed ownership spread across:

- `render/mod.rs`,
- `render/canvas_camera.rs`,
- `render/canvas_input.rs`,
- `render/canvas_visuals.rs`,
- `render/canvas_overlays.rs`,
- `render/spatial_index.rs`,
- and the graph-view scene runtime bridge in `graph/scene_runtime.rs`.

### What it does replace

- widget-local graph rendering as the primary abstraction
- widget-local hit testing and hover/selection resolution
- implicit projection behavior hidden in the current adapter path
- backend-specific draw assumptions leaking into graph-space semantics

### What it does not replace

- graph truth
- `graph-tree`
- workbench/tree layout
- the three-pass compositor contract
- viewer content rendering

`graph-canvas` is the graph-view scene and interaction surface, not a second
application model.

---

## 3. Relationship to GraphTree

`graph-tree` and `graph-canvas` are sibling portable subsystems.

### `graph-tree`

Owns:

- graphlet-native tree structure,
- layout projection for workbench/navigator,
- focus/expansion/session tree semantics.

### `graph-canvas`

Owns:

- graph-view scene derivation,
- camera and projection,
- interaction and hit testing,
- render packets and backend seam.

### How they meet

`graph-tree` determines where graph views and panes live.
`graph-canvas` renders and interacts with the graph inside a graph-view pane.

The split should be as clean as:

- `graph-tree` answers "which view/pane/tree member is active and where is it"
- `graph-canvas` answers "what does this graph view look and feel like inside
  its rect"

---

## 4. Framework-Agnostic by Construction

The core `graph-canvas` crate should be framework-agnostic.

That means the core should avoid direct dependence on:

- egui widgets,
- winit windowing,
- Vello command submission,
- host-specific accessibility adapters.

The core crate should instead define:

- portable geometry and viewport types,
- canvas input/event types,
- projected scene packets,
- hit-test/query contracts,
- backend traits and degraded-mode signaling.

Framework and backend bindings can start inside the crate as modules, but the
public contract must remain portable.

Likely adapters:

- `graph-canvas-egui`: host bridge for egui input/output and paint callback wiring
- `graph-canvas-vello`: world-render backend
- future `graph-canvas-web`: DOM/WebGPU host bridge

The Vello backend may live inside `graph-canvas` initially, but it should be
designed as a separable backend from day one.

---

## 5. Crate API Design

### 5.1 Crate identity

```
graph-canvas/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── scene.rs         # scene derivation inputs and scene packets
│   ├── camera.rs        # camera state and transforms
│   ├── projection.rs    # TwoD / TwoPointFive / Isometric projection rules
│   ├── interaction.rs   # hover, selection, drag, lasso, click grammar
│   ├── hit_test.rs      # hit proxies and query API
│   ├── lod.rs           # LOD and culling policy
│   ├── packet.rs        # ProjectedScene and draw items
│   ├── diagnostics.rs   # degraded mode and performance channels
│   ├── backend.rs       # backend traits and backend selection
│   ├── host_egui.rs     # optional egui host adapter
│   └── backend_vello.rs # optional Vello backend
```

The exact module layout may vary, but the boundary should hold:

- core canvas logic stays separate from host-framework glue,
- backend-specific rendering stays behind an explicit seam.

### 5.2 Core types

```rust
pub struct CanvasViewport {
    pub rect: Rect,
    pub scale_factor: f32,
}

pub struct CanvasCamera {
    pub pan: Vec2,
    pub zoom: f32,
}

pub enum ProjectionMode {
    TwoD,
    TwoPointFive { z_source: ZSource },
    Isometric { z_source: ZSource },
    Standard, // architecture-only for now
}

pub struct CanvasSceneInput<N> {
    pub view_id: ViewId,
    pub graph_nodes: Vec<CanvasNode<N>>,
    pub graph_edges: Vec<CanvasEdge<N>>,
    pub scene_objects: Vec<CanvasSceneObject>,
    pub overlays: Vec<CanvasOverlayItem>,
    pub scene_mode: SceneMode,
    pub projection: ProjectionMode,
}

pub struct ProjectedScene<N> {
    pub background: Vec<SceneDrawItem>,
    pub world: Vec<SceneDrawItem>,
    pub overlays: Vec<SceneDrawItem>,
    pub hit_proxies: Vec<HitProxy<N>>,
}

pub struct InteractionState<N> {
    pub hovered_node: Option<N>,
    pub hovered_edge: Option<EdgeRef<N>>,
    pub selection: HashSet<N>,
    pub lasso: Option<LassoState>,
}

pub enum CanvasAction<N> {
    HoverNode(N),
    SelectNode(N),
    DragNode { node: N, to: Pos2 },
    LassoSelect { nodes: Vec<N> },
    PanCamera(Vec2),
    ZoomCamera(f32),
}
```

These are intentionally canvas-facing derived carriers, not graph-domain truth.

### 5.3 Backend contract

```rust
pub trait CanvasBackend {
    type FrameHandle;

    fn prepare(&mut self, scene: &ProjectedScene<NodeKey>, viewport: CanvasViewport);
    fn render(&mut self, frame: &mut Self::FrameHandle);
    fn capabilities(&self) -> CanvasBackendCapabilities;
}
```

The backend must consume `ProjectedScene`; it must not define graph semantics.

---

## 6. Scene Derivation Contract

`graph-canvas` should own scene derivation as a pure transformation from
Graphshell-owned state into a `ProjectedScene`.

That derivation must:

- preserve canonical `(x, y)` graph positions,
- derive `z` ephemerally from `ZSource`,
- fold in scene-view composition state,
- fold in scene-runtime overlays and hit proxies,
- produce deterministic draw ordering for the same inputs.

This is the seam that lets:

- 2D,
- 2.5D,
- isometric,
- Vello-backed scene rendering,
- Parry-backed hit testing,
- and later Rapier-backed `Simulate`

share one product-owned canvas contract.

---

## 7. Camera and Projection Contract

`graph-canvas` owns camera and projection rules.

### 7.1 TwoD

- orthographic
- current pan/zoom semantics
- no derived depth

### 7.2 TwoPointFive

- fixed camera
- depth cues derived from `ZSource`
- preserves current pan/zoom ownership
- selection/hit testing remain graph-continuous

### 7.3 Isometric

- fixed-angle projection
- deterministic layer spacing from `ZSource`
- no free-camera navigation
- preserve current graph interaction continuity

### 7.4 Standard

`Standard` must remain architecture-only until a later 3D program defines:

- true camera contract,
- backend contract,
- 3D hit testing,
- and possible `rapier3d` alignment.

---

## 8. Interaction and Hit Testing Contract

`graph-canvas` should own interaction logic at the canvas surface.

This includes:

- hover resolution,
- click grammar,
- drag and lasso behavior,
- camera pan/zoom gestures,
- selection continuity across projection modes,
- hit-proxy generation and query APIs.

Parry-backed geometry helpers should support:

- projected node hit areas,
- edge hover proxies,
- scene-object picking,
- route/path editing,
- collider editing surfaces.

The interaction engine should emit typed `CanvasAction` values rather than
mutating app state directly.

---

## 9. Diagnostics and Degradation

`graph-canvas` should expose explicit diagnostics for:

- backend capability resolution,
- projection degradation,
- missing scene assets,
- render-packet size and cull counts,
- hit-test failures or blocked interaction paths.

Degradation must preserve:

- graph truth,
- view isolation,
- selection continuity where possible,
- deterministic fallback to `TwoD` when projection support is unavailable.

---

## 10. Acceptance Shape

The `graph-canvas` subsystem is correctly specified when:

- it can accept view-owned graph/scene inputs without owning graph truth,
- `TwoPointFive` and `Isometric` are expressible as projection variants over
  canonical 2D layout truth,
- interaction and hit testing are defined independently of any one widget stack,
- `ProjectedScene` is the backend seam,
- Vello can be a backend without redefining graph semantics,
- Rapier and Wasmtime can plug into scene/runtime inputs without taking over
  the canvas contract.
