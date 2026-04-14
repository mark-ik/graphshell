<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# GraphCanvas Phase 0–2: Scaffold, Types, Derivation, and Interaction (2026-04-13)

**Status**: Active execution
**Scope**: Phases 0–2 of `2026-04-11_graph_canvas_crate_plan.md` — scaffold
the `graph-canvas` workspace member, land portable core types, implement the
scene derivation pipeline with projection support, and add interaction/hit testing.

**Related**:

- `2026-04-11_graph_canvas_crate_plan.md` — parent phase plan
- `../../technical_architecture/graph_canvas_spec.md` — crate API design
- `../aspect_render/2026-04-12_rendering_pipeline_status_quo_plan.md` — current pipeline truth
- `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md` — Vello/scene direction

---

## Plan

### Context

This is Phase 0 of the graph-canvas extraction. The broader motivation has
shifted: the user intends to migrate Graphshell from egui to iced. This makes
graph-canvas extraction the framework-escape path — every line of canvas logic
extracted into a portable crate is a line that doesn't need rewriting for iced.

The parent plan (`2026-04-11_graph_canvas_crate_plan.md`) defines Phases 0–7.
Phase 0 is deliberately narrow: crate scaffold, portable types, serde, tests.
No egui dependency. No host wiring. No rendering.

### Phase 0 tasks

1. **Crate scaffold**: add `crates/graph-canvas` as a workspace member with
   MPL-2.0, edition 2024, `serde` + `serde_json` dependencies. Match sibling
   crate conventions from `graphshell-core` and `graph-tree`.

2. **Core geometry types**: define portable versions of the types the crate will
   need. These are canvas-facing derived carriers, not graph-domain truth:
   - `CanvasViewport` — rect + scale factor
   - `CanvasCamera` — pan + zoom
   - `ProjectionMode` — `TwoD`, `TwoPointFive { z_source }`, `Isometric { z_source }`, `Standard`
   - `ZSource` — portable mirror of the current `app::graph_views::ZSource`
   - `ThreeDMode` — portable mirror of the current `app::graph_views::ThreeDMode`
   - `SceneMode` — `Browse`, `Arrange`, `Simulate`
   - `ViewDimension` — `TwoD`, `ThreeD { mode, z_source }`

3. **Scene input and packet types**:
   - `CanvasNode<N>` — position, radius, payload ref
   - `CanvasEdge<N>` — source, target, weight/style
   - `CanvasSceneInput<N>` — view id, nodes, edges, scene objects, overlays, scene mode, projection
   - `ProjectedScene<N>` — background, world, overlay draw items, hit proxies
   - `SceneDrawItem` — enum of drawable primitives (circle, line, rect, label, image ref)

4. **Interaction types**:
   - `InteractionState<N>` — hovered node/edge, selection set, lasso state
   - `CanvasAction<N>` — hover, select, drag, lasso, pan, zoom
   - `LassoState` — origin + current corner

5. **Backend capability types**:
   - `CanvasBackendCapabilities` — flags for what the backend supports
   - `CanvasBackend` trait — prepare/render/capabilities (trait only, no impl yet)

6. **Serde and construction tests**: round-trip serialization for all types,
   construction from representative inputs, projection mode default behavior.

7. **WASM check**: verify `cargo check -p graph-canvas --target wasm32-unknown-unknown`
   passes clean, matching the `graphshell-core` WASM-clean standard.

### What stays out of Phase 0

- No egui dependency
- No actual scene derivation logic (Phase 1)
- No interaction logic (Phase 2)
- No host bridge (Phase 3)
- No Vello backend impl (Phase 4)
- No extraction of existing `render/canvas_*.rs` code — that starts in Phase 1

### Extraction source map (for Phase 1+ reference)

| Current file | Lines | Phase 0 portable type mirror |
|---|---|---|
| `render/mod.rs` | 3413 | Scene derivation, camera wiring |
| `render/canvas_camera.rs` | 853 | `CanvasCamera`, pan/zoom transforms |
| `render/canvas_input.rs` | 431 | `CanvasAction`, interaction grammar |
| `render/canvas_visuals.rs` | 653 | Presentation, culling, node visuals |
| `render/canvas_overlays.rs` | 1565 | Overlay draw items |
| `render/spatial_index.rs` | 194 | Hit-test spatial index |
| `graph/scene_runtime.rs` | 1103 | Scene runtime bridge |
| **Total** | **~8200** | |

### Iced migration context

The iced migration direction means:

- Phase 0 types must be fully egui-free. Use `euclid` or plain `f32` tuples
  for geometry, not `egui::Pos2`/`egui::Vec2`/`egui::Rect`.
- The `CanvasBackend` trait should be designed so that an iced `canvas::Program`
  can implement it as naturally as an egui paint callback.
- Phase 3 (egui host bridge) becomes a transitional shim. An iced host bridge
  will follow as a parallel or replacement adapter.
- `graphshell-core` already uses `euclid` for geometry — align with that choice.

---

## Findings

### Sibling crate conventions

- `graphshell-core`: edition 2024, MPL-2.0, uses `serde`, `rkyv`, `euclid`,
  `petgraph`, `uuid`. WASM-clean with target-gated `uuid/v4`.
- `graph-tree`: edition 2024, MPL-2.0, uses `serde`, `serde_json`, `taffy`,
  optional `petgraph`.
- Both are `publish = false`.

### Current type locations

- `SceneMode`, `ZSource`, `ThreeDMode`, `ViewDimension` live in
  `app/graph_views.rs` and are tightly coupled to `GraphBrowserApp`.
- `NodeSpatialIndex` in `render/spatial_index.rs` uses `egui::Pos2` and `rstar`.
  The portable version should use `euclid` points and keep `rstar` as a
  dependency (it's framework-agnostic).
- Camera state is spread across `render/canvas_camera.rs` (transforms) and
  `app/graph_views.rs` (`Camera` struct on `GraphViewState`).
- `NodeKey` is `petgraph::graph::NodeIndex` — already portable.

### Dependency choices for Phase 0

- `euclid` — geometry primitives, matches `graphshell-core`
- `serde` + `serde_json` — serialization
- `rstar` — spatial index (can defer to Phase 2 if preferred)
- No `petgraph` needed yet — `CanvasNode<N>` / `CanvasEdge<N>` are generic over
  the node identifier type

---

## Progress

| Task | Status |
|---|---|
| Plan document created | done |
| Crate scaffold (`crates/graph-canvas`, workspace member) | done |
| Core geometry types (`camera.rs`, `projection.rs`) | done |
| Scene input/packet types (`scene.rs`, `packet.rs`) | done |
| Interaction types (`interaction.rs`) | done |
| LOD policy types (`lod.rs`) | done |
| Backend capability types + trait (`backend.rs`) | done |
| Serde + construction tests (28 tests, all pass) | done |
| WASM check (`wasm32-unknown-unknown` clean) | done |
| DOC_README update | done |

### Notes

- euclid 0.22 requires `features = ["serde"]` for serde support — not enabled
  by default. This is Servo's euclid fork.
- Geometry interop: euclid (graph-canvas/graphshell-core) vs kurbo (vello) vs
  nalgebra (parry2d/rapier2d). Thin conversion traits will be needed at backend
  and hit-test boundaries in Phase 2/Phase 4. Not a blocker for Phase 0.
- `SceneDrawItem` covers the current `egui_graphs` rendering vocabulary:
  `Circle`, `Line`, `RoundedRect`, `Label`, `ImageRef`. May need extension for
  Vello scene primitives in Phase 4, but the packet contract is the seam — the
  backend consumes it, it doesn't define it.

---

## Phase 1: Packet Derivation and Projection

### Phase 1 plan

**Goal**: the crate can derive a deterministic `ProjectedScene` from a
`CanvasSceneInput` for TwoD, TwoPointFive, and Isometric projections.

1. **Projection math** (`projection.rs`): add `project_position()` function
   that transforms a world-space point + z value through a `ProjectionMode`,
   producing screen-space x/y, depth, and depth_scale.

2. **Scene derivation pipeline** (`derive.rs`): pure function `derive_scene()`
   that takes `CanvasSceneInput`, camera, viewport, z-value closure, and
   optional per-node visual overrides, and produces a `ProjectedScene`.

3. **LOD integration**: the pipeline calls `LodPolicy::level_for_node()` and
   skips culled nodes, omits labels below Full LOD.

4. **Viewport culling**: world-space bounding-rect test before projection,
   edges culled when both endpoints are offscreen.

5. **Host z-derivation contract**: the host is responsible for computing z
   values from `ZSource` (requires graph metadata the crate doesn't own). The
   pipeline accepts a `Fn(&N) -> f32` for this.

6. **Visual override contract**: the host resolves theme/presentation colors
   and passes `NodeVisualOverride` per node. Keeps theme logic in the host.

### Phase 1 progress

| Task | Status |
|---|---|
| Projection math (`project_position`, TwoD/TwoPointFive/Isometric) | done |
| `derive_scene()` pipeline | done |
| LOD integration in derivation | done |
| Viewport culling | done |
| Node visual override support | done |
| Edge derivation with endpoint lookup | done |
| Determinism tests | done |
| Projection tests (identity, depth, bounds, shift) | done |
| Derivation tests (11 tests covering all paths) | done |
| WASM check clean | done |
| Total tests: 49 pass, 0 fail | done |

### Phase 1 findings

- **No z-derivation exists in the host codebase yet.** `ZSource` is defined,
  `z_positions: HashMap<NodeKey, f32>` is mentioned in doc comments, but no
  code computes z values from recency/BFS depth/UDC class. This means the
  `Fn(&N) -> f32` contract works cleanly — when the host builds this, it
  plugs directly into the pipeline.

- **Projection tuning constants** are conservative starting points:
  - `PERSPECTIVE_K = 0.003` (2.5D convergence rate)
  - `MIN_DEPTH_SCALE = 0.4` (nodes never shrink below 40%)
  - `TWOPOINTFIVE_Y_SHIFT = 0.15` (vertical depth offset)
  - `ISOMETRIC_X_SHIFT = 0.5`, `ISOMETRIC_Y_SHIFT = 0.35`
  These will need visual tuning once hooked into a live render path.

- **The current host rendering pipeline** (`render/mod.rs`) builds
  `EguiGraphState` and hands it to `egui_graphs::GraphView`. This is ~900
  lines of tightly coupled egui code. Phase 3 (host bridge) will wire
  `derive_scene()` as a replacement for this path, but that requires egui
  paint-callback integration or the iced migration.

- **Edge culling** uses a conservative policy: edges are only culled when both
  endpoints are outside the viewport. This matches the host's existing
  ghost-endpoint policy in `canvas_visuals.rs`.

- **Projection constants made configurable.** Per user feedback, hardcoded
  constants were extracted into `ProjectionConfig` with `TwoPointFiveConfig`
  and `IsometricConfig` sub-structs. All serializable. `DeriveConfig` carries
  the `ProjectionConfig`. No `dyn` trait object needed — the pipeline is a
  pure function call with closures.

---

## Phase 2: Interaction and Hit Testing

### Phase 2 plan

**Goal**: the crate can resolve pointer events into canvas actions (hover,
select, drag, lasso, pan, zoom) without touching framework state.

1. **Hit testing** (`hit_test.rs`): point-in-circle for nodes, bounding-box
   for edges, with reverse-order priority (topmost drawn = first hit).
   Lasso-rect node collection by center containment.

2. **Portable input events** (`input.rs`): `CanvasInputEvent` enum with
   `PointerMoved`, `PointerPressed`, `PointerReleased`, `PointerDoubleClick`,
   `Scroll`, `PointerLeft`. Host translates framework events into these.

3. **Interaction engine** (`engine.rs`): stateful `InteractionEngine<N>` with
   a `DragState` state machine (None → Pending → DraggingNode/Lasso/Panning).
   `process_event()` takes an event + hit proxies + camera + viewport and
   returns `Vec<CanvasAction<N>>`. Handles:
   - Hover resolution (pointer move → hit test → hover action)
   - Click vs drag distinction via threshold
   - Node drag with delta computation
   - Lasso lifecycle (background drag → rect update → commit on release)
   - Camera pan via secondary/middle button drag
   - Scroll → zoom action
   - Ctrl+click toggle selection
   - Click on background → clear selection

### Phase 2 progress

| Task | Status |
|---|---|
| Point hit testing (`hit_test_point`) | done |
| Lasso rect collection (`nodes_in_screen_rect`) | done |
| Portable input events (`CanvasInputEvent`) | done |
| Interaction engine (`InteractionEngine`, `DragState`) | done |
| Hover resolution | done |
| Click vs drag distinction | done |
| Node drag with delta | done |
| Lasso lifecycle | done |
| Camera pan (secondary/middle drag) | done |
| Scroll → zoom | done |
| Ctrl+click toggle | done |
| Hit test tests (8 tests) | done |
| Engine tests (10 tests) | done |
| Modules registered in lib.rs | done |
| WASM check clean | done |
| Total tests: 67 pass, 0 fail, 0 warnings | done |

### Phase 2 findings

- **Hit proxy geometry is simple.** Nodes use circle (center + radius), edges
  use axis-aligned bounding box (midpoint + half_width). This matches the
  current rendering vocabulary. Phase 4+ may need more precise edge hit testing
  (e.g. capsule/line-segment distance) when edges become curved Béziers.

- **No `rstar` spatial index yet.** Hit testing iterates the full hit proxy
  slice in reverse order. This is O(n) per event but sufficient for hundreds
  of nodes. Phase 4+ can add `rstar`-based acceleration if profiling shows
  need.

- **Drag threshold prevents accidental drags.** A 4px threshold distinguishes
  click from drag. The `DragState::Pending` state buffers the press until the
  pointer moves past the threshold, then resolves into the appropriate drag
  mode based on what was hit.

- **Camera gesture pattern.** Secondary button (right-click) drag and middle
  button drag both initiate panning. This mirrors the existing egui canvas
  behavior in `canvas_input.rs`.

---

## Phase 3: Host Bridge

### Phase 3 plan

**Goal**: the host application can construct graph-canvas inputs from domain
graph state and convert graph-canvas outputs back into the host's action/intent
model.

Given the iced migration direction, Phase 3 is deliberately thin — it creates
the adapter layer, not a full egui render pipeline replacement. The adapter
code lives in `render/canvas_bridge.rs` and will be replaced by an iced
equivalent when the framework migration happens.

1. **graph-canvas dependency** — add `graph-canvas` to the main app's
   `Cargo.toml`.

2. **Scene input builder** (`build_scene_input`) — converts the domain graph
   (`Graph`) and view state into a `CanvasSceneInput<NodeKey>`. Iterates
   nodes for positions/labels and raw petgraph edges for connectivity.

3. **Action translator** (`canvas_action_to_graph_actions`) — maps
   `CanvasAction<NodeKey>` back into the host's existing `GraphAction` enum.
   Hover/pan/lasso-lifecycle actions are handled by the interaction engine's
   state and don't map to GraphActions.

4. **Drag node helper** (`apply_drag_node_delta`) — separated from the
   stateless translator because it needs mutable graph access. Applies a
   world-space delta to the node's projected position.

5. **Camera sync** (`camera_from_view_frame`, `camera_to_view_frame`,
   `viewport_from_egui_rect`) — bidirectional conversion between the app's
   `GraphViewFrame`/egui `Rect` and graph-canvas's `CanvasCamera`/
   `CanvasViewport`.

6. **Camera mutation helpers** (`apply_pan`, `apply_zoom`) — apply
   `PanCamera` and `ZoomCamera` actions to a `CanvasCamera`, including
   focus-point-preserving zoom.

7. **Event translator** (`collect_canvas_events`) — reads egui's per-frame
   `InputState` and produces `Vec<CanvasInputEvent>`. Handles primary,
   secondary, and middle button press/release, pointer movement, double-click,
   and scroll/zoom.

### Phase 3 progress

| Task | Status |
|---|---|
| Add `graph-canvas` dependency to main Cargo.toml | done |
| Scene input builder (`build_scene_input`) | done |
| Action translator (`canvas_action_to_graph_actions`) | done |
| Drag node helper (`apply_drag_node_delta`) | done |
| Camera sync (GraphViewFrame ↔ CanvasCamera, egui Rect → CanvasViewport) | done |
| Camera mutation helpers (apply_pan, apply_zoom) | done |
| Event translator (`collect_canvas_events`) | done |
| Module registered in render/mod.rs | done |
| Compile clean (no errors or warnings from bridge) | done |
| graph-canvas tests still pass (67/67) | done |
| Bridge unit tests (7 tests: roundtrip, conversion, zoom) | done |

### Phase 3 findings

- **Pre-existing compile errors** in `middlenet-engine` (taffy API breakage)
  and `graph-tree` (taffy Children API change). These are unrelated to
  graph-canvas and pre-date this work.

- **No node radius in domain graph.** The domain `Node` type doesn't carry a
  display radius — that's determined by `GraphNodeShape` in the egui_graphs
  adapter layer. The bridge uses a constant `16.0` default. When the iced
  migration happens, the host will need to resolve node radius from its own
  presentation policy and pass it per-node.

- **Edge deduplication.** The domain graph's `edges()` method fans out one
  `EdgeView` per edge family/kind. The bridge uses `graph.inner
  .edge_references()` directly to get one `CanvasEdge` per petgraph edge,
  avoiding visual duplication. Edge kind/styling will be provided via
  `NodeVisualOverride` or an edge-override mechanism in a future phase.

- **GraphViewId → ViewId.** `GraphViewId` wraps a UUID; `ViewId` is an
  opaque `u64`. The bridge extracts the lower 64 bits of the UUID. Collision
  is astronomically unlikely for typical view counts.

- **Middle button.** egui 0.34 has `primary_pressed()`/`secondary_pressed()`
  convenience methods but not `middle_pressed()`. The bridge uses
  `button_pressed(egui::PointerButton::Middle)` instead.

---

## Phase 4: Vello Backend

### Phase 4 plan

**Goal**: the crate can render a `ProjectedScene` through Vello's scene
builder, producing GPU-ready drawing commands for circles, lines, rounded
rects, and strokes.

1. **Optional dependency** — add `vello` 0.6 and `peniko` 0.5 as optional
   dependencies behind a `vello` feature flag. Both the base crate and the
   vello-featured crate are WASM-clean. The feature flag controls dependency
   weight (skrifa, vello_encoding, png, etc.), not portability.

2. **Geometry/color conversion** — `euclid::Point2D<f32>` → `kurbo::Point`
   (f32→f64), graph-canvas `Color` → `peniko::AlphaColor<Srgb>`, graph-canvas
   `Stroke` → `kurbo::Stroke`.

3. **Scene rendering** (`render_projected_scene`) — iterates background,
   world, and overlay layers of a `ProjectedScene`, converting each
   `SceneDrawItem` into vello `Scene::fill()` / `Scene::stroke()` calls.

4. **Draw item coverage**:
   - `Circle` → `kurbo::Circle`, fill + optional stroke
   - `Line` → `kurbo::Line`, stroke
   - `RoundedRect` → `kurbo::RoundedRect`, fill + optional stroke
   - `Label` → placeholder dot (text requires host-side font pipeline)
   - `ImageRef` → skipped (requires host-side ImageHandle resolution)

5. **Capability reporting** (`vello_capabilities()`) — reports support for
   2.5D, isometric, and anti-aliased strokes. Images and labels are reported
   as unsupported (host responsibility).

### Phase 4 progress

| Task | Status |
|---|---|
| Vello + peniko optional dependencies (feature flag `vello`) | done |
| Geometry conversion (euclid → kurbo, f32 → f64) | done |
| Color conversion (Color → AlphaColor<Srgb>) | done |
| Stroke conversion (Stroke → kurbo::Stroke) | done |
| `render_projected_scene()` function | done |
| Circle rendering | done |
| Line rendering | done |
| RoundedRect rendering | done |
| Label placeholder | done |
| ImageRef placeholder | done |
| `vello_capabilities()` report | done |
| Module registered behind `#[cfg(feature = "vello")]` | done |
| Base tests pass (67/67 without feature) | done |
| Vello tests pass (72/72 with feature) | done |
| WASM check clean (base, no vello) | done |
| WASM check clean (with vello feature) | done |

### Phase 4 findings

- **Stroke lives in kurbo, not peniko.** peniko 0.5 re-exports kurbo but
  `Stroke` is `peniko::kurbo::Stroke`, not `peniko::Stroke` (which is an
  enum variant of `Style`).

- **f32 → f64 precision.** kurbo uses `f64` throughout; graph-canvas uses
  `f32`. The conversion is lossless (f32 → f64 is exact). The reverse
  direction would need care, but the backend only reads from graph-canvas
  types — it never writes back.

- **Text rendering is host-specific.** Vello's text API requires
  skrifa/parley/fontique for font loading and glyph shaping. This is too
  heavy for the portable crate. The backend emits a placeholder dot at label
  positions. The host should render text using its own pipeline or overlay
  labels via the UI framework (egui text, iced text, etc.).

- **Image rendering is host-specific.** Vello's `draw_image()` needs an
  `ImageData` (pixel buffer), but graph-canvas uses opaque `ImageHandle(u64)`
  resolved by the host. The host should post-process image items or implement
  a custom image resolver.

- **The backend is a function, not a trait impl.** The `CanvasBackend` trait
  defined in Phase 0 uses an associated `FrameHandle` type, which would
  require the caller to know the concrete backend type. Instead, Phase 4
  provides `render_projected_scene()` as a standalone function that takes
  `&mut vello::Scene`. This is more ergonomic for the common case where the
  host already owns the vello Scene/Renderer lifecycle. The trait remains
  available for future polymorphic backend selection.

---

## Phase 5 — Scene Region Types & Physics Algorithms

### Phase 5 scope

The parent plan (Phase 5) originally called for "avatar systems and scene
packages." Neither exists in the codebase. What *does* exist is scene region
editing (Arrange mode) and physics-style node manipulation (separation,
containment, region effects). Phase 5 extracts those into graph-canvas as
portable, framework-agnostic modules.

### Phase 5 tasks

| Task | Status |
|------|--------|
| `scene_region.rs` — `SceneRegionId`, `SceneRegionShape`, `SceneRegionEffect`, `SceneRegion` | done |
| Shape methods: `center()`, `contains()`, `translate()` | done |
| `SceneRegionResizeHandle` + `resize_shape_to_pointer()` with min-size enforcement | done |
| `scene_physics.rs` — `ScenePhysicsConfig` with tunable constants | done |
| `NodeSnapshot<N>` generic carrier type | done |
| `compute_node_separation()` — O(n²) circle-circle collision, pinned-aware | done |
| `compute_viewport_containment()` — clamp nodes to viewport bounds | done |
| `compute_region_effects()` — attractor/repulsor/dampener/wall effects | done |
| `wall_pushout_delta()` — circle wall (radial) and rect wall (nearest-side) | done |
| Delta clamping (`max_region_delta`) | done |
| All types Serialize/Deserialize | done |
| Modules registered in `lib.rs` | done |
| Base tests pass (93/93 without vello) | done |
| Vello tests pass (98/98 with feature) | done |
| WASM check clean (base, no vello) | done |

### Phase 5 findings

- **parry2d removed.** The previous codebase used `parry2d::shape::Ball` for
  node separation. The algorithm is just circle-circle overlap detection
  (distance < r1 + r2), which is trivial to express with euclid. Removing the
  parry dependency keeps graph-canvas lean and WASM-safe without pulling in a
  full collision library.

- **Physics functions are stateless.** Each function takes a `&[NodeSnapshot<N>]`
  slice and returns a `HashMap<N, Vector2D<f32>>` of deltas. The host applies
  deltas to its own node storage. This avoids ownership entanglement between
  graph-canvas and the graph data model.

- **Region effects are composable.** `compute_region_effects()` iterates all
  visible regions and accumulates per-node deltas. The total is clamped to
  `max_region_delta` per node to prevent instability when multiple strong
  attractors/repulsors overlap.

- **Pinned nodes are immovable.** All three functions respect the `pinned` flag
  on `NodeSnapshot`. Pinned nodes still participate as obstacles (pushing
  others away in separation) but never receive deltas themselves.

- **Parry2d deferred, not dropped.** Phase 5 extracts physics as pure math
  (circle-circle, rect containment) without a parry dependency. This is the
  correct baseline — parry comes back in Phase 6 as the geometry/query/editor
  layer for non-circle colliders, projected hit proxies, and spatial queries.
  Phase 5's stateless delta-return contract (`HashMap<N, Vector2D>`) is
  designed to coexist with richer parry/rapier-backed computations.

---

## Phase 6 — Rapier Simulate & Parry Geometry

### Phase 6 context

The parent plan's Phase 6 is "Rapier `Simulate`." The architecture plan
(`2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`)
defines Parry2D as the geometry/query/editor layer and Rapier2D as the live
rigid-body world for `SceneMode::Simulate`.

No Rapier code exists in the codebase yet. The current simulate behavior is a
hand-rolled `SimulateMotionProfile` (Float/Packed/Magnetic presets) with
release-impulse coasting in `graph/scene_runtime.rs`. Phase 6 extracts that
baseline into graph-canvas and builds the real Rapier integration on top.

### Phase 6 scope

1. **Extract existing simulate baseline** — `SimulateMotionProfile` and
   release-impulse decay logic become portable in graph-canvas, independent of
   any physics engine.

2. **Add parry2d + rapier2d as optional dependencies** — behind `physics` and
   `simulate` feature flags respectively. Parry is useful without Rapier
   (Browse/Arrange geometry queries); Rapier implies Parry.

3. **Define scene composition types** — the portable type vocabulary from the
   architecture plan: `ColliderSpec`, `PhysicsMaterial`, `SceneBodyKind`,
   `NodeAvatarBinding`, `SceneBodyMapping`. These live in graph-canvas
   unconditionally (they're just data); only the implementations that touch
   rapier/parry are feature-gated.

4. **Implement `RapierSceneWorld`** — the runtime world carrier behind the
   `simulate` feature flag. Owns `RigidBodySet`, `ColliderSet`,
   `IntegrationParameters`, `PhysicsPipeline`. Provides:
   - `build_from_scene()` — construct bodies/colliders from node snapshots +
     avatar bindings + scene regions
   - `step()` — advance one physics frame
   - `read_positions()` → `HashMap<N, Vector2D<f32>>` — same delta contract
     as Phase 5's stateless functions
   - `read_events()` → `Vec<SceneEvent>` — contact/trigger events

5. **Implement parry2d geometry queries** — behind the `physics` feature flag.
   Point queries, ray queries, and shape-cast for projected hit proxies. These
   work in Browse and Arrange without a Rapier world.

6. **Preserve graph truth isolation** — Rapier body positions are runtime-only.
   The host decides whether to apply deltas. Each view that opts into Simulate
   gets its own world instance.

### Phase 6 feature flag design

```
[features]
default = []
vello = ["dep:vello", "dep:peniko"]
physics = ["dep:parry2d"]
simulate = ["physics", "dep:rapier2d"]
```

- `physics`: Parry2D geometry queries for Browse/Arrange. No rigid-body
  simulation.
- `simulate`: Rapier2D rigid-body world for Simulate mode. Implies `physics`.
- The base crate (no features) retains the Phase 5 pure-math physics baseline.

### Phase 6 tasks

| Task | Status |
|------|--------|
| Extract `SimulateMotionProfile` + release impulse decay into `scene_physics.rs` | done |
| Add `parry2d` + `rapier2d` optional deps with feature flags | done |
| `scene_composition.rs` — `ColliderSpec`, `PhysicsMaterial`, `SceneBodyKind`, `NodeAvatarBinding` | done |
| `simulate.rs` — `RapierSceneWorld` (build, step, read positions, read events) | done |
| `SceneEvent` type — `ContactBegin`/`ContactEnd`, `TriggerEnter`/`TriggerExit` | done |
| `geometry.rs` — parry2d point/ray/shape queries behind `physics` feature | done |
| Tests: simulate motion profile (extracted from scene_runtime.rs tests) | done |
| Tests: rapier world construction and step (behind `simulate` feature) | done |
| Tests: parry geometry queries (behind `physics` feature) | done |
| All base tests still pass without features | done |
| Vello tests still pass | done |
| WASM check clean (base) | done |
| WASM check clean (`physics` feature) | done |
| WASM check clean (`simulate` feature) | done |

### Phase 6 type inventory

From the architecture plan's canonical scene composition types:

```rust
// scene_composition.rs — unconditional (pure data)

enum ColliderSpec {
    Circle { radius: f32 },
    Rect { half_extents: Vector2D<f32> },
    Capsule { half_height: f32, radius: f32 },
    ConvexHull { points: Vec<Point2D<f32>> },
    Compound(Vec<ColliderSpec>),
}

struct PhysicsMaterial {
    density: f32,
    friction: f32,
    restitution: f32,
    linear_damping: f32,
    angular_damping: f32,
    gravity_scale: f32,
}

enum SceneBodyKind {
    Dynamic,
    Static,
    KinematicPositionBased,
    KinematicVelocityBased,
}

struct NodeAvatarBinding<N> {
    node_id: N,
    collider: ColliderSpec,
    material: PhysicsMaterial,
    body_kind: SceneBodyKind,
}
```

```rust
// simulate.rs — behind `simulate` feature

struct RapierSceneWorld<N> {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    body_map: HashMap<N, RigidBodyHandle>,
    event_collector: Vec<SceneEvent<N>>,
}
```

```rust
// scene_physics.rs — extended

enum SceneEvent<N> {
    ContactBegin { a: N, b: N },
    ContactEnd { a: N, b: N },
    TriggerEnter { node: N, region: SceneRegionId },
    TriggerExit { node: N, region: SceneRegionId },
}

struct SimulateMotionProfile {
    release_impulse_scale: f32,
    release_decay: f32,
    min_impulse: f32,
}

enum SimulateBehaviorPreset {
    Float,
    Packed,
    Magnetic,
}
```

### Phase 6 findings

**parry2d 0.26 and rapier2d 0.32 use glam, not nalgebra.** Both crates
migrated from nalgebra to glam internally. `parry2d::math::Vector` is a
re-export of `glam::Vec2`, and `Pose` replaces the old `Isometry2<f32>`.
However, rapier2d still re-exports nalgebra types in its prelude and provides
the `vector![]` macro which produces nalgebra `Vector2<f32>` — these are
**incompatible** with the glam-based internal types. Use `Vec2::new()` directly
for all rapier/parry vectors rather than the `vector![]` macro.

**Key API patterns for parry2d 0.26:**

- `Pose::from_translation(vec2)` — replaces `Isometry::translation(x, y)`
- `shape.project_point(&pose, vec2, solid)` — point arg is `Vec2`, not `Point2`
- `(proj.point - query_point).length()` — `norm()` does not exist on `Vec2`

**rapier2d 0.32 `step()` signature** takes 12 arguments: gravity by value
(`Vec2`), `&IntegrationParameters`, `&mut IslandManager`, `&mut BroadPhase`,
`&mut NarrowPhase`, `&mut RigidBodySet`, `&mut ColliderSet`,
`&mut ImpulseJointSet`, `&mut MultibodyJointSet`, `&mut CCDSolver`,
`Option<&mut QueryPipeline>`, `&dyn PhysicsHooks`, `&dyn EventHandler`. The
hooks arg accepts `&()` (unit impl), and the event handler is a separate
argument from the collector.

**Event collection uses `std::sync::mpsc`.** `ChannelEventCollector` is
constructed from `mpsc::Sender<CollisionEvent>` and
`mpsc::Sender<ContactForceEvent>`. Events are drained from the receiver after
each step.

**Feature flag architecture:**

- Base crate (no features): scene_composition types + scene_physics types are
  unconditional pure data with no engine dependency. Phase 5 circle-circle
  math baseline also unconditional.
- `physics` feature: enables `geometry.rs` (parry2d queries). Adds
  `collider_to_shape()`, `point_query()`, `nearest_body()`,
  `shapes_overlap()`. Useful in Browse/Arrange without a simulation.
- `simulate` feature: enables `simulate.rs` (RapierSceneWorld). Implies
  `physics`. Provides build/step/read delta contract.

**WASM compatibility confirmed.** All feature combos pass `cargo check
--target wasm32-unknown-unknown`. Neither parry2d 0.26 nor rapier2d 0.32
pull in non-WASM-safe dependencies under their default feature sets.

**Test counts:** base 110, physics 122, simulate 131, vello 115. All green.

---

## Phase 7: Scripting hooks

### Phase 7 context

Phase 7 of the parent plan: "Wasmtime scripting hooks." Done gate: **Wasmtime
scene objects can participate in the same canvas packet and interaction model.**

graph-canvas must remain WASM-portable (`wasm32-unknown-unknown`), so it cannot
embed Wasmtime. The pattern matches `CanvasBackend<N>`: graph-canvas defines the
types and integration points; the host implements with Wasmtime/Extism.

Currently `CanvasSceneObject` is a placeholder (`{position, kind: String}`)
that `derive_scene` ignores entirely. `HitProxy`, `HitTestResult`,
`CanvasAction`, and `InteractionState` only know about nodes and edges. Scene
objects cannot be rendered, hit-tested, or interacted with.

### Phase 7 scope

Make scene objects first-class participants in the canvas packet, hit testing,
and interaction model — on par with nodes and edges. Add a scripting types
module defining the capability model and script I/O contract. No new
dependencies, no feature flags (all pure data types + pipeline logic).

**Not in scope:**

- Wasmtime/Extism runtime (lives in host, not in graph-canvas)
- WIT definitions or guest ABI (host-side concern)
- Layout scripting hooks (separate concern, covered by WASM layout runtime plan)
- Scene object dragging (scripts control their own position)

### Phase 7 tasks

| Task | Status |
|------|--------|
| `scripting.rs` — `SceneObjectId`, `SceneObjectHitShape`, `ScriptCapability`, `ScriptDiagnostic`, `SceneObjectOutput` | done |
| Enrich `CanvasSceneObject` in `scene.rs` — id, draw_items, hit_shape, overlay_items | done |
| `HitProxy::SceneObject` variant in `packet.rs` | done |
| `HitTestResult::SceneObject` variant + hit testing in `hit_test.rs` | done |
| `CanvasAction::HoverSceneObject` / `ClickSceneObject` + `InteractionState.hovered_scene_object` in `interaction.rs` | done |
| `derive_scene_objects` stage in `derive.rs` — cull, project, emit draw items + hit proxies | done |
| Scene object handling in `engine.rs` — hover, click, clear, drag-fallthrough to pan | done |
| Register `pub mod scripting` in `lib.rs` | done |
| Tests: scripting types (construction, serde roundtrip, defaults) | done |
| Tests: scene object hit testing (hit, topmost-wins, lasso ignores) | done |
| Tests: scene object derivation (draw items, culling, mixed scene) | done |
| Tests: engine scene object interactions (hover, click, leave-clears) | done |
| All feature combos pass tests | done |
| WASM check clean (all combos) | done |

### Phase 7 type inventory

```rust
// scripting.rs — unconditional (pure data, no engine dependency)

/// Opaque identifier for a scripted scene object.
struct SceneObjectId(u64);

/// Interaction surface shape for a scene object.
enum SceneObjectHitShape {
    Circle { radius: f32 },
    Rect { half_extents: Vector2D<f32> },
}

/// Capability flags declaring what a script is allowed to produce.
struct ScriptCapability {
    emit_draw_items: bool,
    emit_hit_proxy: bool,
    emit_overlays: bool,
    read_node_positions: bool,
    read_scene_mode: bool,
}

/// Diagnostic emitted when a capability is blocked or a script misbehaves.
struct ScriptDiagnostic {
    severity: DiagnosticSeverity,
    message: String,
    source_id: SceneObjectId,
}

enum DiagnosticSeverity { Info, Warn, Error }

/// Per-frame output from a scripted scene object.
/// The host runs the script, packs this, passes it in CanvasSceneInput.
struct SceneObjectOutput {
    draw_items: Vec<SceneDrawItem>,
    hit_shape: Option<SceneObjectHitShape>,
    overlay_items: Vec<SceneDrawItem>,
    diagnostics: Vec<ScriptDiagnostic>,
}
```

```rust
// scene.rs — enriched CanvasSceneObject (replaces placeholder)

struct CanvasSceneObject {
    id: SceneObjectId,
    position: Point2D<f32>,
    draw_items: Vec<SceneDrawItem>,
    hit_shape: Option<SceneObjectHitShape>,
    overlay_items: Vec<SceneDrawItem>,
}
```

```rust
// packet.rs — extended HitProxy

enum HitProxy<N> {
    Node { id: N, center: Point2D<f32>, radius: f32 },
    Edge { source: N, target: N, midpoint: Point2D<f32>, half_width: f32 },
    SceneObject { id: SceneObjectId, center: Point2D<f32>, radius: f32 },
}
```

```rust
// interaction.rs — extended CanvasAction + InteractionState

enum CanvasAction<N> {
    // ... existing variants ...
    HoverSceneObject(Option<SceneObjectId>),
    ClickSceneObject(SceneObjectId),
}

struct InteractionState<N> {
    // ... existing fields ...
    hovered_scene_object: Option<SceneObjectId>,
}
```

### Phase 7 integration flow

```
Host (Wasmtime/Extism)             graph-canvas
─────────────────────              ────────────
1. Load WASM script
2. Call script.render(context)
3. Get SceneObjectOutput
4. Pack into CanvasSceneObject ──► CanvasSceneInput.scene_objects
                                   │
                                   ▼
                               derive_scene()
                                   │ derive_scene_objects stage:
                                   │ - viewport cull
                                   │ - project position
                                   │ - offset draw items to screen space
                                   │ - emit HitProxy::SceneObject
                                   ▼
                               ProjectedScene
                                   │ world + overlays + hit_proxies
                                   ▼
                               InteractionEngine
                                   │ hit test → HitTestResult::SceneObject
                                   │ hover/click → CanvasAction
                                   ▼
5. Host receives CanvasAction ◄── CanvasAction::ClickSceneObject(id)
6. Route to script.on_event()
```

### Phase 7 findings

**No new dependencies or feature flags.** All Phase 7 types and pipeline logic
are pure data and unconditional — no physics engine or runtime dependency. The
`scripting` module sits alongside `scene_composition` as framework-agnostic
infrastructure that any host can consume.

**Scene objects are now first-class in the canvas pipeline.** They flow through
the same derivation, hit testing, and interaction model as nodes and edges:

- `derive_scene_objects` applies viewport culling, world→screen offset, and
  hit proxy emission
- `hit_test_point` resolves scene object hits with the same reverse-order
  (topmost-wins) priority as nodes/edges
- `InteractionEngine` emits `HoverSceneObject`/`ClickSceneObject` actions
  and tracks `hovered_scene_object` in `InteractionState`
- Scene object drags fall through to camera panning (scripts control position)
- Lasso selection correctly ignores scene objects (they are not graph nodes)

**Draw item offset pattern.** Scene object draw items use object-local
coordinates (relative to the object's position). The derivation pipeline
offsets them to screen space via `offset_draw_item()`, which handles all five
`SceneDrawItem` variants (Circle, Line, RoundedRect, Label, ImageRef).

**Capability model is defined but not enforced in the canvas.** The
`ScriptCapability` flags and `ScriptDiagnostic` types exist for the host to
validate script outputs and emit diagnostics. The canvas itself does not check
capabilities — it trusts the host to pack valid `CanvasSceneObject` data. This
keeps the derivation pipeline simple and deterministic. Capability enforcement
is the host's responsibility.

**The `SceneObjectOutput` type bridges host and canvas.** The host collects
script output as `SceneObjectOutput` (draw items, hit shape, overlays,
diagnostics), then packs the renderable parts into `CanvasSceneObject` for
`CanvasSceneInput`. Diagnostics stay host-side. This separation keeps the
canvas input clean.

**Test counts:** base 133, physics 145, simulate 154, vello 138. All green.
WASM check clean across all feature combos.
