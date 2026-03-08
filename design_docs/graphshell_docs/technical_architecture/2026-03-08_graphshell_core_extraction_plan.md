<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# `graphshell-core` Extraction Plan

**Date**: 2026-03-08
**Status**: Design / Planning
**Scope**: Extract the graph domain model, intent/reducer system, and headless physics engine into
a WASM-clean crate (`graphshell-core`) that compiles to `wasm32-unknown-unknown` with zero errors
and has no knowledge of egui, wgpu, or Servo.

**Related docs**:

- [`canvas/petgraph_algorithm_utilization_spec.md`](../implementation_strategy/canvas/petgraph_algorithm_utilization_spec.md) — petgraph algorithm surface used in graph/physics pre-passes
- [`canvas/2026-02-24_physics_engine_extensibility_plan.md`](../implementation_strategy/canvas/2026-02-24_physics_engine_extensibility_plan.md) — current physics extensibility architecture
- [`canvas/2026-02-23_udc_semantic_tagging_plan.md`](../implementation_strategy/canvas/2026-02-23_udc_semantic_tagging_plan.md) — UDC semantic tagging (partially in-core)
- [`2026-02-18_universal_node_content_model.md`](2026-02-18_universal_node_content_model.md) — node identity / `Address` enum vision
- [`aspect_render/2026-02-20_embedder_decomposition_plan.md`](../implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md) — completed Stage 4 decomposition (prerequisite context)

---

## 1. Motivation

Graphshell's graph domain logic (topology, identity, intents, reducers) is currently entangled with
the host application crate. This produces three concrete problems:

1. **No headless testing**: testing graph state changes requires a running egui context.
2. **No Verse server-side hosting**: a server-side Verse node that maintains a shared graph cannot
   use the same reducer without pulling in egui/Servo as dead weight.
3. **No WASM deployment path**: if Graphshell ever runs in a browser tab, the graph core needs to
   be available on `wasm32-unknown-unknown` without the host's platform dependencies.

The WASM compilation constraint is the mechanical enforcement mechanism. If `graphshell-core`
compiles to `wasm32-unknown-unknown` with zero errors, it is definitionally free of platform
dependencies. This is better than any code review.

---

## 2. Crate Boundary: What Goes In, What Stays Out

### 2.1 In `graphshell-core`

| Component | Current location | Notes |
| --- | --- | --- |
| `Graph` (petgraph-backed) | `model/graph/mod.rs` | Includes all petgraph algorithm accessors (§4) |
| `NodeKey` / `EdgeKey` UUID identity | `model/graph/mod.rs` | UUID is WASM-clean (`uuid` crate has wasm32 support) |
| `Node`, `EdgePayload` data types | `model/graph/mod.rs` | Pure data; no render state |
| `GraphWorkspace` | `graph_app.rs` | State container; no egui types (see §2.3) |
| `GraphIntent` enum | intent system | All variants; `apply_intents()` reducer |
| `GraphSemanticEvent` | event boundary | The clean boundary between core and host |
| `Address` enum | aspirational (UNC model) | Pure data; WASM-clean including `PathBuf` |
| `HistoryEntry` | aspirational (UNC model) | Pure data |
| `semantic_tags` / `semantic_index_dirty` | currently `GraphBrowserApp` | Moves to core after UDC stabilizes (§6.2) |
| `TagNode` / `UntagNode` intent variants | intent system | Already graph mutations |
| URL normalization utilities | scattered | Shared by NIP-84 `r` tag and node deduplication |
| Petgraph algorithm accessors | specified in petgraph spec §4.1 | `hop_distances_from`, `shortest_path`, `orphan_node_keys`, etc. |
| **Headless physics engine** | `graph/physics.rs` (new) | Pure math; see §3 |
| Topology pre-passes (MST seed, component loci, `LayoutHint`) | new | Uses petgraph; feeds physics engine |

### 2.2 Not In `graphshell-core`

| Component | Reason |
| --- | --- |
| `egui::Pos2`, `egui::Vec2`, any egui type | Would break WASM compilation gate |
| `GraphPhysicsState` / `GraphPhysicsLayout` (egui_graphs types) | egui_graphs is not WASM-clean |
| `KnowledgeRegistry`, `reconcile_semantics` | Application-layer; depends on `nucleo`, UDC dataset |
| `ContentRenderer`, `ProtocolResolver` traits | Reference egui, filesystem, OS |
| iroh, libp2p, Nostr transports | Network I/O; host crate concern |
| Servo / webview lifecycle | Host crate only |
| `PhysicsProfile.apply_to_state()` | Depends on egui_graphs state types |
| Tile compositor, workbench layout | Render pipeline concern |

### 2.3 Position Type: No `egui::Pos2` in Core

The petgraph spec's `ComponentGravityParams` currently uses `egui::Pos2` for locus positions.
This must change. `graphshell-core` owns a position newtype:

```rust
/// A 2D position in graph layout space. WASM-clean; no egui dependency.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphPos2 {
    pub x: f32,
    pub y: f32,
}

impl From<GraphPos2> for egui::Pos2 {
    fn from(p: GraphPos2) -> Self { egui::Pos2::new(p.x, p.y) }
}

impl From<egui::Pos2> for GraphPos2 {
    fn from(p: egui::Pos2) -> Self { GraphPos2 { x: p.x, y: p.y } }
}
```

The conversions live in the host crate (which knows about egui), not in core. Core uses `GraphPos2`
everywhere positions appear.

---

## 3. Headless Physics Engine

### 3.1 Design Principles

- Pure math: no allocator beyond `std`, no platform I/O, no callbacks into render code.
- WASM-clean: compiles to `wasm32-unknown-unknown` with zero errors.
- No petgraph dependency: topology analysis is a pre-pass in the host/core graph layer;
  the physics engine receives results (node positions, force params) and outputs positions.
- Topology-aware via `LayoutHint`: the engine selects initial conditions and force parameters
  based on a hint from the topology classifier pre-pass.

### 3.2 Core Interface

```rust
/// Opaque node description for the physics engine.
/// The engine does not know about NodeKey, URLs, or graph topology.
pub struct PhysicsNode {
    pub pos: GraphPos2,
    pub mass: f32,           // default 1.0; heavier = less displaced by repulsion
    pub pinned: bool,        // if true, repulsion/attraction are applied but displacement is not
}

/// Force configuration parameters for one physics step.
pub struct PhysicsParams {
    pub k: f32,              // optimal distance constant (Fruchterman-Reingold)
    pub temperature: f32,    // current annealing temperature
    pub gravity: GravityMode,
    pub semantic_forces: Vec<SemanticForce>,   // UDC attraction pairs
    pub extra_forces: Vec<ExtraForceSpec>,
}

pub enum GravityMode {
    /// Single center well at canvas origin.
    Center { strength: f32 },
    /// Per-component gravity loci (from ComponentGravityParams pre-pass).
    ComponentLoci { loci: Vec<(usize, GraphPos2)>, strength: f32 },
    /// No gravity.
    None,
}

/// UDC-derived semantic attraction between two node indices.
pub struct SemanticForce {
    pub a: usize,
    pub b: usize,
    pub similarity: f32,    // [0.0, 1.0] from KnowledgeRegistry prefix distance
}

/// A layout hint from the topology classifier pre-pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutHint {
    /// General graph — standard Fruchterman-Reingold.
    ForceGeneral,
    /// Tree or DAG — radial or BFS-level initial positions.
    ForceTree,
    /// Linear chain (bus) — horizontal spine, branch repulsion.
    ForceBus,
    /// Cycle / ring — circular initial arrangement, then spring-relax.
    ForceRing,
    /// Dense clique — charge repulsion dominant, minimize crossings.
    ForceClique,
    /// Caller provides explicit initial positions; engine only relaxes.
    ExplicitSeed,
}

/// Step the physics simulation by `dt` seconds.
/// `nodes` positions are updated in-place.
/// `edges` are (a_index, b_index) pairs into `nodes`.
/// Returns the maximum displacement of any node (convergence signal).
pub fn step(
    nodes: &mut [PhysicsNode],
    edges: &[(usize, usize)],
    params: &PhysicsParams,
    dt: f32,
) -> f32;

/// Compute topology-aware initial positions for a cold-start graph.
/// Call once before the first `step()` when nodes have no committed positions.
/// Uses `LayoutHint` to select the initial arrangement algorithm.
pub fn cold_start_positions(
    node_count: usize,
    edges: &[(usize, usize)],
    hint: LayoutHint,
    area: f32,   // canvas area hint for scaling
) -> Vec<GraphPos2>;
```

### 3.3 Algorithm Selection by `LayoutHint`

| `LayoutHint` | `cold_start_positions` algorithm | `step` force profile |
| --- | --- | --- |
| `ForceGeneral` | Random scatter in area circle | Standard FR: repulsion + attraction + gravity |
| `ForceTree` | BFS-level layout (root = highest-degree node, children at angular intervals per depth) | FR with reduced repulsion, stronger attraction along edges |
| `ForceBus` | Nodes on horizontal line, branches above/below | Spine-constraint force keeping chain nodes horizontal |
| `ForceRing` | Nodes equally spaced on a circle | Circular constraint force preserving ring spacing |
| `ForceClique` | Random scatter in small area (nodes are tightly bound) | Charge repulsion dominant; attraction suppressed within clique |
| `ExplicitSeed` | No-op (caller has set positions) | Standard FR |

### 3.4 Topology Classifier Pre-Pass

The classifier runs in the host/core graph layer (has petgraph access) before handing work to the
physics engine. It returns a `LayoutHint`:

```rust
/// Classify the graph topology for physics layout hint selection.
/// Runs petgraph algorithms; lives in graphshell-core alongside Graph.
pub fn classify_topology(graph: &Graph) -> LayoutHint {
    let node_count = graph.node_count();
    if node_count == 0 { return LayoutHint::ForceGeneral; }

    // Check for linear chain (bus): all nodes have degree ≤ 2, graph is connected
    let max_degree = graph.inner.node_indices()
        .map(|n| graph.inner.edges(n).count() + graph.inner.edges_directed(n, Direction::Incoming).count())
        .max().unwrap_or(0);

    // Ring: cycle detected, all nodes degree == 2
    // Bus: no cycle, all nodes degree ≤ 2
    // Tree/DAG: toposort succeeds, no cycles
    // Clique: edge count ≈ n*(n-1)/2
    // General: everything else

    match petgraph::algo::toposort(&graph.inner, None) {
        Ok(_) if max_degree <= 2 => LayoutHint::ForceBus,
        Ok(_) => LayoutHint::ForceTree,
        Err(_) => {
            let e = graph.inner.edge_count();
            let n = node_count;
            let clique_threshold = n * (n - 1) / 2;
            if e >= clique_threshold * 3 / 4 { LayoutHint::ForceClique }
            else if max_degree == 2 { LayoutHint::ForceRing }
            else { LayoutHint::ForceGeneral }
        }
    }
}
```

### 3.5 MST Warm Seed Integration

The MST warm seed (petgraph spec §2.6) feeds `LayoutHint::ExplicitSeed`:

1. Host calls `graph.min_spanning_tree_positions()` — petgraph MST + radial tree layout → `Vec<GraphPos2>`.
2. Host writes positions to `PhysicsNode::pos` for each node.
3. Host calls `step()` with `LayoutHint::ExplicitSeed` — engine relaxes from the MST seed, not random scatter.

Guard: only apply when all nodes have `pos == GraphPos2::zero()` (first load or imported graph
with no committed positions). If any node has a non-zero position, skip the seed entirely.

### 3.6 Semantic Forces Integration

The `KnowledgeRegistry` (host crate) computes per-pair UDC similarity after each
`reconcile_semantics` call and produces a `Vec<SemanticForce>`. This is passed into `PhysicsParams`
each frame. The physics engine applies:

```
F_semantic(A, B) = similarity * (pos_B - pos_A) * k_semantic
```

The engine does not know UDC codes, prefix distances, or registry logic.
Similarity scores arrive as pre-computed `f32` values. This is the correct split:
ontology in the host, force application in the engine.

---

## 4. Petgraph Algorithm Surface in Core

All accessors specified in `petgraph_algorithm_utilization_spec.md §4.1` live on `Graph` in
`graphshell-core`. Summary:

```rust
impl Graph {
    pub fn neighbors_undirected(&self, key: NodeKey) -> impl Iterator<Item = NodeKey> + '_;
    pub fn hop_distances_from(&self, source: NodeKey) -> HashMap<NodeKey, usize>;
    pub fn orphan_node_keys(&self) -> Vec<NodeKey>;
    pub fn shortest_path(&self, from: NodeKey, to: NodeKey) -> Option<Vec<NodeKey>>;
    pub fn is_reachable(&self, from: NodeKey, to: NodeKey) -> bool;
    pub fn weakly_connected_components(&self) -> Vec<Vec<NodeKey>>;
    pub fn strongly_connected_components(&self) -> Vec<Vec<NodeKey>>;
    pub fn condensation_dag(&self) -> petgraph::Graph<Vec<NodeKey>, ()>;
    pub fn toposort(&self) -> Result<Vec<NodeKey>, NodeKey>;   // Err = cycle node
    pub fn min_spanning_tree_positions(&self) -> Vec<(NodeKey, GraphPos2)>;
    pub fn classify_topology(&self) -> LayoutHint;
}
```

`hop_distance_cache` and `component_membership_cache` live on `GraphWorkspace` (also in core),
with invalidation on structural change exactly as specified in the petgraph spec §4.2–§4.3 — but
using `GraphPos2` instead of `egui::Pos2` for locus positions.

---

## 5. UDC / Semantic Tagging Boundary

### 5.1 What moves into core now

- `TagNode` / `UntagNode` as `GraphIntent` variants — already graph mutations, already in core scope.
- `semantic_tags: HashMap<NodeKey, HashSet<String>>` — graph state; moves onto `GraphWorkspace`.
- `semantic_index_dirty: bool` — graph state; moves onto `GraphWorkspace`.

### 5.2 What stays in the host crate (permanently)

- `KnowledgeRegistry` — provider/router/parser, `nucleo` fuzzy search, UDC dataset.
- `reconcile_semantics` — application-layer reconciliation loop; reads `semantic_index_dirty`,
  calls registry, writes `semantic_index`, prunes stale keys.
- `SemanticIndex` (the parsed index) — produced by the registry, not owned by core.

### 5.3 Migration note

`semantic_tags` is currently held on `GraphBrowserApp`, not on `GraphWorkspace`, as a deliberate
temporary decision during registry stabilization (UDC plan §1). When `graphshell-core` is
extracted, `semantic_tags` and `semantic_index_dirty` move to `GraphWorkspace`. The host crate
holds a reference to the workspace and passes it to `reconcile_semantics` as before.

---

## 6. `Address` Enum and Node Identity

The UNC model (`2026-02-18_universal_node_content_model.md`) specifies:

- `NodeId` (UUID) as stable identity, decoupled from address.
- `Address` enum: `Http(Url)`, `File(PathBuf)`, `Onion`, `Ipfs(Cid)`, `Gemini`, `Custom`.
- `address_history: Vec<HistoryEntry>` — append-only navigation log per node.

All of these are pure data types, WASM-clean, and belong in `graphshell-core`.

**Critical prerequisite**: The UNC model §8 notes that migrating from URL-based node identity
to UUID-based identity is "the highest-friction migration in the whole vision." This migration must
happen before `graphshell-core` is extracted. Extracting the crate before the identity migration
means doing both refactors simultaneously on the same types — high merge complexity.

See §8 (Sequencing) for the ordered dependency chain.

---

## 7. WASM Compilation Gate

The WASM constraint is enforced by CI, not convention:

```toml
# graphshell-core/Cargo.toml
[package]
name = "graphshell-core"
edition = "2021"

[dependencies]
petgraph = { version = "0.8", features = ["serde-1"] }
uuid = { version = "1", features = ["v4", "serde", "js"] }   # "js" for wasm32 RNG
serde = { version = "1", features = ["derive"] }
url = "2"           # Url type; WASM-clean

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
# host-only deps if any (none expected)
```

CI addition:

```yaml
- name: WASM compilation gate
  run: cargo build -p graphshell-core --target wasm32-unknown-unknown
```

This job must pass on every PR that touches `graphshell-core`. Any egui/wgpu/Servo import causes
a compile error. The gate is self-enforcing.

**`uuid` RNG on WASM**: `uuid` v1 with the `"js"` feature uses `getrandom` with the `"js"` backend
on WASM, which calls `crypto.getRandomValues()`. This is correct for browser WASM. For
`wasm32-unknown-unknown` in non-browser environments (Verse node WASM), a deterministic seed may
be needed — tracked as an open question.

---

## 8. Sequencing and Prerequisites

The following dependency chain must be respected. Each step is a prerequisite for the next.

### Step 0 — Petgraph algorithm replacements (active: petgraph spec PRs 1–5)

Finish the petgraph spec PR sequence (hop-distance cache, neighbors_undirected, component
membership cache, ComponentGravityLoci). This cleans the `Graph` API boundary before extraction.

**Status**: PR-1 through PR-5 are the active work from the petgraph spec.

### Step 1 — Introduce `GraphPos2` position type in the host crate

Add `GraphPos2` as a newtype in the current codebase. Replace `egui::Pos2` in
`ComponentGravityParams` and any locus-position fields with `GraphPos2`. Add `From<>` conversions
at the render boundary. This is a preparatory refactor with no behavior change.

**Effort**: Small. **Risk**: Low.

### Step 2 — UUID node identity migration

Migrate `Node` identity from URL-based to UUID-based (`NodeId: Uuid`). Extend the fjall log with
`NavigateNode` replacing `UpdateNodeUrl`. This is the UNC model §8 prerequisite.

**Effort**: Large. **Risk**: High (persistence migration). **Gate**: Must not start until Step 0
is complete (avoids simultaneous refactors on the same types).

### Step 3 — `Address` enum introduction

Add `Address` enum to the codebase (in the host crate initially). Wire `Node::address: Address`
replacing the current URL field. Renderer selection by `ContentRenderer::can_render()` hook is
not required at this step — the enum just needs to exist and be persisted.

**Effort**: Medium. **Gate**: After Step 2.

### Step 4 — Extract `graphshell-core` crate

Create the crate. Move:
- `model/graph/mod.rs` → `graphshell-core/src/graph/mod.rs`
- `GraphIntent` + `apply_intents()` → `graphshell-core/src/intent.rs`
- `GraphSemanticEvent` → `graphshell-core/src/event.rs`
- `GraphPos2` → `graphshell-core/src/pos.rs`
- `Address`, `HistoryEntry` → `graphshell-core/src/address.rs`
- `GraphWorkspace` (with `semantic_tags`, `hop_distance_cache`, `component_membership_cache`) →
  `graphshell-core/src/workspace.rs`
- Petgraph algorithm accessors → `graphshell-core/src/graph/algorithms.rs`
- `classify_topology()` → `graphshell-core/src/graph/topology.rs`

**Effort**: Large. **Gate**: After Steps 1–3.

### Step 5 — Headless physics engine in core

Add `graphshell-core/src/physics/` module:
- `physics/step.rs` — `step()` and `cold_start_positions()`
- `physics/hint.rs` — `LayoutHint` enum
- `physics/node.rs` — `PhysicsNode`, `PhysicsParams`, `GravityMode`, `SemanticForce`

Wire host crate: replace `egui_graphs` FR step with a call to `core::physics::step()` for the
pure-math engine path. `egui_graphs` remains as the rendering adapter (it draws nodes/edges);
only the physics computation moves.

**Effort**: Medium. **Gate**: After Step 4.

### Step 6 — Semantic forces wiring

After UDC Phase 2 (`SemanticGravity` force), replace the O(N²) pair loop with the pre-computed
`Vec<SemanticForce>` from `KnowledgeRegistry`, passed into the physics engine via `PhysicsParams`.

**Effort**: Small (integration; the engine interface already supports it after Step 5). **Gate**:
After UDC Phase 2 lands.

---

## 9. Acceptance Criteria

### For `graphshell-core` (Steps 4–5)

1. `cargo build -p graphshell-core --target wasm32-unknown-unknown` passes with zero errors.
2. No import of `egui`, `wgpu`, `servo`, `iroh`, `libp2p`, or `nostr` in `graphshell-core`.
3. All `Graph` algorithm accessors from petgraph spec §4.1 are present and tested.
4. `apply_intents()` is in core; all `GraphIntent` variants are handled.
5. `GraphSemanticEvent` is the only boundary type crossing from core to host.
6. `step()` is deterministic given the same inputs (no thread-local RNG in physics step).
7. `classify_topology()` correctly identifies tree, ring, bus, clique, and general topologies
   against a suite of synthetic graphs.
8. `graphshell` (host crate) still compiles and all existing tests pass.

### For WASM gate (Step 7 — CI enforcement)

9. A CI job `cargo build -p graphshell-core --target wasm32-unknown-unknown` is present and
   required to pass on every PR touching `graphshell-core`.

---

## 10. Open Questions

1. **`wasm32-unknown-unknown` vs. `wasm32-wasi`**: The gate target should be
   `wasm32-unknown-unknown` (most restrictive — no OS, no file I/O, no threads by default).
   `wasm32-wasi` allows more. Confirm with the Verse server-side WASM deployment target.

2. **`petgraph` on WASM**: petgraph has no platform dependencies. Confirm no feature flags
   are needed for `wasm32-unknown-unknown` (expected: none required).

3. **`uuid` RNG on non-browser WASM**: The `"js"` feature requires a browser JS runtime.
   For Verse server-side WASM (non-browser), a different RNG strategy is needed. Options:
   - `uuid` with deterministic seed passed in from the host.
   - `"getrandom"` with WASI backend when targeting `wasm32-wasi`.
   Track per deployment target.

4. **`GraphWorkspace` vs. `GraphBrowserApp`**: Currently `GraphBrowserApp` holds both graph
   state and application state (UI state, webview maps, etc.). The extraction must not create
   a circular dependency. The correct split: `GraphWorkspace` in core owns pure graph state;
   `GraphBrowserApp` in the host owns everything else and holds a `GraphWorkspace`.

5. **`url` crate WASM compatibility**: `url` 2.x is WASM-clean. Confirm that `Url` parsing
   does not invoke any platform-specific path normalization on WASM targets.

---

*This document is the authoritative design reference for `graphshell-core` extraction.
Update it as steps complete or prerequisites change.*
