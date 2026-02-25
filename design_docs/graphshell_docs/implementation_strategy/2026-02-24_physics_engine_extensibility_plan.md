<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Physics Engine Extensibility Plan (2026-02-24)

**Status**: Research / Design (updated 2026-02-24 — expanded with crate landscape, WASM layout
mod architecture, fractal layouts, rapier2d functional physics layer, mobile considerations,
2D↔3D hotswitch architecture)
**Relates to**:

- `2026-02-22_registry_layer_plan.md` — `PhysicsProfileRegistry` owns named presets; `CanvasRegistry` owns engine execution; `LayoutRegistry` owns positioning algorithms
- `2026-02-24_layout_behaviors_plan.md` — behavioral micro-features (reheat, clustering, magnetic zones) that build on top of whichever engine is active
- `2026-02-23_graph_interaction_consistency_plan.md` — gravity locus dampening (Phase 3) is a concrete `ExtraForce` candidate
- `2026-02-22_multi_graph_pane_plan.md` — pane-hosted multi-view architecture; `GraphViewState`, `Canonical`/`Divergent` layout modes, and `LocalSimulation` remain graph-pane payload concepts; 3D views are a new `ViewDimension` axis on `GraphViewState`
- `design_docs/PROJECT_DESCRIPTION.md` — 2D↔3D hotswitch with position parity is a named first-class vision feature

---

## Context: What We Have Today

`render/mod.rs` uses egui_graphs 0.29.0 force-directed layout. The type threaded through
`GraphView`, `LocalSimulation`, `PhysicsProfile.apply_to_state()`, and the `set_layout_state`/
`get_layout_state` roundtrip is:

```rust
// egui_graphs upstream — a convenience alias over the composable extras system
pub type FruchtermanReingoldWithCenterGravity =
    FruchtermanReingoldWithExtras<(Extra<CenterGravity, true>, ())>;

pub type FruchtermanReingoldWithCenterGravityState =
    FruchtermanReingoldWithExtrasState<(Extra<CenterGravity, true>, ())>;
```

This is not a locked-in concrete struct. The full upstream surface available today:

| egui_graphs export | What it is |
| --- | --- |
| `FruchtermanReingold` / `FruchtermanReingoldState` | Base FR, no extras |
| `FruchtermanReingoldWithCenterGravity` *(current)* | FR + center-gravity extra |
| `FruchtermanReingoldWithExtras<E>` | FR + arbitrary composable extra tuple |
| `LayoutHierarchical` / `LayoutStateHierarchical` | Sugiyama-style layered layout |
| `LayoutRandom` / `LayoutStateRandom` | Random scatter (init / chaos mode) |
| `Layout<S>` trait | Open extension point — implement to replace engine entirely |
| `LayoutState` trait | Companion state contract (`SerializableAny + Default + Debug`) |
| `ExtraForce` trait | Open extension point — inject a custom force into FR pipeline |
| `AnimatedState` trait | Optional — pause/resume, convergence metrics, step count |

The architecture already owns the seam: `GraphView` is parameterized over `(StateType, LayoutType)`
and `set_layout_state` / `get_layout_state` are the only two call sites that touch the concrete
type. `PhysicsProfile.apply_to_state()` is the only place that knows the state field names.

`SerializableAny` in egui 0.33.3 is:
`'static + Any + Clone + serde::Serialize + for<'a> serde::Deserialize<'a> + Send + Sync`
(with `persistence` feature; without it, just `'static + Any + Clone + Send + Sync`). Every
custom `LayoutState` must satisfy this — serde derives are sufficient.

---

## Three Levels of Extension

These are not competing options. They are a progression.

### Level 1 — Naming (Zero Risk)

Add a local type alias module so the long upstream name disappears from the codebase.
No behavior change. Zero risk.

**Location**: `graph/physics.rs` (new file, plain crate module — not a native mod).

```rust
// graph/physics.rs

pub use egui_graphs::FruchtermanReingoldWithCenterGravityState as GraphPhysicsState;
pub use egui_graphs::FruchtermanReingoldWithCenterGravity       as GraphPhysicsLayout;
pub use egui_graphs::FruchtermanReingoldWithExtrasState         as GraphPhysicsExtrasState;
pub use egui_graphs::FruchtermanReingoldWithExtras              as GraphPhysicsExtrasLayout;
pub use egui_graphs::FruchtermanReingoldState                   as FrBaseState;
pub use egui_graphs::layouts::force_directed::{
    ExtraForce, Extra, ExtrasTuple, CenterGravity, CenterGravityParams,
};
pub use egui_graphs::layouts::{Layout, LayoutState, AnimatedState};
```

Every Graphshell file imports from `graph::physics` instead of `egui_graphs` directly.
`PhysicsProfile.apply_to_state()` continues unchanged; only the import path changes.

---

### Level 2 — Extend via `ExtraForce` (Composable Custom Forces)

Implement custom forces as `ExtraForce` impls, compose them into the type alias tuple.
The FR engine core (repulsion, attraction, damping, convergence) remains upstream; Graphshell
injects additional forces after base forces run each frame.

**Location**: `graph/forces/` submodule under `graph/physics.rs`.

```rust
// graph/forces/gravity_locus.rs
pub struct GravityLocus;

impl ExtraForce for GravityLocus {
    type Params = GravityLocusParams;
    fn apply<...>(params: &Self::Params, g, indices, disp, area, k) {
        for (i, idx) in indices.iter().enumerate() {
            let pos = g.node(*idx).location();
            let delta = params.target - pos;
            disp[i] += delta * params.strength * k;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GravityLocusParams {
    pub target: egui::Pos2,   // updated per-frame from viewport center (lerped)
    pub strength: f32,        // driven by PhysicsProfile.gravity_strength
}
```

The gravity locus dampening from the interaction plan (`gravity_target.lerp(viewport_center, 0.05)`)
becomes a per-frame param update — set `params.target` before `set_layout_state` instead of
applying the lerp inside the render loop.

```rust
// graph/physics.rs — the active compound type used everywhere in Graphshell

pub type GraphPhysicsState = FruchtermanReingoldWithExtrasState<(
    Extra<CenterGravity, true>,     // center-gravity (existing)
    Extra<GravityLocus, false>,     // viewport-relative gravity locus
    Extra<DomainCluster, false>,    // domain clustering (Phase 2.2)
    Extra<DegreeRepulsion, false>,  // degree-dependent repulsion (Phase 2.1)
    (),
)>;

pub type GraphPhysicsLayout = FruchtermanReingoldWithExtras<(
    Extra<CenterGravity, true>,
    Extra<GravityLocus, false>,
    Extra<DomainCluster, false>,
    Extra<DegreeRepulsion, false>,
    (),
)>;
```

`apply_to_state()` maps each extra's `enabled` flag and `params` from the active `PhysicsProfile`:

```rust
pub fn apply_to_state(&self, state: &mut GraphPhysicsState) {
    state.base.c_repulse = self.repulsion_strength;
    state.base.c_attract = self.attraction_strength;
    state.base.damping   = self.damping;
    state.extras.0.params.c  = self.gravity_strength;   // CenterGravity
    state.extras.1.enabled   = self.viewport_gravity;   // GravityLocus
    state.extras.2.enabled   = self.domain_clustering;  // DomainCluster
    state.extras.3.enabled   = self.degree_repulsion;   // DegreeRepulsion
}
```

**Compile-time tuple constraint**: The extras tuple is fixed at compile time. Adding a new
`ExtraForce` requires one new tuple slot and one new line in `apply_to_state()`. This is a
small local change but requires a recompile. It is not a runtime-extensible plugin surface.
For runtime extensibility without recompilation, see Level 3 and the WASM layout mod section.

---

### Level 3 — Replace via `Layout<S>` (Custom Engine)

Implement `Layout<S>` and `LayoutState` from scratch. The egui_graphs rendering machinery
(`GraphView`, `GraphNodeShape`, `GraphEdgeShape`) is entirely independent of the layout engine —
swapping the engine does not touch any render code.

**When Level 3 is warranted**:

- Force-directed is fundamentally wrong for the topology (strict DAG → hierarchical; citation graph → radial; traversal archive → timeline).
- The layout needs data that `ExtraForce` cannot see (external anchors, rapier2d physics bodies, constraint solvers, fractal decomposition state).
- Runtime mod extensibility: a WASM mod must be able to contribute layout computation without recompilation (see WASM section below).

**Call site change** (only `render/mod.rs`):

```rust
// Before
GraphView::<_, _, FruchtermanReingoldWithCenterGravityState,
              LayoutForceDirected<FruchtermanReingoldWithCenterGravity>>::new(...)

// After — custom engine, all render code unchanged
GraphView::<_, _, MyLayoutState, MyLayout>::new(...)
```

`set_layout_state` / `get_layout_state` remain unchanged in shape — only the type parameter
changes. `PhysicsProfile.apply_to_state()` maps onto `MyLayoutState` fields instead.

---

## Can Layouts Be Defined in WASM?

**Short answer**: not directly as a `Layout<S>` impl, but yes through a host-side dispatch layer.

The constraint is `LayoutState: SerializableAny`, which requires `'static + Any + Send + Sync`.
A WASM module's exported functions are `extern "C"` ABI — they cannot implement Rust traits.
A WASM mod cannot hand Graphshell a `Box<dyn Layout<S>>` directly.

The solution is a **host-side WASM layout adapter** — a native Rust struct that implements
`Layout<S>`, but whose `next()` delegates to a loaded WASM function via the extism (or wasmtime)
guest/host ABI:

```rust
// registries/layout/wasm_layout.rs (host side, compiled Rust)

pub struct WasmLayoutAdapter {
    plugin: extism::Plugin,       // loaded WASM module
    state: WasmLayoutState,       // serializable state: node positions + opaque mod state
}

impl Layout<WasmLayoutState> for WasmLayoutAdapter {
    fn next<...>(&mut self, g: &mut Graph<...>, ui: &egui::Ui) {
        // 1. Serialize graph positions to msgpack/JSON
        // 2. Call WASM fn: compute_layout(positions_in) -> positions_out
        // 3. Apply returned positions back to graph nodes
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WasmLayoutState {
    pub node_positions: HashMap<NodeKey, [f32; 2]>,
    pub opaque_mod_state: Vec<u8>,   // WASM mod's own serialized state
}
impl LayoutState for WasmLayoutState {}
```

The WASM mod exports a single pure function:

```rust
compute_layout(positions: &[NodePos], edges: &[Edge], state: &[u8]) -> LayoutResult
```

All semantics live in the WASM module. The host adapter handles serialization and applies the
result. The WASM mod can implement any algorithm — Barnes-Hut, golden ratio tiling, organic
fractal decomposition, Verse-aware spatial sync — as long as it produces node positions.

**What the WASM mod runtime gives us** (`ModType::Wasm` is already scaffolded in
`registries/infrastructure/mod_loader.rs`; the extism runtime itself is not yet wired):

- Sandboxed execution — a misbehaving layout mod cannot crash the host
- Capability restrictions — `ModCapability` controls what the mod can access
- Hot-swap at runtime: unload old plugin, load new `.wasm`, instantiate new `WasmLayoutAdapter`
- Cross-platform / cross-architecture: the same `.wasm` file works on desktop and mobile

**The hot-swap question** is precisely this gap: egui_graphs `GraphView` is monomorphic over
`L: Layout<S>` — the type parameter is compile-time only. To hot-swap layout at runtime without
recompilation, Graphshell needs a stable **enum dispatch layer** or a **type-erased wrapper** that
always presents the same concrete type to `GraphView` while delegating to the currently active
layout strategy:

```rust
// One stable concrete type that GraphView always sees
pub struct ActiveLayout {
    inner: Box<dyn DynLayout>,   // trait object for runtime dispatch
}

impl Layout<ActiveLayoutState> for ActiveLayout {
    fn next<...>(&mut self, g, ui) { self.inner.next_dyn(g, ui) }
    fn state(&self) -> ActiveLayoutState { self.inner.state_dyn() }
    fn from_state(state: ActiveLayoutState) -> Self { ... }
}
```

Or as a sum type if the set of built-in layouts is finite at compile time:

```rust
pub enum ActiveLayout {
    ForceDirected(GraphPhysicsLayout),
    Hierarchical(LayoutHierarchical),
    Radial(RadialLayout),
    Timeline(TimelineLayout),
    Wasm(WasmLayoutAdapter),
}
```

The sum type is simpler, faster, and avoids `dyn` overhead. The `Wasm` variant handles true
runtime extensibility. Built-in layouts are compile-time selections. `ActiveLayoutState` is the
union of all layout states — persisted together, the active variant determines which fields are
used.

---

## Canvas Editor Layer: rapier2d

The `ExtraForce` system handles forces on graph nodes as abstract 2D points. For richer simulation
— regions with rules, surfaces with friction, inter-node collisions, spring constraints, fluid
simulation — a separate **Canvas Editor layer** makes sense. The right crate is **rapier2d**.

**rapier2d key capabilities**:

- Rigid body simulation with articulated joints and spring-damper constraints
- Collision detection via shapes (circles, convex hulls, polylines, heightfields)
- Force fields via physics hooks trait — per-frame custom force/impulse injection
- Continuous collision detection (CCD) for fast-moving bodies
- Deterministic simulation (given same inputs, same outputs — important for Verse sync)
- WASM-compatible: official JS bindings, Rust compiles to WASM natively

**How it fits into Graphshell's layout architecture**:

The `ExtraForce` system and rapier2d are **not competing** — they operate at different levels:

| Layer | What it is | Who owns it |
| --- | --- | --- |
| `ExtraForce` extras | Per-frame force injection into egui_graphs' FR pipeline | `graph/forces/` |
| rapier2d `PhysicsWorld` | Full rigid-body simulation with regions, rules, surfaces | `graph/physics_world.rs` |
| `Layout<S>` rapier adapter | Custom `Layout<S>` impl that drives egui_graphs node positions from rapier2d body positions | `graph/layouts/rapier.rs` |

The adapter works by maintaining a rapier2d `RigidBody` for each graph node and reading body
positions back into node locations each frame via `Layout::next()`. Users configure the physics
world through the **Canvas Editor** (a future Graphshell panel), not by tuning raw
force parameters.

**What "regions, rules, surfaces" means in this context**:

- **Regions**: rapier2d sensor colliders — a bounding area that applies a custom force to any body
  inside it (e.g., a repulsion well, an attraction basin, a "sticky" zone that slows nodes down).
  Registered in the physics world as sensor bodies; checked in the physics hook each frame.
- **Rules**: per-body or per-pair constraints — spring joints, distance constraints, gear
  constraints. Graphshell exposes these as named edge types ("spring edge", "rigid link",
  "elastic band") in addition to semantic graph edges.
- **Surfaces**: static colliders — floor, walls, curved surfaces. Nodes bounce off them. Useful
  for "bounded canvas" mode where the graph cannot drift off the viewport edge.

**Persistence for Verse layouts**: rapier2d simulation state is serializable. A named Verse layout
is a `PhysicsWorldSnapshot` (body positions, velocities, constraint parameters, region definitions)
that persists across sessions and can be shared with peers. When a peer opens the same workspace,
they restore the same physics world state and continue from there. This is a natural fit with the
Verse bilateral sync model.

**Relevant crates**:

| Crate | Role | Notes |
| --- | --- | --- |
| `rapier2d` | Full 2D physics engine | `0.28.0+`; WASM-compatible; replaces nphysics |
| `parry2d` | Collision geometry | Used by rapier internally; also useful standalone |
| `rstar` | R*-tree spatial index | O(log n) nearest-neighbor; dynamic insert/delete; for region queries |
| `kddo` `kiddo` | K-d tree KNN | SIMD-accelerated; for force field range queries |

---

## Fractal and Geometric Layout Algorithms

These all implement `Layout<S>` and live in `graph/layouts/`. None require rapier2d — they are
purely positional algorithms.

### Golden Ratio Recursive Polygon (Phyllotaxis / Fibonacci Spiral)

Nature's packing algorithm. Nodes placed at successive golden-angle offsets on an expanding spiral:

```text
angle_n = n * 137.508°   (golden angle = 360° / φ²)
radius_n = scale * sqrt(n)
pos_n = (radius_n * cos(angle_n), radius_n * sin(angle_n))
```

At small n this produces a tight spiral core; at large n it fills a disk without gaps.
No crate needed — the formula is five lines. The result looks like a sunflower head.

**Oriented inward or outward**: "inward" means the most recently added node is at the center
(most important / most recent floats to the top of the spiral). "Outward" means the oldest node
is at the center (root-first ordering). Toggle via a `SpiralOrientation` field in state.

**As a hierarchical queue**: assign nodes to spiral positions by a priority or recency score.
The center slot is the highest-priority node. Reprioritizing a node moves it to a new spiral
position; all other nodes shift accordingly. This gives you a spatial priority queue where
"position in the graph" directly encodes importance — the nearer to center, the more salient.
Excellent fit for a "focus queue" or "working memory" visualization.

**State**:

```rust
pub struct PhyllotaxisState {
    pub scale: f32,
    pub orientation: SpiralOrientation,  // Inward | Outward
    pub priority_key: PriorityKey,       // Recency | Score | Manual
    pub node_order: Vec<NodeKey>,        // current priority-sorted ordering
}
```

No crate required. The `l-system-fractals` crate exists for L-system grammars but is not needed
here — phyllotaxis is a closed-form formula, not a grammar.

### Penrose / Aperiodic Tiling

Nodes placed at vertices of a Penrose tiling (P2 kite-dart or P3 rhombus variant). Tilings are
generated by recursive subdivision (deflation): a P3 rhombus subdivides into smaller rhombi
following golden-ratio proportions. The tiling is non-periodic — it never exactly repeats — but
locally structured. Provides a "more than grid, less than organic" feel.

**Generation**: no Rust crate for Penrose tiling exists. Implementation: recursive subdivision
using geometric transforms with golden ratio scaling. ~150 lines of geometry code.

The aperiodic structure means no two regions look identical, which helps users build spatial
memory of where nodes are — a key usability property for a knowledge graph.

### L-System Fractal Layout

Nodes placed along the path of an L-system fractal expansion. The fractal grammar determines
the spatial path; node index determines position along that path. Different grammars produce
different path topologies (Hilbert curve, Koch snowflake, dragon curve, space-filling curves).

`l-system-fractals` (`crates.io`) provides grammar parsing and turtle graphics expansion.
The path points become node target positions.

**Hilbert curve variant**: assigns 2D positions via Hilbert curve index, giving cache-coherent
spatial locality. Nodes that are "close" in Hilbert index are "close" spatially, and vice versa.
For very large graphs (10k+ nodes) this is significantly better than random scatter for visual
coherence.

### Radial / Concentric

BFS from a focal node; nodes on ring n are at graph-theoretic distance n. Angular spacing by
degree or alphabetic. Clean, symmetric. Natural for "explore from this node" mode.

No crate needed — BFS is in petgraph, radial positioning is trigonometry.

### Timeline / Temporal

Nodes placed on a horizontal time axis by `created_at` or `last_visited`. Vertical position by
UDC cluster or domain group. Edges as arcs above the axis. Not force-directed — positions
computed analytically from timestamps.

---

## Mobile Portability

Both the physics presets and the custom layout algorithms are mobile-portable by default because:

- `Layout<S>::next()` is pure math + mutable graph positions. No platform I/O. No `std::time`
  (use `ui.ctx().input(|i| i.time)` from egui, which uses `web_time` internally — WASM-safe).
- `ExtraForce::apply()` is the same — pure Vec2 arithmetic.
- `rapier2d` has official WASM bindings and compiles to `wasm32-unknown-unknown`.
- The phyllotaxis, Penrose, and L-system algorithms are pure math. No OS dependencies.

**Touch input gap**: egui has `MultiTouchInfo` with `zoom_delta` and `rotation_delta` for
pinch-to-zoom and two-finger rotation. egui_graphs does not currently expose touch-specific
handling — it routes touch events through the pointer abstraction. Graphshell's pre-render
input interception (Phase 1 of the interaction plan) should check `ui.input(|i| i.multi_touch())`
and map pinch → zoom, two-finger drag → pan, without requiring Ctrl. This is a `render/mod.rs`
change, not a layout change, but it gates mobile usability.

**Render cost**: the phyllotaxis and L-system layouts are static (positions computed once, not
per-frame), so they are inherently cheaper than continuous FR simulation on mobile. The
`physics:void` preset (no forces, static positions) is the natural "mobile default" for large
graphs.

---

## Crate Landscape Summary

| Category | Crate | Version | WASM | Notes |
| --- | --- | --- | --- | --- |
| 2D physics engine | `rapier2d` | 0.28+ | Yes | Spring-damper joints, physics hooks, regions |
| Collision geometry | `parry2d` | latest | Likely | Used by rapier; standalone for shape queries |
| Spatial index (R*-tree) | `rstar` | 0.12.2 | Likely | Dynamic insert/delete; O(log n) range/NN |
| Spatial index (k-d tree) | `kiddo` | 5.2.2 | Likely | SIMD KNN; rkyv serialize |
| Barnes-Hut n-body | `barnes_hut` | latest | Likely | O(n log n) FR variant; quadtree approx |
| Force-directed (alt) | `fdg-sim` | latest | Likely | Standalone FD framework; ForceAtlas2 |
| L-system fractals | `l-system-fractals` | latest | Likely | Grammar + turtle graphics path |
| WASM mod runtime | `extism` | — | Yes | Guest/host ABI for sandboxed WASM mods |
| WASM mod runtime (alt) | `wasmtime` | — | — | Lower-level; more control; heavier |

`extism` is the planned WASM mod runtime per the registry architecture. The `ModType::Wasm`
variant is already scaffolded in `registries/infrastructure/mod_loader.rs`; the runtime itself
is not yet wired.

---

## Layout Algorithm Reference Table

| Algorithm | Level | Crate needed | Mobile | Best for |
| --- | --- | --- | --- | --- |
| FR + extras (current) | 2 | none (egui_graphs) | Yes | General organic layout |
| Barnes-Hut FR | 3 | `barnes_hut` or custom | Yes | Large graphs (>500 nodes) |
| Hierarchical (Sugiyama) | — | none (egui_graphs) | Yes | DAGs, citation graphs |
| Radial / Concentric | 3 | none (petgraph BFS) | Yes | Explore-from-node mode |
| Timeline / Temporal | 3 | none | Yes | Traversal archive view, node history axis |
| Kanban / Column Projection | 3 | none | Yes | Status-tag bucketing, workflow stages |
| Map / Geospatial Projection | 3 | none (custom) | Yes | Lat/long node placement from metadata |
| Grid / Snapped Grid | 3 | none | Yes | Structured note-taking |
| Constraint-Based / Elastic | 3 | `rapier2d` | Yes | Zone-pinned layout, Verse sync |
| Phyllotaxis / Fibonacci Spiral | 3 | none | Yes | Priority queue, recency ring |
| Penrose / Aperiodic Tiling | 3 | none (custom geometry) | Yes | Spatial memory, no-repeat grid |
| L-system Fractal Path | 3 | `l-system-fractals` | Yes | Topological path structures |
| Semantic Embedding (UMAP-style) | 3 | none (custom) | Partial | UDC/topic proximity layout |
| rapier2d Canvas Editor | 3 | `rapier2d` | Yes | Full scene editor, Verse layouts |
| WASM layout mod | 3 | `extism` host adapter | Yes | Runtime-hotswappable, sandboxed |

---

## Module Scope Boundary

The key boundary: algorithm implementation is always a plain module. The `inventory::submit!`
registration is the native mod seam — the line between "internal algorithm" and "user-visible
named capability in the registry."

| What | Location | Why |
| --- | --- | --- |
| Type aliases (`GraphPhysicsState`, etc.) | `graph/physics.rs` | Plain module — naming only |
| `ExtraForce` impls | `graph/forces/` | Plain module — pure math |
| `PhysicsProfile` presets | `app.rs` → `registries/presentation/physics_profile.rs` | Data presets; registry migration Phase 6 |
| `Layout<S>` custom engines | `graph/layouts/` | Plain module — algorithm impl |
| Named `layout:*` in `LayoutRegistry` | `graph/layouts/<name>.rs` + `inventory::submit!` | **Native mod scope** — only for user-visible Lens options |
| `WasmLayoutAdapter` | `registries/layout/wasm_layout.rs` | Native host adapter; loads WASM plugin |
| `ActiveLayout` enum dispatcher | `graph/layouts/active.rs` | Hot-swap seam; always one concrete type in `GraphView` |
| rapier2d physics world | `graph/physics_world.rs` | Plain module — no registry coupling |
| Canvas Editor UI | `desktop/panels/canvas_editor.rs` | Desktop layer; dispatches `GraphIntent` |

**Implement the algorithm as a plain module first.** Add `inventory::submit!` only when the
algorithm needs to appear as a named Lens option selectable by users.

---

## Integration with `PhysicsProfileRegistry`

The registry currently vends three presets as hardcoded constructors in `app.rs`. Migration path:

1. Add `graph/physics.rs` with aliases (Level 1).
2. Add `graph/forces/` with `ExtraForce` impls (Level 2).
3. Expand `PhysicsProfile` to carry `enabled` flags for each extra.
4. Add new preset constructors for the ten thematic profiles (see below).
5. Add `graph/layouts/active.rs` — `ActiveLayout` enum dispatcher + `ActiveLayoutState`.
6. Migrate `render/mod.rs` to use `ActiveLayout` / `ActiveLayoutState` as the concrete type.
7. Move presets to `registries/presentation/physics_profile.rs` (Phase 6 cleanup).
8. Wire `PhysicsProfileRegistry` into `LensCompositor` resolution (Phase 6.2).

Steps 1–4 are `app.rs` + new file changes. Step 5–6 introduce the hot-swap seam. Steps 7–8 are
Phase 6 registry work.

---

## Ten `ExtraForce` Thematic/Topological Physics Presets

These are named `PhysicsProfile` presets backed by `ExtraForce` extras, registerable as
`physics:*` IDs. All build on Level 2 — no engine replacement required.

### 1. `physics:liquid` (existing, refined)

**Theme**: Organic, free-flowing exploration.
**Extras**: `CenterGravity` (weak), `GravityLocus` (lerp toward viewport center).
**Use case**: Default browsing mode. Nodes breathe and settle organically.

### 2. `physics:gas` (existing, refined)

**Theme**: Explosive spread.
**Extras**: `CenterGravity` off, `GravityLocus` off.
**Use case**: Blow apart a dense cluster; initial placement on import.

### 3. `physics:solid` (existing, refined)

**Theme**: Stable, domain-clustered.
**Extras**: `CenterGravity` (medium), `DomainCluster` on, `DegreeRepulsion` on.
**Use case**: Working set of familiar sites; settles into a domain map.

### 4. `physics:archipelago`

**Theme**: Islands of related content with clear open water between them.
**Extras**: `DomainCluster` (strong), `DegreeRepulsion` on, `GravityLocus` off.
**Use case**: Research session with multiple distinct topic threads.

### 5. `physics:constellation`

**Theme**: Hub-and-spoke — high-degree nodes become spatial attractors.
**Extras**: `DegreeRepulsion` (strong), `HubGravity` (gravity scales with log(degree)).
**Use case**: Scale-free graphs; social/reference graphs with dominant link targets.

### 6. `physics:crystal`

**Theme**: Grid-aligned, minimal motion.
**Extras**: `GridSnap` (spring force toward nearest grid position).
**Use case**: Structured note-taking, Zettelkasten with spatial discipline.

### 7. `physics:tide`

**Theme**: Slow rhythmic current — never fully settled.
**Extras**: `GravityLocus` target oscillates on a slow sine (`ui.ctx().input(|i| i.time)`).
**Use case**: Ambient display / "living graph" mode.

### 8. `physics:sediment`

**Theme**: Topological depth drives vertical position — roots float, leaves sink.
**Extras**: `DepthGravity` (downward force proportional to BFS depth from root nodes).
**Use case**: Directed graphs with identifiable roots; reading-flow visualization.

### 9. `physics:magnet`

**Theme**: User-defined magnetic zones dominate; FR is a secondary tie-breaker.
**Extras**: `ZoneGravity` (reads `GraphWorkspace.zones`), `CenterGravity` (weak fallback).
**Use case**: Explicitly organized nodes in named zones. Requires Phase 3 Magnetic Zones.
**Dependency**: `2026-02-24_layout_behaviors_plan.md` Phase 3.

### 10. `physics:void`

**Theme**: No forces. Fully manual positions.
**Extras**: None. `is_running = false`. All strengths zero.
**Use case**: Manual spatial arrangement; pin-and-place Zettelkasten; "lock this layout."

---

## Risks

**Tuple arity grows**: Beyond ~6 extras, the type alias becomes unwieldy. Mitigation: group
related extras into a composite (e.g., `ClusteringBundle` combining `DomainCluster` +
`DegreeRepulsion`).

**`apply_to_state()` grows linearly**: Use a structured helper once the list exceeds ~5 extras.

**Level 3 state serialization**: Custom `LayoutState` must satisfy `SerializableAny` (serde
derives sufficient). Test roundtrip persistence before committing to a custom state type.

**ActiveLayout enum grows**: Adding a new built-in layout adds a variant. Fine for a bounded set
(~8–10 layouts). If the set is unbounded, prefer the `Box<dyn DynLayout>` approach — accepting
the `dyn` dispatch cost.

**rapier2d simulation divergence on Verse sync**: Deterministic simulation requires identical
inputs across peers. Frame timing differences can cause divergence. Mitigation: sync position
snapshots periodically rather than relying on deterministic replay.

**Tide / animated presets clock**: Use `ui.ctx().input(|i| i.time)` — this is `web_time`-backed
in egui and safe on WASM and mobile. Do not use `std::time::Instant`.

**WASM layout ABI stability**: The `compute_layout` guest function signature must be stable across
mod versions. Define a versioned msgpack schema for `LayoutRequest` / `LayoutResult` and version
it with the `ModManifest` `provides` string (e.g., `layout-wasm-api:1`).

---

## 2D↔3D Hotswitch Architecture

The project vision (PROJECT_DESCRIPTION.md) includes first-class 2D↔3D switching with position
parity — the same graph, same node relationships, same relative clustering, but rendered in a
richer spatial context. This section defines how that fits into the physics and layout
architecture developed above.

---

### Perspective Modes

**2D** — Standard planar graph.

**2.5D** (formerly Soft 3D) — non-reorientable camera; always top-down with a fixed perspective projection
adding a slight depth illusion. Nodes at different z-depths render smaller and slightly offset.
No camera tilt. The 3D effect is purely visual — it adds depth cues without changing navigation.
The user still navigates in 2D (pan/zoom); the z-axis is decorative / semantic. Easiest to
implement; easiest to use; mobile-compatible.

**Isometric** (formerly Stacked 3D) — layers of depth, not arbitrary z. The graph is organized into discrete
depth layers (e.g., z = 0, 1, 2, ... by BFS depth or UDC category). The camera uses an isometric or
fixed-angle projection to reveal layer separation. Z is quantized. Less overwhelming than full 3D;
layer separation makes structure legible. Natural for hierarchical and temporal graphs.

**3D** (formerly Full 3D) — reorientable camera, full six-degree-of-freedom navigation. Nodes placed in
genuine 3D space. The user can orbit, zoom, and tilt the view arbitrarily. The z-axis carries
semantic meaning (e.g., depth by recency, semantic layer, traversal depth). Highest fidelity;
most disorienting for dense graphs.

---

### Position Parity Contract

The invariant across all three modes and all switches:

> **The (x, y) position of a node in 2D space maps to the (x, y) position of that node in 3D
> space. The z-coordinate is computed from node metadata, not from the physics engine.**

This means:

- Switching 2D → 3D: node (x, y) positions are preserved exactly. A z-coordinate is assigned
  from a configurable `ZSource` (recency, BFS depth, UDC level, manual, zero).
- Switching 3D → 2D: the (x, y) positions are read directly from the 3D node positions. The
  z-coordinate is discarded. Any physics that ran in 3D mode (rapier3d) projected onto (x, y)
  is preserved.
- The switch is instantaneous — no animation required on the position data, though a camera
  transition animation is appropriate.

The z-coordinate is **not stored in the physics state** — it is a pure function of node metadata
computed at render time. This means:

1. The physics engine (rapier2d or rapier3d) never needs to know about the current 3D mode.
2. Position parity requires no conversion — (x, y) is the same in both spaces.
3. Switching modes does not invalidate any persisted layout state.

```rust
pub enum ZSource {
    Zero,                          // All nodes coplanar — soft 3D visual effect only
    Recency { max_depth: f32 },    // Recent nodes float to front
    BfsDepth { scale: f32 },       // Root nodes at z=0, deeper nodes further back
    UdcLevel { scale: f32 },       // UDC main class determines z layer
    Manual,                        // Per-node z override from node metadata
}
```

`ZSource` is part of `GraphViewState` — it is a per-view configuration, not a global one.
A Canonical view can show the graph in 2D while a Divergent view shows it in Soft 3D with
`ZSource::BfsDepth`, all from the same underlying (x, y) positions in `app.graph`.

---

### Render Backend: egui_glow + Custom OpenGL Pass

The current render backend is `egui_glow` (OpenGL via the `glow` crate, version 0.16.0).
egui's `Painter::add(Shape::Callback(PaintCallback { ... }))` mechanism allows custom OpenGL
draw calls to be injected at a specific Z-order within the egui paint list.

This is the natural 3D render path:

1. The egui UI (Workbench chrome, pane borders, tab strips) renders via egui's 2D pipeline.
2. For graph panes in 3D mode, a `PaintCallback` issues a custom OpenGL draw call that renders
   the 3D graph using the glow API directly — lines for edges, instanced quads or billboards
   for nodes, depth buffer enabled.
3. Node interaction (hit-testing, hover, selection) requires projecting the mouse position into
   3D space using the current MVP matrix — this replaces egui_graphs' 2D hit-testing for 3D views.

**Why not wgpu?** wgpu is in the Cargo.lock as a transitive dep (likely from Servo/mozjs), but
Graphshell's own render path uses `egui_glow`. Introducing wgpu as a first-class dependency just
for 3D would add a second GPU API with its own state management. The glow path is already in use;
extending it is lower cost.

**Why not bevy?** Bevy is a full game engine with its own ECS, asset pipeline, windowing, and
event loop. Embedding it inside an egui application is architecturally hostile — the two event
loops cannot share a window without significant plumbing. Bevy is not a viable option here.

**Why not `three-d`?** `three-d` (crates.io) is a pure-Rust 3D rendering library that sits above
OpenGL/wgpu. It is WASM-compatible and has egui integration examples. It is a viable option for
2.5D and Isometric modes, where a full scene graph is not required. For 3D mode with a complete
scene, `three-d` reduces boilerplate significantly. Worth evaluating as the 3D render layer for
Isometric and 3D modes. 2.5D can be done with raw glow calls given its simplicity.

---

### rapier2d → rapier3d Upgrade Path

rapier2d and rapier3d are the same library at different type parameterizations (`rapier2d` vs
`rapier3d`). The physics hook API, joint system, and body/collider model are identical — only
the vector type changes (`Vec2` → `Vec3`, `Rotation2` → `Rotation3`/`UnitQuaternion`).

For the **rapier2d Canvas Editor layer** described earlier (regions, rules, surfaces):
the entire API translates directly to rapier3d with `z = 0` for all bodies — a 3D simulation
constrained to the z=0 plane is identical to a 2D simulation. This means:

- Phase 1: build the Canvas Editor against rapier2d.
- Phase 2: when 3D mode is introduced, upgrade the physics world to rapier3d. Bodies that
  were at z=0 remain at z=0 in 3D space — no positional migration required. Add z-axis
  forces/constraints only for the features that need them (e.g., z-layer gravity, vertical
  springs between layers in Isometric mode).

The `Layout<S>` rapier adapter (`graph/layouts/rapier.rs`) wraps the physics world. The
adapter's `LayoutState` carries `ViewDimension` — when the view is 2D, the adapter constrains
bodies to z=0; when 3D, it allows z movement. The constraint is a zero-length slider joint
along the z-axis, trivially added or removed.

---

### `ViewDimension` on `GraphViewState`

Extend `GraphViewState` from the multi-graph pane plan:

```rust
pub enum ViewDimension {
    TwoD,
    ThreeD { mode: ThreeDMode, z_source: ZSource },
}

pub enum ThreeDMode {
    Standard, // 3D: reorientable camera, arbitrary z
    Isometric,// Isometric: quantized z layers, fixed angle/tilt
    TwoPointFive, // 2.5D: fixed top-down perspective, z is visual only
}

pub struct GraphViewState {
    // ... existing fields ...
    pub dimension: ViewDimension,      // new
}
```

The switch intent:

```rust
GraphIntent::SetViewDimension {
    view: GraphViewId,
    dimension: ViewDimension,
}
```

The reducer handles the switch:

1. If transitioning to 3D: compute z-coordinates from `ZSource` for all nodes; store in a
   per-view `z_positions: HashMap<NodeKey, f32>` (ephemeral — recomputed on each switch from
   node metadata, never persisted as a separate field).
2. If transitioning to 2D: discard z-positions. (x, y) is unchanged.
3. Camera state transitions: preserve pan (x, y) translation; reset tilt/orbit for 2D; restore
   last orbit for 3D.

`z_positions` is not persisted — it is a pure function of `ZSource` + node metadata and is
recomputed on restore. Only `ViewDimension` (including `ZSource` variant and params) is persisted
with the view state.

---

### Interaction in 3D Modes

**2.5D**: interaction is identical to 2D. The perspective projection is applied only in the
render pass; all hit-testing, hover, selection, and camera control operate in 2D screen space.
This is the zero-cost 3D mode from an interaction standpoint.

**Isometric**: pan and zoom operate in the (x, y) plane as before. A tilt gesture (two-finger
vertical drag on touch; middle-mouse drag on desktop) adjusts the view angle to reveal layer
separation. Node selection requires a ray-cast from screen to the closest node (by projected
distance). Keyboard navigation between layers (PageUp/PageDown) is a natural addition.

**3D**: full orbit controls (arcball camera). Mouse-drag orbits; scroll zooms; pan requires
a modifier. Selection requires a proper 3D ray-cast against node bounding volumes. This is the
highest-complexity interaction mode and should be the last implemented.

---

### Implementation Sequence

1. **2.5D** — purely visual z-offset in the render pass; z computed from `ZSource::Recency`
   or `ZSource::BfsDepth`. No camera changes. No interaction changes. One `PaintCallback` that
   draws nodes at their (x, y) position with a slight depth offset. ~200 lines of glow code.
   Mobile-compatible.

2. **Isometric** — quantized z layers. Camera tilt via `three-d` or raw glow MVP matrix.
   Layer-aware ray-cast for selection. Keyboard layer navigation.

3. **3D** — arcball camera, full z freedom, `three-d` for scene management. rapier3d
   optional (required only if Canvas Editor needs z-axis forces).

The `ViewDimension` type and `SetViewDimension` intent are added in step 1 and extended in
steps 2–3. The position parity contract holds from the beginning — Soft 3D proves it.

---

### Crate Additions for 3D

| Crate | Role | Notes |
| --- | --- | --- |
| `three-d` | 3D scene + rendering, egui integration | Reduces boilerplate for Isometric and 3D |
| `rapier3d` | 3D physics (3D mode + Canvas Editor z-axis) | Same API as rapier2d |
| `glow` (existing) | OpenGL draw calls for 2.5D | Already in Cargo.toml at 0.16.0 |

---

## Rapier as a Semantic Scene Composer

### What Rapier Is (and Isn't)

rapier is a **deterministic rigid-body physics engine running entirely on the CPU**. It has no
GPU dependency and no awareness of rendering. It does not know what a graph node is. What it
provides is a `PhysicsWorld` where you register `RigidBody` objects, attach `Collider` shapes to
them, connect them with `Joint` constraints, and define `Sensor` volumes that trigger callbacks
when bodies enter or leave them. Each simulation step, you call `world.step()` and read updated
`RigidBody.translation()` values back into your own data structures.

The integration with egui_glow is **zero-coupling**: rapier computes positions on the CPU;
egui_glow draws them on the GPU via OpenGL. There is no shared pipeline. The only integration
point is one position read per node per frame — extract `body.translation()` and write it to
`node.location()` before the egui_graphs `GraphView` renders. This is O(n) and effectively free
at any graph size that fits in a frame budget.

There is no efficiency concern here. rapier is among the fastest physics engines available in
any language. At 500 nodes, a full simulation step takes under 1ms on a modern CPU. The
per-frame cost is dominated by egui_graphs' render pass, not by rapier's step.

---

### Nodes as Semantic Objects in a Rapier Scene

Each graph node maps to a rapier `RigidBody`. The body's physical properties can be derived
directly from semantic node metadata:

| Node property | Rapier body property | Effect |
| --- | --- | --- |
| Node degree (link count) | `mass` — scales with log(degree) | High-degree hubs are harder to move; satellites orbit them |
| Node recency | `linear_damping` — recent nodes: low, cold nodes: high | Fresh nodes are "alive"; cold nodes settle quickly |
| UDC category | `collision_groups` — same-category nodes share a group | Nodes in the same semantic category can be configured to collide or pass through each other |
| Pin state | Body type: `Fixed` | Pinned nodes become immovable anchors; all forces act around them |
| Node lifecycle (warm/cold/archived) | `dominance_group` — warm nodes dominate cold ones | Warm nodes push cold nodes aside spatially |
| Manual zone membership | Attached to a `FixedJoint` anchored at zone centroid | Zone-bound nodes are spring-pulled toward their zone, not the global gravity locus |

This is the **semantic scene** model: every physical property of the simulation is a function
of graph semantics, not arbitrary tuning values. The physics editor becomes a semantic editor —
"how much does link count affect mass?" rather than "what is body 47's mass?".

---

### Edges: Spring Joints vs. Visual-Only Lines

Edges have two independent representations:

**Physics edges** — rapier `SpringJoint` (or `RopeJoint` for maximum distance only). These
apply attractive forces between connected bodies. They have rest length, stiffness, and damping
parameters. Enabling a physics edge between two nodes causes them to pull toward each other.

**Visual edges** — the line drawn by `GraphEdgeShape` in the egui_graphs render pass. This is
a render-only concern.

The two are toggled independently:

```rust
pub enum EdgePhysicsMode {
    Spring { stiffness: f32, rest_length: f32, damping: f32 },
    None,   // nodes connected in graph topology but no physics force between them
}

pub enum EdgeVisualMode {
    Visible,
    Hidden,
    Ghost,  // faint, de-emphasized — visible but not salient
}
```

Toggling `EdgeVisualMode::Hidden` hides the line without removing the joint. Toggling
`EdgePhysicsMode::None` removes the spring force without hiding the line. These are independent
user controls, both expressible as `GraphIntent` variants and serializable in the `LensConfig`.

A "pure force" layout mode (e.g., `physics:constellation`) might use spring joints for structural
edges but hide their visual lines to reduce clutter — showing only the emergent spatial clusters,
not the explicit connections. A "pure topology" mode shows visual edges but disables springs —
the graph looks like a traditional node-link diagram but positions are computed by FR rather
than spring forces.

---

### Sensor Regions: Physics Rules in Space

rapier `Collider`s with `sensor = true` fire `ContactEvent` callbacks when a rigid body enters
or exits their volume. This is the foundation for the **physics regions** system:

```rust
pub struct PhysicsRegion {
    pub id: Uuid,
    pub name: String,
    pub shape: RegionShape,       // Circle, Rectangle, Polygon
    pub centroid: Pos2,
    pub rule: RegionRule,
}

pub enum RegionRule {
    GravityWell { strength: f32 },           // attract all bodies inside
    RepulsionField { strength: f32 },         // repel all bodies inside
    Friction { coefficient: f32 },            // slow bodies passing through
    Boundary,                                 // hard wall — bodies cannot exit
    SemanticFilter { predicate: TagFilter },  // only applies to nodes matching a UDC filter
}
```

`SemanticFilter` is the key feature: a `GravityWell` region that only affects nodes tagged
with `UDC:5` (Mathematics) acts as a semantic magnet — math nodes are pulled into the region,
all other nodes pass through unaffected. This gives the user a spatial vocabulary for semantic
organization that goes beyond parameter tweaking into genuine spatial rule authoring.

Regions are persisted in `GraphWorkspace.physics_regions`, survive snapshot roundtrip, and sync
over Verse. A Verse layout is a `PhysicsWorldSnapshot` that includes region definitions — peers
who open the workspace restore the same spatial rules and continue from there.

---

### The Functional Physics Editor

The Canvas Editor is a Graphshell panel (`desktop/panels/canvas_editor.rs`) that lets users
author the physics scene directly:

- **Body panel**: select a node, inspect its physical properties, override mass/damping/collision
  group, set pin state. All changes dispatch `GraphIntent::SetNodePhysicsOverride`.
- **Region panel**: draw a new region on the canvas (lasso/circle tool), assign a `RegionRule`,
  name it. Dispatches `GraphIntent::CreatePhysicsRegion`.
- **Joint panel**: select two nodes, create a `SpringJoint` between them with configurable
  stiffness. This is a physics-only edge — it adds a force relationship without adding a graph
  edge. Dispatches `GraphIntent::CreatePhysicsJoint`.
- **Profile panel**: activate a named `PhysicsProfile` preset, or save current world parameters
  as a new named preset for the `PhysicsProfileRegistry`.

The editor is a **level designer for knowledge space** — the mental model is closer to placing objects
in a game level than tuning numbers in a settings panel. It allows users to "paint" or "draw" layout
constraints, and edit the scene itself, making it a powerful tool for **theming the canvas**.

---

### rapier + egui_glow: No GPU Conflict

rapier is CPU-only. egui_glow is GPU-only (OpenGL). burn-wgpu (when integrated) is GPU-only
(wgpu compute, separate context). These three pipelines never share state:

```
CPU:  rapier step()  →  positions[]  →  egui_graphs Layout::next()
GPU:  egui_glow      →  OpenGL draw calls  (reads positions from CPU)
GPU:  burn-wgpu      →  WebGPU compute     (reads node embeddings, writes similarity scores)
```

There is no GPU memory sharing between egui_glow's OpenGL context and burn's wgpu context.
This is fine — the output of burn (similarity scores, embeddings) flows back to the CPU as
`Vec<f32>`, where rapier consumes them as force parameters (semantic physics hook). Neither
engine needs to read the other's GPU memory.

---

### Will We Need wgpu for burn?

burn's wgpu backend (`burn-wgpu`) uses wgpu as a compute backend — it is not a render backend.
It creates its own `wgpu::Device` and `wgpu::Queue` independently of egui_glow's OpenGL context.
These coexist in the same process with no conflict. Graphshell does not need to switch its render
backend from egui_glow to wgpu to use burn.

The only constraint is that burn-wgpu needs a wgpu-compatible GPU adapter. On platforms where
wgpu is available (Windows/DX12, macOS/Metal, Linux/Vulkan, web/WebGPU), this is automatic.
On platforms where it is not (some older OpenGL-only drivers), burn falls back to its `ndarray`
CPU backend automatically. From Graphshell's perspective, burn is a pure compute dependency —
it has no coupling to the render path.

**The question of a future wgpu render backend for Graphshell itself** (replacing egui_glow)
is separate. egui_glow is the render backend today and is fully sufficient for 2D, Soft 3D, and
Isometric. 3D with a complex scene might eventually benefit from a wgpu render path (for
better mobile/WebGPU support, compute shaders for LOD culling, etc.). That is a future render
backend migration decision, not a burn integration concern. The two can proceed independently:

- **burn**: add as a compute dependency whenever the Tier 1 embedding model is ready. No render
  changes required.
- **wgpu render backend**: a separate, later migration from egui_glow if 3D scene
  complexity warrants it. burn's presence neither accelerates nor blocks this decision.

**Recommendation**: do not let the burn integration drive the render backend choice. Add burn
with its wgpu backend first. Evaluate whether the render backend needs to change only after 3D
is implemented and its performance characteristics are known.

---

## Research Gaps, Secondary Mod Candidates, and User Configuration Surface

This section captures open design questions, secondary systems not yet specified, and user-facing
configuration options that the architecture implies but has not yet made explicit. Items are
organized by category and should be resolved before or during each relevant implementation phase.

---

### Open Research Questions and Incomplete Design Areas

**WASM error handling and fallback**

The `WasmLayoutAdapter` section specifies the happy path but not failure recovery:

- What happens when a WASM plugin returns malformed positions (out-of-bounds coordinates, NaN)?
- What is the fallback when a WASM plugin panics or times out mid-frame?
- Is there a watchdog timeout? What layout does `ActiveLayout` revert to on timeout?
- How does the user know a WASM layout failed (UI feedback vs. silent fallback)?

Suggested approach: validate returned positions, clamp out-of-bounds values, revert to
`ActiveLayout::FruchtermanReingold` on panic/timeout, surface a per-pane status indicator.

**`ActiveLayout` enum growth policy**

The enum will accumulate variants over time. Design questions:

- At what point does it warrant a plugin registration table instead of an enum?
- Should WASM layouts and native layouts share one `ActiveLayout` enum, or separate registries?
- How are layout variants versioned for snapshot persistence? (A renamed variant breaks
  deserialization of saved workspaces.)

Suggested: define a stable string key per variant for serialization independent of enum variant
name; use a `#[serde(rename = "...")]` attribute from day one.

**Rapier semantic bidirectionality**

The current model is unidirectional: node metadata → rapier body properties. Open questions:

- Can the user override a derived property (e.g., pin a node that would normally be Dynamic)?
- How are per-node manual overrides persisted? As a field on the graph node, or in a separate
  physics override map?
- When node metadata changes (e.g., recency score updates), does the rapier body property update
  live (requiring a body mutation) or only on next simulation restart?
- Bidirectional sync (rapier body position → graph node position) is already described; but what
  about rapier → node metadata (e.g., collision events marking a node as "active")?

**Rapier region user interaction model**

The `PhysicsRegion` data model is defined but user interaction is not:

- How does the user draw/place a region? Lasso selection? Drag a shape from a palette?
- Can regions overlap? What is the precedence order for conflicting rules?
- Are regions per-view (local to a `LocalSimulation`) or global (written to `app.graph`)?
- Regions attached to Divergent views should not persist to the global graph unless explicitly
  committed (matching `CommitDivergentLayout` semantics).

**Touch input gesture mapping**

Mobile portability section notes that touch input is a gap but does not specify:

- Which gestures map to which layout interactions (pinch-to-zoom, two-finger pan,
  long-press for context menu, tap for select)?
- How does the drag-zone interaction work on touch (no hover, no right-click)?
- Does the physics reheat on touch-drag node, or is drag position always kinematic during touch?

Suggested: adopt the standard egui touch input abstractions (`egui::TouchPhase`,
`egui::PointerButton`) and gate touch-specific gesture logic behind a `#[cfg(target_os = "...")]`
or a capability flag in `AppPreferences`.

**2.5D visual rendering specification**

The 2.5D mode is described semantically but not visually:

- How is z rendered? Drop shadow? Node size scaling? Opacity? Parallax offset?
- What is the rendering budget for z-depth cues in egui_glow (PaintCallback overhead)?
- Are edges rendered with depth cues (thinner/more transparent for distant nodes)?

**burn embeddings → physics data flow**

The local intelligence plan describes computing cosine similarity vectors from burn. This plan
mentions using them as a `SemanticFilter` predicate or as an `ExtraForce` (semantic clustering
force). The connection is not yet specified:

- What is the data contract between `LocalIntelligenceAgent` and the physics layer?
- Is similarity a per-pair score pushed into a force lookup table, or a per-node embedding
  vector from which the force engine computes pairwise attraction at tick time?
- Per-pair pre-computation scales as O(n²). What is the practical node count limit?
- How are embeddings invalidated and recomputed when node content changes?

**`apply_to_state()` helper pattern**

`PhysicsProfile::apply_to_state()` currently maps profile fields to `FruchtermanReingoldWithCenterGravityState`.
When `GraphPhysicsState` is promoted to the full type alias (Level 2), `apply_to_state()` must
set fields for all `ExtraForce` extras. Open questions:

- Should `apply_to_state()` take a mutable reference to the extras tuple by trait method, or
  is it a concrete method on the full type alias?
- The extras tuple is heterogeneous. Does each `Extra<T, _>` expose a `set_params()` method,
  or does `apply_to_state()` hard-code the index-based field access?

**PhysicsProfile preset migration schema**

When a stored preset from an older snapshot references a profile field that no longer exists
(or gains a new required field), migration must be handled:

- Add `#[serde(default)]` to all `PhysicsProfile` fields from day one.
- Define a schema version field on `PhysicsProfile` for future migrations.
- Document the intent: presets should degrade gracefully to defaults rather than failing to load.

---

### Secondary Systems Implementable as Native Mods

These are complete feature systems that fit cleanly into the mod registry architecture and are
valuable to users, but are not part of the core physics engine. Each is a candidate for a
separate `mods/native/` module registered via `inventory::submit!`.

**Layout morphing / interpolation engine**

Smoothly interpolate node positions between two layout states (e.g., phyllotaxis → force-directed).
`ActiveLayout::Morphing { from: Box<LayoutSnapshot>, to: Box<ActiveLayout>, t: f32 }`. Uses
slerp/lerp on position vectors. The `t` value advances each frame until 1.0, then the layout
transitions. This is a pure CPU operation over position arrays — no physics engine changes
required. Useful for presentation mode and layout experimentation.

**Semantic clustering algorithms (k-means, DBSCAN)**

Cluster nodes by their burn embedding vectors (or by UDC tag similarity) into spatial groups.
The clustering algorithm runs out-of-band (not every frame), produces `cluster_id` assignments,
and those assignments feed into the `DomainCluster` or `ZoneGravity` ExtraForce as centroid
targets. k-means is simple enough to implement without external crates. DBSCAN handles
irregular cluster shapes better. Both are natural candidates for a `clustering` native mod that
integrates with both the physics layer and the UDC tagging system.

**Constraint solver for edge routing (non-overlapping edges)**

For dense graph topologies, edges frequently overlap. A post-physics constraint pass can route
edges around node bounding boxes using a simplified orthogonal routing or bundling algorithm.
This is separate from physics — it operates on the final positions after layout converges.
Candidate crates: `lyon` (path geometry), custom bundling. Gate by `CanvasRegistry.edge_routing_enabled`.

**Physics damping profile library**

A library of named damping curves (linear, exponential, spring, critically-damped) as serializable
structs that can be referenced by `PhysicsProfile.damping_profile_id`. Different from parameter
presets — these govern the shape of energy dissipation over time, not just the magnitude.
Useful for achieving specific animation aesthetics (snap-to-rest, oscillate-to-rest, etc.).

**Node glyph renderer plugins**

Replace the standard circle/rectangle node renderer with custom SVG, icon, or procedural glyph
renderers. These are render-phase mods, not physics mods, but they benefit from knowing the
active physics preset (e.g., render nodes as water droplets when `physics:liquid` is active).
Registered via the render dispatch table, not `PhysicsProfileRegistry`. Interacts with
`ThemeRegistry`.

**Gesture recognition mods (mobile and desktop)**

Recognize higher-level gestures from raw pointer/touch events: "fling to dismiss", "pinch cluster
to collapse", "shake to reheat", "two-finger tap to reset camera". These are event-layer mods
that fire `GraphIntent` variants. On desktop they can augment trackpad gestures. Cleanly
isolated from physics and layout — pure intent emitters.

**Lens-physics binding mods**

Pre-wired `(LensId, PhysicsProfileId)` binding pairs that auto-switch the physics preset when
a Lens is applied. Example: applying `lens:research` automatically activates `physics:liquid`.
This is a mod (not hardcoded) because binding policy is user-configurable. Registered as a
`LensTransitionHook` in `LensCompositor`. Default bindings are seed floor entries.

**Physics simulation recording and playback**

Record a sequence of position snapshots (one per frame) and play them back as a deterministic
animation. Useful for demonstrating layout convergence, creating "physics replay" walkthroughs,
and debugging physics divergence bugs. The recorded sequence is a separate artifact from the
graph snapshot — it records positions, not topology. Store as a compressed time series (delta
encoding + LZ4). This is entirely additive — no changes to the physics step itself.

**Texture-driven Force Fields (Flow Maps)**

Allow users to "texture" the canvas with forces by importing an image or painting directly on the
background. Brightness = gravity strength, or RGB = vector direction (flow map).
Implementation: `ExtraForce` that samples a `TextureHandle` at `node.position`.
Useful for artistic layouts or guiding flow in a specific pattern (e.g. "river" layout).

**Audio-reactive layout (speculative)**

Map audio amplitude/frequency bands to physics parameters in real time (e.g., bass → repulsion
pulse, treble → gravity spike). Useful for creative/exploratory presentations. Requires an
audio input pipeline (platform mic API or file playback). Best kept as an opt-in native mod
with a clear capability gate. Not needed for the core product.

**Persistence-aware layout history (undo/redo for layout)**

Track a ring buffer of `PositionSnapshot` entries so that layout-destructive operations
(Commit Divergent, bulk move) can be undone. Independent of graph topology undo (which is
tracked at the intent level). Position undo is a purely spatial operation — no graph mutations.
Bounded ring buffer (e.g., 20 entries) keeps memory cost predictable.

---

### User Configuration Surface

The architecture implies the following user-facing configuration options. These should be
surfaced through `AppPreferences`, per-view settings, or per-layout UI controls. Most do not
require new data structures — they are toggles and sliders wiring into fields that already
exist in the design.

**Per-ExtraForce enable/disable toggle UI**

Each `Extra<T, DEFAULT>` has a compile-time default but should be runtime-togglable. The physics
settings panel should render one toggle per active `ExtraForce` type, labelled by the force's
display name. Toggling off suspends the force without removing it from the type tuple.
Implementation: `ExtraForce` trait adds a `fn is_enabled(&self) -> bool` method; the force's
`apply()` body checks it.

**ExtraForce parameter sliders**

Each `ExtraForce` exposes named parameters (strength, radius, decay). The physics settings
panel renders sliders for these, with min/max bounds defined by the force. Implementation:
`ExtraForce` trait adds a `fn params_ui(&mut self, ui: &mut egui::Ui)` method, called from
the settings panel.

**Per-node physics overrides persistence**

Allow users to manually override derived physics properties on individual nodes (e.g., pin a
node, set mass to zero, assign to a collision group manually). These overrides must survive
snapshot roundtrip. Add `physics_overrides: Option<NodePhysicsOverrides>` to graph node
metadata. `NodePhysicsOverrides { pinned: bool, mass_override: Option<f32>, group_override: Option<u32> }`.

**Lens-physics binding preference** *(specified — see `2026-02-25_progressive_lens_and_physics_binding_plan.md §1.2`)*

A per-user preference `lens_physics_binding: LensPhysicsBindingPreference` stored in
`AppPreferences` controlling whether applying a Lens automatically switches the physics
preset. Options: `Always`, `Ask` (default), `Never`. The binding is declared via
`LensConfig.physics_profile_id: Option<PhysicsProfileId>` (§1.1 of that plan). A
companion preference `progressive_lens_auto_switch: ProgressiveLensAutoSwitch` governs
zoom-triggered switching with an independent `Always / Ask / Never` gate (§2.4). See the
linked plan for the full chaining semantics and `AppPreferences` field names.

**Layout convergence timeout setting**

After how many frames of sub-threshold energy does the simulation auto-pause? Currently
governed by `CanvasRegistry`'s physics execution policy. Expose as a user preference
(default: 120 frames / ~2 seconds at 60fps). Also expose a "never auto-pause" toggle for
users who want continuous simulation.

**Region visibility toggle**

`PhysicsRegion` backdrops (zone shapes drawn on the canvas) should be individually hideable.
Add `visible: bool` to `PhysicsRegion`. Add a "Show physics regions" toggle to the view menu
(default: true). Regions always affect physics regardless of visibility.

**Platform-specific physics defaults (mobile detection)**

Detect the platform at startup and apply a mobile-optimized physics default from
`PhysicsProfileRegistry` (lower node count budget, no WASM layouts, static layout preferred).
Surface as an explicit preference ("Optimize for touch / low-power device") so users can
override automatic detection.

**Preset export and import**

Allow users to export a `PhysicsProfile` preset to a file (JSON/TOML) and import it. This
enables sharing community presets outside of the native mod system. The export format should
be the same serialization as `PhysicsProfileRegistry` seed entries so that imported presets
can be promoted to mod-registered presets later. Add to the physics settings panel: "Export
preset…" / "Import preset…" buttons.

**Z-source per-preset suggestion**

Each `PhysicsProfile` preset has a natural Z-source pairing (e.g., `physics:liquid` → ZSource::Zero,
`physics:solid` → ZSource::BfsDepth). Store a `suggested_z_source: Option<ZSource>` field on
`PhysicsProfile`. When the user activates a preset in a 3D view, offer to apply the suggested
Z-source as well (non-destructive suggestion, not an automatic override).

**Region persistence strategy preference**

Control where `PhysicsRegion` entries are stored: per-view (local to a `LocalSimulation`,
lost on Canonical switch), per-workspace (persisted in graph snapshot), or per-Lens
(restored when the Lens is applied). Surface as a dropdown in region creation UI:
"Region scope: View / Workspace / Lens".

---

### Cross-Plan Integration Gaps

The following items require coordination with other plans and are not fully resolved here:

- **`CanvasRegistry` authority**: This plan invents several new `CanvasRegistry` fields
  (`degree_repulsion_enabled`, `domain_clustering_enabled`, `edge_routing_enabled`, etc.).
  These must be registered in `2026-02-22_registry_layer_plan.md §Layout Domain` to avoid
  duplication. Resolve before implementing Level 2 ExtraForce forces.

- **`LensCompositor` + physics preset binding** *(resolved — see `2026-02-25_progressive_lens_and_physics_binding_plan.md §1`)*:
  `LensConfig.physics_profile_id: Option<PhysicsProfileId>` is now formally specified.
  The full `Always/Ask/Never` binding contract and runtime behavior in `LensCompositor::apply_lens`
  are defined in the linked plan.

- **`GraphViewState.dimension` placement**: `ViewDimension` is defined here but belongs to
  the `GraphViewState` struct in `2026-02-22_multi_graph_pane_plan.md`. Confirm that plan
  is updated to include the `dimension` field before implementing 3D rendering.

- **`LocalSimulation` + rapier**: `LocalSimulation` (from multi_graph_pane_plan) owns a
  shadow position set. Divergent views that also run rapier physics need a rapier `RigidBodySet`
  per `LocalSimulation`. Specify whether `LocalSimulation` owns both the shadow positions and
  the rapier world, or whether rapier is always global.

- **Snapshot format for `PhysicsRegion`**: Regions reference `GraphWorkspace.zones`. The
  snapshot format (`persistence_ops.rs`) must be extended to include `physics_regions`. Confirm
  this is tracked in the persistence plan.

- **burn semantic force data contract**: The interface between `LocalIntelligenceAgent` and
  the `DomainCluster` / semantic ExtraForce must be defined as a trait or data structure,
  not left as narrative. Coordinate with `2026-02-24_local_intelligence_research.md`.

---

### Failure Modes and Degradation Paths

These are not yet specified in the plan and should be resolved before shipping each feature:

- **WASM layout plugin load failure**: plugin file missing, invalid WASM binary, wrong
  exported function signature. Fallback: `ActiveLayout::FruchtermanReingold`. Show per-pane
  error badge. Do not panic or crash.

- **rapier body count overflow**: very large graphs (>10,000 nodes) may exceed practical
  rapier step time budget. Define a node count threshold above which rapier physics is
  automatically downgraded to the simpler `ExtraForce`-only pipeline. Surface as a warning
  in the physics settings panel.

- **burn backend unavailable**: wgpu adapter not found (old OpenGL-only driver). Automatic
  fallback to `burn-ndarray` CPU backend. Note: on the ndarray backend, embedding inference
  will be CPU-bound. Expose backend selection in `AppPreferences` ("ML backend: Auto / CPU / GPU").

- **Layout convergence failure**: a layout never converges (energy oscillates above the
  auto-pause threshold indefinitely). The auto-pause timeout setting (above) is the primary
  mitigation. Additionally, expose a "Force pause" button that overrides the energy threshold.

- **3D camera gimbal lock**: Full 3D mode with an arcball camera can hit gimbal lock near
  the poles. Use quaternion-based camera rotation (not Euler angles) from day one.

- **Snapshot deserialization with unknown `ActiveLayout` variant**: a snapshot was saved with
  a WASM layout mod that is no longer installed. Fallback to `ActiveLayout::FruchtermanReingold`.
  Log a warning with the missing layout mod name. Do not fail to load the snapshot.

---

## Progress

### 2026-02-24

- Initial research document created. Three-level extension model documented.
- Updated with full crate landscape: rapier2d, parry2d, rstar, kiddo, barnes_hut, fdg-sim,
  l-system-fractals, extism.
- WASM layout mod architecture documented: host-side adapter pattern, `ActiveLayout` enum
  dispatcher as hot-swap seam.
- Fractal and geometric layout algorithms documented: phyllotaxis/Fibonacci spiral, Penrose
  tiling, L-system fractal path, radial, timeline.
- Functional physics layer (rapier2d): regions/rules/surfaces model, Verse sync persistence.
- Mobile portability: touch input gap noted, static layout types (phyllotaxis, void) as mobile
  default recommendation.
- Module scope boundary table updated with new locations.
- 2D↔3D hotswitch architecture added: position parity contract, render backend options,
  perspective modes (2D, 2.5D, Isometric, 3D), rapier3d upgrade path, `ViewDimension` on `GraphViewState`.
- Rapier as Semantic Scene Composer added: semantic object mapping, `EdgePhysicsMode`/`EdgeVisualMode`,
  `PhysicsRegion`/`RegionRule`, Canvas Editor, GPU pipeline separation, burn-wgpu
  relationship.
- Research gaps, secondary mod candidates, and user configuration surface documented (38 items
  across 7 categories). Implementation work begins with Level 1 (naming) then Level 2 (ExtraForce).
