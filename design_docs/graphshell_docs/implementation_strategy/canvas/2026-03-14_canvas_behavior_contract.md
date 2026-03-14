# Canvas Behavior Contract

**Date**: 2026-03-14
**Status**: Design — Pre-Implementation
**Purpose**: Define canonical scenario outcomes and computable behavioral
invariants for the graph canvas physics and layout system. This contract gives
the physics tuning process a reference point that is checkable rather than
subjective, and gives CI a regression surface beyond the portfolio readability
metrics already defined in `layout_algorithm_portfolio_spec.md §4`.

**Related**:

- `layout_behaviors_and_physics_spec.md` — physics micro-behavior and force injection contracts
- `layout_algorithm_portfolio_spec.md` — portfolio selection, readability metrics, diagnostics
- `graph_node_edge_interaction_spec.md` — interaction model authority
- `2026-03-14_graph_relation_families.md` — family physics weights (`FamilyPhysicsPolicy`)
- `2026-02-25_progressive_lens_and_physics_binding_plan.md` — lens/physics binding

---

## 1. Why This Contract Exists

`layout_algorithm_portfolio_spec.md §4` defines readability metrics evaluated
after layout execution (`crossing_density`, `label_overlap_ratio`,
`edge_len_cv`, etc.). Those metrics answer "is the layout readable?" They do not
answer:

- Does the simulation converge to a stable state at all?
- Does the Liquid/Gas/Solid distinction produce perceptibly different outcomes?
- Do disconnected components stay separated or collapse onto each other?
- Does adding one node destabilize the entire graph?
- Does the containment lens actually cluster nodes by URL hierarchy?

These questions require **scenario-level behavioral assertions** — synthetic
graphs with known topology, run through the simulation for a bounded step count,
with computable properties checked at the end. That is what this contract
defines.

This contract is not a replacement for the portfolio quality metrics. It is the
layer below them: the physics foundation that the readability metrics assume is
already correct.

---

## 2. Computable Properties

The following properties are referenced by scenario assertions in §3. All are
computable on a `Graph` snapshot plus a `Vec<(NodeKey, Vec2)>` position array
without rendering.

### 2.1 Kinetic Energy (convergence proxy)

```
KE = sum over all nodes of: 0.5 * velocity.length_squared()
```

A simulation has **converged** when `KE < convergence_threshold` for
`convergence_window` consecutive steps. Default thresholds:

| Parameter | Default value |
| --- | --- |
| `convergence_threshold` | `0.5` (px²/step²) |
| `convergence_window` | `10` steps |

### 2.2 Node Overlap Count

Number of pairs of nodes whose bounding boxes intersect. Node bounding box is
`position ± (node_radius + overlap_margin)` where `overlap_margin = 4.0 px`.

```
overlap_count = count of (i, j) pairs where distance(pos_i, pos_j) < 2 * (node_radius + overlap_margin)
```

Target: `overlap_count == 0` at convergence for all Small graphs.

### 2.3 Component Separation

For a graph with K connected components, the **component separation ratio** is:

```
min_inter_component_distance / mean_intra_component_diameter
```

Where `min_inter_component_distance` is the minimum distance between any two
nodes in different components, and `mean_intra_component_diameter` is the mean
of each component's node-position bounding-box diagonal.

Target: `component_separation_ratio >= 1.5` — components are clearly distinct,
not interleaved.

### 2.4 Edge Length Coefficient of Variation (CV)

Already defined in `layout_algorithm_portfolio_spec.md §4` as `edge_len_cv`.
Repeated here for scenario reference:

```
edge_len_cv = std_dev(edge_lengths) / mean(edge_lengths)
```

Target per scenario: `edge_len_cv <= 0.65` (portfolio default), tightened to
`<= 0.45` for Small graphs at convergence.

### 2.5 Perturbation Return Distance

After convergence, displace one node by a fixed offset vector `(50.0, 0.0)` px.
Run the simulation for an additional 200 steps. Measure the distance between
the node's final position and its pre-perturbation position.

```
perturbation_return_distance = distance(final_position, pre_perturbation_position)
```

Target: `perturbation_return_distance <= 20.0 px` for a Small connected graph.
This checks that the equilibrium is stable under perturbation — not that the
node returns to exactly the same pixel, but that it returns to roughly the same
region.

### 2.6 Family Force Dominance

For a scenario with multiple edge families active, **family force dominance**
checks whether the family with the highest `FamilyPhysicsPolicy` weight is
actually the dominant contributor to mean node displacement per step.

This is a directional check: it does not produce a scalar threshold but instead
asserts a ranking: `dominant_family_displacement > other_family_displacement`
for all other families.

### 2.7 Convergence Step Count

Number of simulation steps required to reach convergence (§2.1). Used to check
that physics does not become pathologically slow.

Target: `convergence_step_count <= max_steps` per scenario (see §3 for
per-scenario values).

---

## 3. Canonical Scenarios

Each scenario specifies:
- A synthetic graph topology (node count, edge structure, edge families)
- A physics profile / preset
- Step budget
- Assertions to check at step budget

Scenarios are independent and repeatable. They use a fixed physics seed for
determinism.

### Scenario P1 — Small connected ring, Solid preset

**Graph**: 6 nodes in a ring (A→B→C→D→E→F→A), all `Hyperlink` edges (Semantic
family).
**Physics preset**: Solid (`PhysicsProfile::Solid`).
**Step budget**: 800 steps.

**Assertions**:
- `overlap_count == 0`
- `KE < convergence_threshold` for 10 consecutive steps before step 800 (converged)
- `edge_len_cv <= 0.45`
- `perturbation_return_distance <= 20.0 px`

**Purpose**: Baseline. If this fails, the physics engine is fundamentally broken
for the simplest connected case. Solid should produce a stable, tight ring.

---

### Scenario P2 — Small connected ring, Gas preset

**Graph**: Same 6-node ring as P1.
**Physics preset**: Gas (`PhysicsProfile::Gas`).
**Step budget**: 1200 steps.

**Assertions**:
- `overlap_count == 0`
- Converged before step 1200
- `edge_len_cv <= 0.65` (Gas is looser; full portfolio threshold acceptable)
- Mean inter-node distance in Gas > mean inter-node distance in P1 (Solid).
  Specifically: `mean_edge_length_gas >= mean_edge_length_solid * 1.3`

**Purpose**: Verifies that Gas and Solid produce perceptibly different spatial
outcomes on the same graph. This is the "Liquid/Gas/Solid are distinct" check.
If `mean_edge_length_gas` is within 10% of Solid, the presets are not
meaningfully differentiated.

---

### Scenario P3 — Small connected ring, Liquid preset

**Graph**: Same 6-node ring as P1.
**Physics preset**: Liquid (`PhysicsProfile::Liquid`).
**Step budget**: 1000 steps.

**Assertions**:
- `overlap_count == 0`
- Converged before step 1000
- `mean_edge_length_liquid` is between `mean_edge_length_solid` and
  `mean_edge_length_gas` from P1/P2:
  `mean_edge_length_solid <= mean_edge_length_liquid <= mean_edge_length_gas`

**Purpose**: Verifies that Liquid is genuinely intermediate. This is the
ordering invariant: Solid tightest → Liquid middle → Gas loosest.

---

### Scenario P4 — Two disconnected clusters

**Graph**: Two 4-node complete graphs (A-B-C-D fully connected, E-F-G-H fully
connected), no edges between clusters. All `Hyperlink` edges.
**Physics preset**: Solid.
**Step budget**: 800 steps.

**Assertions**:
- `overlap_count == 0`
- Converged before step 800
- `component_separation_ratio >= 1.5`
- Both clusters have converged positions: `KE_per_component < convergence_threshold`
  for each component independently

**Purpose**: Verifies disconnected components do not collapse onto each other.
This is a common pathology when global gravity is too strong.

---

### Scenario P5 — Star topology (hub and spokes)

**Graph**: 1 hub node connected to 8 leaf nodes via `Hyperlink` edges. Leaves
have no edges to each other.
**Physics preset**: Solid.
**Step budget**: 800 steps.

**Assertions**:
- `overlap_count == 0`
- Converged before step 800
- All 8 leaf nodes are at approximately equal distance from hub:
  `std_dev(leaf_hub_distances) / mean(leaf_hub_distances) <= 0.15`
- No leaf node is closer to hub than `node_radius * 2.5`

**Purpose**: Verifies that degree-1 nodes (spokes) distribute evenly around a
high-degree hub. Uneven spoke distribution indicates repulsion forces are not
balancing attraction correctly.

---

### Scenario P6 — Incremental node addition stability

**Graph**: Start with P1's 6-node ring at convergence (use P1 final positions).
Add 1 new node connected to one existing node via `Hyperlink`.
**Physics preset**: Solid.
**Step budget**: 400 additional steps (post-add).

**Assertions**:
- Reheat triggered: physics running after `AddNode` (per
  `layout_behaviors_and_physics_spec.md §2.1`)
- New node placed near its semantic parent (within `3 * node_radius`)
- `overlap_count == 0` at step 400
- Displacement of original 6 nodes:
  `max_original_node_displacement <= 40.0 px` — original nodes are not
  dramatically destabilized by one addition
- Converged before step 400

**Purpose**: Verifies incremental stability. Adding one node should not cause
the entire graph to reorganize. This tests the mental-map preservation
invariant from `layout_algorithm_portfolio_spec.md §4` (`incremental_displacement_ratio`).

---

### Scenario P7 — Containment family clustering

**Graph**: 6 nodes divided into two URL path groups:
- Nodes A, B, C: `url-path` containment edges pointing to a synthetic parent
  node P1 (URL `https://example.com/docs`)
- Nodes D, E, F: `url-path` containment edges pointing to a synthetic parent
  node P2 (URL `https://example.com/blog`)
- No semantic or traversal edges between any nodes.

**Lens**: A containment-active lens with `containment_weight = 1.0`,
`semantic_weight = 0.0`.
**Physics preset**: Solid with `FamilyPhysicsPolicy` active.
**Step budget**: 1000 steps.

**Assertions**:
- `overlap_count == 0`
- Converged before step 1000
- `component_separation_ratio >= 1.5` between the two groups
  (nodes A/B/C cluster together; D/E/F cluster together)
- With `containment_weight = 0.0` (lens off): `component_separation_ratio < 1.0`
  — nodes drift without containment force

**Purpose**: Verifies that containment-family physics actually clusters nodes
by hierarchy when the lens is active, and that nodes do not cluster when the
weight is zero. This is the core test for `FamilyPhysicsPolicy` effectiveness.

---

### Scenario P8 — Semantic vs traversal family dominance

**Graph**: 4 nodes: A and B connected by `UserGrouped` (Semantic) edge;
B and C connected by `TraversalDerived` (Traversal) edge; C and D connected
by `TraversalDerived` edge.

**Lens configuration A**: `semantic_weight = 1.0`, `traversal_weight = 0.0`
**Lens configuration B**: `semantic_weight = 0.0`, `traversal_weight = 1.0`
**Physics preset**: Solid.
**Step budget**: 800 steps each.

**Assertions** (A vs B comparison):
- In configuration A: distance(A, B) < distance(B, C) — semantic edge pulls
  A and B together more than traversal edge pulls B and C
- In configuration B: distance(B, C) < distance(A, B) — traversal edge now
  dominant
- Family force dominance assertion (§2.6) holds in both configurations

**Purpose**: Verifies that `FamilyPhysicsPolicy` weights actually invert which
family drives layout. If A and B distances are the same in both configurations,
the weight system is not working.

---

## 4. Preset Ordering Invariant

This invariant must hold across any graph topology (not just P1–P3):

```
For any graph G with >= 2 connected nodes and Solid/Liquid/Gas presets:
  mean_edge_length(Solid) <= mean_edge_length(Liquid) <= mean_edge_length(Gas)
```

And the difference must be perceptible:

```
mean_edge_length(Gas) >= mean_edge_length(Solid) * 1.25
```

If this invariant fails for any of the canonical scenarios, the presets are not
meaningfully distinct and must be retuned before physics work proceeds.

---

## 5. Convergence Budget Reference

| Scenario | Graph size | Preset | Max steps |
| --- | --- | --- | --- |
| P1 | 6 nodes, ring | Solid | 800 |
| P2 | 6 nodes, ring | Gas | 1200 |
| P3 | 6 nodes, ring | Liquid | 1000 |
| P4 | 8 nodes, 2 clusters | Solid | 800 |
| P5 | 9 nodes, star | Solid | 800 |
| P6 | 6+1 nodes, incremental | Solid | 400 (post-add) |
| P7 | 8 nodes, containment | Solid + containment lens | 1000 |
| P8 | 4 nodes, family dominance | Solid + family weights | 800 each |

If a scenario does not converge within its step budget, that is a **convergence
failure** — a distinct failure mode from a readability threshold violation. It
means the physics engine is in a pathological state (oscillation, divergence, or
excessive damping preventing settlement).

---

## 6. Diagnostics Integration

Scenario runs should emit existing diagnostics channels where applicable:

- `ux:layout_quality` after each scenario's final step
- `ux:layout_quality_violation` if portfolio thresholds are exceeded
- A new channel `canvas:physics_scenario_result` (severity: `Info`) emitted
  once per scenario run with fields:
  `scenario_id`, `step_count`, `converged`, `overlap_count`,
  `edge_len_cv`, `component_separation_ratio`, `preset`

This channel gives the diagnostics export a machine-readable record of physics
behavior for comparison across builds.

---

## 7. What This Contract Does Not Cover

- **Rendering correctness** — pixel-accurate node or edge appearance; that is a
  render spec concern
- **Interaction gesture correctness** — whether drag, lasso, or click behave
  correctly; that is `graph_node_edge_interaction_spec.md`
- **Large graph performance** — the scenarios here are Small band (`<= 100`
  nodes); Medium and Large band physics behavior is governed by
  `layout_algorithm_portfolio_spec.md` thresholds and the performance contract
  spec
- **Lens visual rendering** — what containment or traversal lenses look like on
  the canvas; that is an edge visual encoding concern (separate spec needed)
- **Per-edge visual encoding** — how different edge families are rendered
  visually; that is the missing edge visual spec referenced in the relation
  families discussion

---

## 8. Acceptance Criteria

- [ ] All 8 scenarios pass their assertions deterministically (fixed seed)
- [ ] Preset ordering invariant (§4) holds for P1, P2, P3
- [ ] Gas mean edge length is >= 1.25× Solid for all Small connected graphs
- [ ] P6 max original node displacement <= 40.0 px
- [ ] P7 containment clustering: `component_separation_ratio >= 1.5` with lens
  active, `< 1.0` with lens off
- [ ] P8 family dominance assertion holds in both weight configurations
- [ ] `canvas:physics_scenario_result` channel emitted for each scenario run
- [ ] Scenarios runnable in CI without a render target (headless, position-only)
