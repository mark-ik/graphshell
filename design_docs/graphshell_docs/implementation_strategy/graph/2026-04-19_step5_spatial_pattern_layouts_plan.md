<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Step 5 — Spatial-Pattern Layouts Plan (2026-04-19)

**Status**: Proposed (not started)
**Scope**: Design and implementation plan for the four Step-5 layouts
deferred from the egui_graphs retirement sequence: Penrose tiling,
L-system fractal paths, Semantic Embedding (precomputed), and Semantic
Edge Weight (iterative). Each lands as a new `graph_canvas::layout::*`
variant.

**Parent**: [2026-02-24_physics_engine_extensibility_plan.md §Step 5](2026-02-24_physics_engine_extensibility_plan.md).
**Umbrella**: [2026-04-03_layout_variant_follow_on_plan.md](2026-04-03_layout_variant_follow_on_plan.md).
**Retirement source**: [../../../archive_docs/checkpoint_2026-04-19/graphshell_docs/implementation_strategy/shell/2026-04-18_egui_graphs_retirement_plan.md](../../../archive_docs/checkpoint_2026-04-19/graphshell_docs/implementation_strategy/shell/2026-04-18_egui_graphs_retirement_plan.md) §8 "Still deferred" (archived 2026-04-19).

---

## 1. Framing

Every layout below has discretionary design choices (algorithm variant,
ordering policy, iteration depth, fallback behavior) where multiple answers
are reasonable and neither is universally wrong. The framing for this
plan — and all Graphshell layouts going forward — is:

> **Discretionary choices become user-configurable settings, not
> hardcoded picks. First-pass implementations may narrow defaults, but the
> plan tracks the full design space so future enrichment slots into named
> config surfaces rather than re-deriving them.**

This means each layout below has two layers:

1. **Config surface** — the enumerated set of configuration knobs the
   layout exposes to the user (via Lens, Settings, or runtime API).
2. **First-pass scope** — which knobs are implemented in the first
   landing and which are deferred as enrichment.

Deferred knobs do not vanish; they are tracked in the "Future
configurability" subsection of each layout so they can be picked up
later without re-design.

### 1.1 Layouts as pluggable mods

As a cross-cutting concern, **all layouts are candidates for the
pluggable-mod model**. Built-in layouts (FR, BH, Radial, Phyllotaxis,
Grid, plus the four added here) ship in `graph_canvas::layout` as
compile-time modules, but the registry that surfaces them to users
should accept third-party `Layout<N>` impls on the same footing. This
is a separate lane tracked in
[2026-04-19_layouts_as_pluggable_mods_plan.md](2026-04-19_layouts_as_pluggable_mods_plan.md).

The WASM guest-hosted subset of that lane is tracked separately in
[2026-04-03_wasm_layout_runtime_plan.md](2026-04-03_wasm_layout_runtime_plan.md).

---

## 2. Penrose / aperiodic tiling

### 2.1 Algorithm overview

Recursive subdivision (deflation) of golden-ratio tiles — either P2
(kite + dart) or P3 (thin + thick rhombus). After `n` deflations, the
tiling yields ~`φ^(2n)` vertices; nodes are placed on vertices.

### 2.2 Config surface

```rust
pub struct PenroseConfig {
    pub variant: PenroseVariant,
    pub subdivision_count: SubdivisionCount,
    pub assignment: NodeAssignmentStrategy,
    pub unused_vertices: UnusedVertexPolicy,
    pub center: Point2D<f32>,
    pub tile_scale: f32,
}

pub enum PenroseVariant {
    /// P2 — kite + dart. Chunkier visual texture; more distinct local
    /// motifs; better for spatial-memory recall.
    KiteDart,
    /// P3 — thin + thick rhombus. Smoother visual texture; more
    /// mathematically uniform.
    Rhombus,
}

pub enum SubdivisionCount {
    /// Deflate until `vertex_count >= node_count`. Smallest tiling that
    /// fits. Default.
    Auto,
    /// Explicit deflation depth. Useful for deterministic tiling
    /// comparisons or artistic control.
    Explicit(u8),
}

pub enum NodeAssignmentStrategy {
    /// Deterministic center-out spiral ordering. Simple, stable across
    /// graph mutations; nodes near in insert order land near in space.
    PreservedOrder,
    /// Group nodes onto nearby vertices by graphlet membership.
    GraphletAware,
    /// Group nodes onto nearby vertices by registrable domain.
    DomainClustered,
    /// Group nodes onto nearby vertices by UDC classification path.
    UdcClustered,
    /// Group nodes onto nearby vertices by edge connectivity.
    EdgeAffinity,
}

pub enum UnusedVertexPolicy {
    /// Unused tiling vertices remain empty; the tiling's full extent is
    /// visible as gaps around placed nodes. Reveals structure; may look
    /// sparse on small graphs.
    LeaveEmpty,
    /// Clip the layout's reported bounds to the convex hull of used
    /// vertices. Tighter visual result but hides the tiling periphery.
    ClipToHull,
    /// Hide the visible tiling geometry entirely on platforms where the
    /// backdrop cost isn't worth the aesthetic (e.g. mobile, low-power
    /// rendering paths).
    HideTiling,
}
```

### 2.3 First-pass scope

- **Shipped first**: all four `PenroseVariant` / `SubdivisionCount` /
  `UnusedVertexPolicy` values. `NodeAssignmentStrategy::PreservedOrder`
  only; the four clustered strategies deferred to enrichment wave 2.
- **Reasonable defaults**: `variant = Rhombus`, `subdivision_count =
  Auto`, `assignment = PreservedOrder`, `unused_vertices = LeaveEmpty`.

### 2.4 Future configurability

- GraphletAware / DomainClustered / UdcClustered / EdgeAffinity assignment
  strategies. Each is ~100 LOC of ordering logic over existing Graphshell
  metadata pipelines; gated on the cross-cutting work in
  [2026-04-03_semantic_clustering_follow_on_plan.md](2026-04-03_semantic_clustering_follow_on_plan.md).
- Explicit `subdivision_count` tuning UI (slider + "regenerate" button)
  for users who want visual control without reaching into config JSON.
- Per-variant sub-knobs (P2 dart/kite ratio bias, P3 thin/thick ratio
  bias) for advanced users.

### 2.5 Approximate size

- First pass: ~200 LOC + ~6 tests.
- Full surface (with clustered assignment): +~400 LOC across four strategy
  modules.

---

## 3. L-system fractal paths

### 3.1 Algorithm overview

A Lindenmayer system defines a string grammar with an axiom and
production rules; iteration produces a long symbol sequence, which a
turtle walks to produce a path. Nodes land on successive turtle-step
positions.

### 3.2 Config surface

```rust
pub struct LSystemConfig {
    pub grammar: LSystemGrammar,
    pub iteration_depth: IterationDepth,
    pub origin: Point2D<f32>,
    pub size: f32,
    /// Rotation applied to the entire path in radians.
    pub rotation: f32,
    pub reverse_order: bool,
}

pub enum LSystemGrammar {
    /// Hilbert space-filling curve. Locality-preserving (near in index
    /// → near in space). Practical default for navigation-oriented
    /// layouts; scales to very large graphs.
    Hilbert,
    /// Koch snowflake path. Fractal boundary; decorative. Good for
    /// small graphs where visual character matters.
    Koch,
    /// Dragon curve. Self-avoiding spiral-fold. Visually striking;
    /// moderate path length.
    Dragon,
    /// Reserved for future user-authored grammars. Current first-pass
    /// treats this variant as `Hilbert` with a diagnostic; see
    /// `2026-04-03_wasm_layout_runtime_plan.md` for the runtime-grammar
    /// lane.
    Custom(CustomGrammarHandle),
}

pub enum IterationDepth {
    /// Smallest depth `n` such that the grammar yields ≥ node_count
    /// positions. Default.
    Auto,
    /// Explicit depth; useful when the user wants a specific fractal
    /// level regardless of node count.
    Explicit(u8),
}

pub struct CustomGrammarHandle(/* opaque ID into a grammar registry */);
```

### 3.3 First-pass scope

- **Shipped first**: `Hilbert`, `Koch`, `Dragon` — all three as named
  options the user can pick between. `IterationDepth::Auto` + `Explicit`.
  `rotation` + `reverse_order` basic transforms.
- **`Custom(...)` reserved but unresolved**: the first-pass behavior is
  "fall back to Hilbert with a diagnostic." Implementing custom grammars
  requires the pluggable-mod registry (§1.1) or WASM guest path.
- **Reasonable defaults**: `grammar = Hilbert`, `iteration_depth = Auto`,
  `rotation = 0.0`, `reverse_order = false`.

### 3.4 Future configurability

- Custom grammars via registry (ties to the pluggable-mod lane).
- Per-grammar sub-knobs (e.g., Koch snowflake side count, Dragon curve
  fold angle variant) — these are parameter slots in the grammar itself.
- Turtle state customization: initial heading, line-segment-to-node
  ratio for sparse placement, branch handling (`[` / `]`) for tree-like
  layouts.
- Additional grammars: Sierpinski, Gosper curve, Penrose rhombus string,
  Peano curve. Each is ~30 LOC of grammar definition once the engine
  supports parametric grammars.

### 3.5 Approximate size

- First pass: ~150 LOC (shared turtle interpreter + 3 grammar tables) +
  ~6 tests.
- Full surface (custom grammars + parametric turtles): +~500 LOC across
  the grammar-registry and runtime-guest lanes.

---

## 4. Semantic Embedding (precomputed)

### 4.1 Algorithm overview

Not a projection algorithm itself — this layout consumes precomputed 2D
embeddings (from UMAP / t-SNE / PCA / a real ML pipeline) and returns
delta-to-target positions. The projection work happens outside
graph-canvas, in the host's ML pipeline (Graphshell's `burn` integration
or a sidecar).

### 4.2 Config surface

```rust
pub struct SemanticEmbeddingConfig {
    pub origin: Point2D<f32>,
    /// Scale factor applied to the host-provided embedding coordinates.
    /// Hosts typically pass coords in `[-1, 1]` or `[0, 1]`; this scales
    /// to world units.
    pub scale: f32,
    pub rotation: f32,
    pub fallback: EmbeddingFallback,
}

pub enum EmbeddingFallback {
    /// Nodes without a precomputed embedding stay at their current
    /// position. Default.
    LeaveInPlace,
    /// Place unembedded nodes at `origin`.
    CollapseToOrigin,
    /// Place unembedded nodes at a deterministic position derived from
    /// their ID hash, radially around the embedded cluster.
    RingOutside,
}
```

The embedding itself comes in via `LayoutExtras`:

```rust
pub struct LayoutExtras<N> {
    // ...existing fields...
    /// Host-provided 2D coordinates per node (from UMAP / t-SNE / etc.).
    /// Coordinate space is arbitrary; the layout scales it via
    /// `SemanticEmbeddingConfig.scale`.
    pub embedding_by_node: HashMap<N, Point2D<f32>>,
}
```

### 4.3 First-pass scope

- All three `EmbeddingFallback` variants.
- Reasonable defaults: `origin = (0, 0)`, `scale = 400.0`, `rotation = 0.0`,
  `fallback = LeaveInPlace`.

### 4.4 Future configurability

- Per-axis scaling (x vs y) when the embedding is known to be
  anisotropic (e.g., t-SNE on dense clusters).
- Post-projection centering / normalization strategies (mean-center,
  max-extent-normalize, unit-sphere-fit).
- Multiple named embeddings per graph (e.g., one UMAP on titles, another
  on edge-derived features); user switches between them.

### 4.5 Approximate size

- First pass: ~80 LOC + ~4 tests.

### 4.6 Where the embedding pipeline actually lives

The layout itself is ~80 LOC; the pipeline that computes the embeddings
lives elsewhere. Graphshell's `burn` integration (or a Python/WASM
sidecar) produces the `HashMap<NodeKey, Point2D<f32>>` that flows into
`LayoutExtras`. Pipeline design is out of scope for this plan; see
[../verse_docs/research/2026-02-24_local_intelligence_research.md](../../../../verse_docs/research/2026-02-24_local_intelligence_research.md)
for the upstream ML design.

---

## 5. Semantic Edge Weight (iterative)

### 5.1 Algorithm overview

Force-directed projection where edge-attraction strength is driven by
semantic similarity rather than topology. Distinct from
`SemanticEmbedding` — this layout does the projection work itself,
inside graph-canvas, using only pairwise similarity as input. Quality is
below real UMAP/t-SNE, but no ML pipeline is required.

Algorithmically: similar to FR, but the edge-attraction coefficient for
each pair `(a, b)` is `similarity(a, b) × base_strength` instead of
uniform. Pairs below a similarity floor contribute no attraction.

### 5.2 Why a distinct layout (not just "FR with edge weights")

- Semantically named so the user sees what they're getting.
- Semantic Edge Weight is explicitly *not* real UMAP; naming this
  distinctly prevents users from assuming they're getting
  research-grade embedding quality.
- The layout operates on pairwise similarity, which does not imply an
  edge — it can pull nodes together that are not graph-adjacent. That's
  a different contract from FR+edge-weights.

### 5.3 Config surface

```rust
pub struct SemanticEdgeWeightConfig {
    pub similarity_floor: f32,
    pub attraction_strength: f32,
    pub repulsion_strength: f32,
    pub damping: f32,
    pub dt: f32,
    pub max_step: f32,
    /// If `true`, graph edges are *also* treated as attraction sources
    /// with uniform weight. If `false`, only similarity drives attraction
    /// (pure semantic projection; topology ignored).
    pub include_graph_edges: bool,
    /// Optional center gravity; same shape as FR's.
    pub gravity_strength: f32,
}
```

Reads `LayoutExtras.semantic_similarity` for pairwise scores (already
shipped with Step 3 extras).

### 5.4 First-pass scope

- All fields above implemented.
- Reasonable defaults: `similarity_floor = 0.2`, `attraction_strength =
  1.0`, `repulsion_strength = 1.0`, `damping = 0.3`, `dt = 0.05`,
  `max_step = 10.0`, `include_graph_edges = false`, `gravity_strength =
  0.2`.

### 5.5 Future configurability

- Anisotropic repulsion (asymmetric force based on per-node tag).
- Multiple named similarity matrices (e.g., topic similarity vs temporal
  similarity) with user-selectable blending weights.
- Convergence detection + auto-pause analogous to FR.

### 5.6 Approximate size

- First pass: ~300 LOC (FR-like loop + similarity-weighted attraction) +
  ~6 tests.

---

## 6. Cross-cutting: layouts as pluggable mods

All four layouts above — and FR, Barnes-Hut, Radial, Phyllotaxis, Grid,
and the extras — are candidates for the pluggable-mod framing. The built-in
set ships as compile-time modules in `graph_canvas::layout`, but the
user-visible registry should accept third-party `Layout<N>` impls on the
same footing (up to trait-object limits).

**Tracked separately** in
[2026-04-19_layouts_as_pluggable_mods_plan.md](2026-04-19_layouts_as_pluggable_mods_plan.md)
with:

- Registry API (how third-party layouts register themselves)
- Discovery + admission rules (what makes a layout admissible)
- Trait-object storage (`Box<dyn Layout<N>>` vs enum dispatch tradeoffs)
- Native vs WASM guest distinction
- UI surfacing: how the user picks a layout from the combined pool of
  built-in and third-party providers
- Per-layout lifecycle hooks (`on_activate`, `on_deactivate`) for
  resource management

---

## 7. Open questions

- **Config persistence**: each layout's config is per-view, right? Or
  can a config be "saved as a named preset" across views? The latter
  matches `PhysicsProfile` — if so, layouts should have a similar
  `LayoutProfile` registry. Probably yes; needs explicit confirmation.
- **Layout picker UX**: do users pick layouts from a flat list
  (all 15+) or hierarchically by family (force-based / analytic /
  spatial / semantic)? Depends on how much user-discoverability
  matters.
- **UDC / domain / graphlet inputs**: the advanced Penrose assignment
  strategies all depend on the same Graphshell-specific metadata
  pipelines (UDC classification, registrable domain extraction,
  graphlet derivation). Confirm the `LayoutExtras` slot shape for
  these is generic enough to carry them through graph-canvas without
  Graphshell-specific types leaking in.

---

## 8. Implementation sequence

1. Start with **SemanticEmbedding** — smallest LOC, no design risk,
   validates the `embedding_by_node` slot shape for others.
2. Then **L-system** — self-contained math; Hilbert/Koch/Dragon ship
   together.
3. Then **SemanticEdgeWeight** — builds on the shipped FR base.
4. Finally **Penrose** — most LOC and the first-pass narrows the
   assignment strategies; leave the advanced strategies for the
   enrichment wave tied to semantic-clustering follow-ons.

Each is independent and can stall without blocking the others.

---

## 9. Progress

### 2026-04-19

- Plan created after Step-5 design pass with Mark. Configurability-first
  framing adopted (see memory file
  `feedback_configurability_over_opinionated_defaults.md`). Four layouts
  specified with full config surfaces. Layouts-as-pluggable-mods carved
  out as its own cross-cutting lane.

### 2026-04-19 — Step 5 first-pass implementation landed

All four layouts implemented and tested in `crates/graph-canvas/src/layout/`:

- **SemanticEmbedding** ([semantic_embedding.rs](../../../../crates/graph-canvas/src/layout/semantic_embedding.rs)) —
  consumes `LayoutExtras::embedding_by_node`. All three
  `EmbeddingFallback` variants shipped. 3 tests.
- **SemanticEdgeWeight** ([semantic_embedding.rs](../../../../crates/graph-canvas/src/layout/semantic_embedding.rs)) —
  iterative similarity-driven projection; all config fields shipped
  (similarity_floor, attraction/repulsion strengths, damping, dt,
  include_graph_edges, edge_strength, gravity_strength, is_running).
  4 tests.
- **L-system** ([l_system.rs](../../../../crates/graph-canvas/src/layout/l_system.rs)) —
  Hilbert + Koch + Dragon grammars shipped as user-choosable defaults.
  `Custom(CustomGrammarHandle)` variant reserved, falls back to
  Hilbert per plan. Auto + Explicit `IterationDepth`. 6 tests.
- **Penrose** ([penrose.rs](../../../../crates/graph-canvas/src/layout/penrose.rs)) —
  Full config surface: `Rhombus` (P3) and `KiteDart` (P2) both
  implemented via Robinson-triangle subdivision; all four
  `UnusedVertexPolicy` variants defined; five `NodeAssignmentStrategy`
  variants defined with four falling back to `PreservedOrder` in
  first-pass as planned; `SubdivisionCount::{Auto, Explicit}`. 6 tests.

Additionally the `axis_value_by_node` slot (added to `LayoutExtras` in
this session) unblocked two axial layouts from the layout-variant plan
that share the same slot:

- **Timeline** ([axial.rs](../../../../crates/graph-canvas/src/layout/axial.rs)) —
  numeric x-axis placement with all three fallback modes. 2 tests.
- **Kanban** ([axial.rs](../../../../crates/graph-canvas/src/layout/axial.rs)) —
  categorical column bucketing with configurable order + `include_other_column`
  toggle. 3 tests.

`embedding_by_node` and `axis_value_by_node` + `AxisValue` enum added
to `LayoutExtras` in `crates/graph-canvas/src/layout/mod.rs`.

**Test results:**

- `cargo test -p graph-canvas --lib`: **180 passed / 0 failed**.
- `cargo test -p graph-canvas --features simulate --lib`:
  **204 passed / 0 failed**.
- `cargo test --lib -- --test-threads=1` (graphshell workspace):
  2143 passed / 1 flaky failure (`navigator_specialty_corridor_uses_selected_pair_and_tree_layout`,
  the known flake tracked in
  [../../../archive_docs/checkpoint_2026-04-20/graphshell_docs/implementation_strategy/testing/2026-04-19_flaky_test_hygiene_plan.md](../../../archive_docs/checkpoint_2026-04-20/graphshell_docs/implementation_strategy/testing/2026-04-19_flaky_test_hygiene_plan.md) (archived 2026-04-20; all flakes fixed);
  passes in isolation; unrelated to Step 5).

**Counts since retirement baseline:**

- graph-canvas layouts implemented: FR, BarnesHut, DegreeRepulsion,
  DomainClustering, SemanticClustering, HubPull, FrameAffinity, Grid,
  Radial, Phyllotaxis, RapierLayout (simulate feature),
  **SemanticEmbedding**, **SemanticEdgeWeight**, **L-system**,
  **Penrose**, **Timeline**, **Kanban** — **seventeen total**.

**First-pass-enrichment items tracked** (per the configurability
framing):

- Penrose advanced assignment strategies (GraphletAware / DomainClustered /
  UdcClustered / EdgeAffinity) — gated on
  [2026-04-03_semantic_clustering_follow_on_plan.md](2026-04-03_semantic_clustering_follow_on_plan.md).
- L-system `Custom(handle)` grammar — gated on the
  [2026-04-19_layouts_as_pluggable_mods_plan.md](2026-04-19_layouts_as_pluggable_mods_plan.md)
  registry and/or WASM guest runtime.
- Additional L-system grammars (Sierpinski, Gosper, Peano) — trivial
  once custom-grammar registry exists; each is ~30 LOC.
- Per-grammar sub-knobs and parametric turtles.
- Timeline / Kanban UI surfacing for choosing the axis input field.

### 2026-04-19 — Retroactive configurability pass for earlier layouts

Applied the configurability-first principle retroactively to the eleven
layouts that landed before the Step-5 design pass. Shipped as four
focused batches:

**Batch A — "free wins"** (hardcoded knobs now config):

- `BarnesHutConfig { theta, min_cell_size }` — `theta = 0.5` and
  `MIN_CELL_SIZE = 1.0` constants now user-tunable (accuracy vs speed).
- `PhyllotaxisConfig` gained `angle_radians: f32` (with `angles::GOLDEN
  / QUARTER_TURN / THIRD_TURN / HALF_TURN` constants) and
  `radius_curve: PhyllotaxisRadiusCurve::{SquareRoot, Linear,
  Quadratic, Logarithmic}`. Golden-angle-Fibonacci is the default;
  other combinations produce three-arm spirals, cross-grids, and
  exotic packings.
- `GridConfig` gained `columns: GridColumns::{Auto, Explicit(u32),
  AspectRatio(f32)}` and `traversal: GridTraversal::{RowMajor,
  ColumnMajor, Snaking, Spiral}`. Spiral traversal is particularly
  useful for priority-ordered node lists.
- `RadialConfig` gained `angular_policy: RadialAngularPolicy::{Uniform,
  DegreeWeighted, HashSorted}`, `rotation_offset: f32`, and
  `unreachable_policy: RadialUnreachablePolicy::{OuterRing, Center,
  LeaveInPlace}`.

**Batch C — extras weighting menus**:

New shared `graph-canvas::layout::curves` module with reusable enums:
`ProximityFalloff::{Linear, Smoothstep, Exponential, Cosine}`,
`DegreeWeighting::{Logarithmic, Linear, SquareRoot, Polynomial(p)}`,
`SimilarityCurve::{Linear, Quadratic, Cubic, Threshold(floor)}`,
`Falloff::{Inverse, InverseSquare, Linear, Exponential(rate)}` (used
by Batch B too).

Wired into:

- `DegreeRepulsionConfig` gained `proximity_falloff`,
  `degree_weighting`, `min_degree`.
- `HubPullConfig` gained `proximity_falloff`, `hub_degree_weighting`.
- `SemanticClusteringConfig` gained `similarity_curve`.
- `DomainClusteringConfig<N>` (now generic) gained `target_policy:
  TargetPolicy::{Centroid, Medoid, FirstMember, NamedAnchor}`,
  `min_members: u32`, `anchor_by_group: HashMap<String, N>`.
- `FrameAffinityConfig` gained `target_policy`, `min_members`; the
  `NamedAnchor` variant uses each `FrameRegion::anchor` as the target.

**Batch D — RapierLayout** edge-joint and body-kind variants:

- `EdgeJoint::{Spring { rest, stiffness, damping }, Rope { max_length,
  stiffness }, Distance { length }, None }` replaces the hardcoded
  spring-per-edge.
- `BodyKindPolicy::{PinnedStatic, PinnedKinematic, AllKinematic}`
  replaces the hardcoded pinned-or-dynamic body kind.

**Batch B — force-shape menu** (biggest blast radius, shipped last):

- `ForceDirectedState` gained `repulsion_falloff: Falloff` (default
  `Inverse`, classic FR) and `gravity_falloff: Falloff` (default
  `Linear`, classic pull-harder-when-far). Applies to both
  `ForceDirected` and `BarnesHut`.
- `SemanticEdgeWeightConfig` gained the same two knobs with the same
  defaults.

**Test results across all four batches:**

- `cargo test -p graph-canvas --lib`: **186/186** (baseline 180 + 5
  curves module tests + 1 repulsion-falloff test).
- `cargo test -p graph-canvas --features simulate --lib`: **212/212**
  (+ 2 rapier edge-joint / kinematic tests and curves tests).
- `cargo test --lib -- --test-threads=1`: **2144/2144** (graphshell
  end-to-end pass with all retroactive config changes; `..Default::default()`
  struct-update spread keeps the existing call sites backward-compatible).

**User-visible total**: every one of the seventeen shipped layouts now
exposes its discretionary choices as typed config knobs rather than
hardcoded constants. The configurability-first principle is uniform
across the portfolio.
