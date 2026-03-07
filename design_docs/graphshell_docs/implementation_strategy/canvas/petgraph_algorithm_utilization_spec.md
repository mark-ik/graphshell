<!-- petgraph_algorithm_utilization_spec.md -->
# Petgraph Algorithm Utilization Spec

**Status:** Planning
**Date:** 2026-03-07
**Scope:** All algorithmic uses of `petgraph` in graphshell — existing replacements and new capabilities.

---

## 0. Background and Motivation

Graphshell's backing store is a `petgraph::stable_graph::StableGraph<Node, EdgePayload, Directed>`.
Petgraph ships a rich algorithm library (`petgraph::algo`, `petgraph::visit`) that we are not yet
using. Instead the codebase contains three hand-rolled BFS implementations and several features that
are algorithmic in nature but not yet built. This spec enumerates:

1. **Direct replacements** — hand-rolled code that should be deleted and replaced with petgraph calls.
2. **New capabilities** — features described in roadmap docs that petgraph algorithms directly unlock.
3. **Performance implications** — which sites are hot-path and require memoization.

The dependency is already declared in `Cargo.toml`:

```toml
petgraph = "0.8.3"   # features: serde-1
```

No new dependencies are required for anything in this spec.

---

## 1. Inventory of Hand-Rolled Graph Algorithms

### 1.1 `connected_hop_distances_for_context`

**File:** `shell/desktop/ui/toolbar/toolbar_omnibar.rs:443`
**What it does:** Full undirected BFS from a context node, returns `HashMap<NodeKey, usize>` mapping
every reachable node to its hop count from the context.
**Call sites:**

- `connected_nodes_matches_for_query` — called once per omnibar render with non-empty query text.
- `omnibar_match_signifier` — called once per visible match entry per render frame, meaning if 10
  matches are displayed, this BFS runs 10 times with the same context node.

**Current code:**

```rust
fn connected_hop_distances_for_context(
    graph_app: &GraphBrowserApp,
    context: NodeKey,
) -> HashMap<NodeKey, usize> {
    let mut distances = HashMap::new();
    let mut queue = VecDeque::new();
    distances.insert(context, 0);
    queue.push_back(context);
    while let Some(current) = queue.pop_front() {
        let Some(current_hop) = distances.get(&current).copied() else { continue; };
        for neighbor in graph_app.domain_graph().out_neighbors(current)
            .chain(graph_app.domain_graph().in_neighbors(current))
        {
            if distances.contains_key(&neighbor) { continue; }
            distances.insert(neighbor, current_hop + 1);
            queue.push_back(neighbor);
        }
    }
    distances
}
```

**Petgraph replacement:** `petgraph::algo::dijkstra` with unit edge weights over an undirected view
of the inner `StableGraph`. Dijkstra with unit weights is BFS, so the result is identical.

```rust
use petgraph::algo::dijkstra;
use petgraph::visit::EdgeRef;

fn connected_hop_distances_for_context(
    graph: &Graph,
    context: NodeKey,
) -> HashMap<NodeKey, usize> {
    dijkstra(
        &petgraph::visit::AsUndirected(&graph.inner),
        context,
        None,            // no early-exit target
        |_| 1_usize,    // unit weight
    )
}
```

`petgraph::visit::AsUndirected` wraps any directed graph and presents it as undirected to the
algorithm, matching the current behavior of chaining `out_neighbors` and `in_neighbors`.

**Memoization requirement (CRITICAL):** This function is called O(matches) times per render frame.
The result must be cached and invalidated only when the primary selection changes or the graph
structure changes. The cache should live on `GraphWorkspace` (or be passed down from the omnibar
parent render call). Signature after memoization:

```rust
/// Cached in GraphWorkspace; recomputed when primary selection or graph structure changes.
pub hop_distance_cache: Option<(NodeKey, HashMap<NodeKey, usize>)>,
```

Invalidation sites: `apply_graph_delta_and_sync` (graph structure change) and wherever primary
selection is committed.

---

### 1.2 `connected_candidates_with_depth`

**File:** `shell/desktop/ui/gui_frame.rs:314`
**What it does:** Bounded BFS from a source node up to depth 2, collecting `(NodeKey, depth)` pairs.
Used for the "Open Connected" layout command.

**Current code:** 25-line `VecDeque`+`HashSet` BFS with a depth cap of 2.

**Petgraph replacement:** `petgraph::visit::Bfs` with a manual depth tracker, or simply two rounds of
neighbor expansion (depth 1 = direct neighbors, depth 2 = neighbors of neighbors minus already
visited). The `Bfs` struct from `petgraph::visit` maintains the visited set internally:

```rust
use petgraph::visit::{Bfs, Walker};

// Depth-1 case (PendingConnectedOpenScope::Neighbors):
// just out_neighbors + in_neighbors of source — no BFS needed, keep as-is.

// Depth-2 case (PendingConnectedOpenScope::Connected):
let mut bfs = Bfs::new(&petgraph::visit::AsUndirected(&graph.inner), source);
let mut out = Vec::new();
bfs.next(&petgraph::visit::AsUndirected(&graph.inner)); // consume source itself
let mut depth1 = HashSet::new();
// gather depth-1
while let Some(n) = bfs.next(&petgraph::visit::AsUndirected(&graph.inner)) {
    depth1.insert(n);
    out.push((n, 1_u8));
    // BFS will naturally expand these next; stop at depth 2 via visited check
}
```

Because `Bfs` doesn't expose per-node depth natively, the cleanest approach for the depth-2 cap is
to run two explicit neighbor expansions rather than a full BFS walk. This avoids needing a wrapper
and is only two loops. The depth cap (2) is small and fixed, so full `Bfs` is over-engineering here;
explicit two-round expansion is cleaner and should be preferred.

**Verdict:** Replace the `VecDeque` BFS loop with two explicit neighbor expansion rounds. Use
`petgraph::visit::AsUndirected` to avoid duplicating the `out_neighbors + in_neighbors` pattern.

---

### 1.3 `connected_frame_import_nodes`

**File:** `shell/desktop/ui/gui_frame.rs:237`
**What it does:** Depth-1 undirected neighbor expansion for a set of seed nodes. Returns all seeds
plus their immediate neighbors.

**Current code:** Straightforward set union over `out_neighbors` + `in_neighbors`.

**Petgraph replacement:** No BFS needed. Use `petgraph::visit::AsUndirected` to make the neighbor
query uniform, but the logic is already correct. This is a style cleanup, not a correctness fix.
The only real gain is replacing the explicit `out_neighbors(...).chain(in_neighbors(...))` pair with
`graph.inner.neighbors_undirected(seed)` once the inner graph is accessible.

**Verdict:** Minor cleanup — expose `Graph::neighbors_undirected(key)` accessor on the domain graph
and call it here.

---

## 2. New Capabilities Unlocked by Petgraph Algorithms

### 2.1 Connected Components (`petgraph::algo::connected_components`, `kosaraju_scc`)

**Function:** `petgraph::algo::connected_components(&graph)` — returns the number of weakly
connected components in a directed graph. `petgraph::algo::kosaraju_scc(&graph)` — returns the
strongly connected components as `Vec<Vec<NodeKey>>` (each SCC is a group of nodes that are
mutually reachable in the directed sense).

**Graphshell applications:**

#### 2.1.1 Workspace Auto-Suggest

When a user opens a new workspace/frame, a sensible default set of panes is "all nodes in the
connected component containing the currently active node." Implemented with:

```rust
// Find which component the active node belongs to
let components = petgraph::algo::kosaraju_scc(&graph.inner);
let active_component = components.iter()
    .find(|comp| comp.contains(&active_node))
    .cloned()
    .unwrap_or_default();
```

The resulting `Vec<NodeKey>` is the natural candidate set for "Open Connected" at maximum depth.

#### 2.1.2 Orphan Detection and Visual Callout

Nodes with no edges (degree 0) are trivially orphaned. Nodes in components of size 1 are graph
orphans. This is useful for:

- Surfacing a "these nodes are disconnected" callout in the canvas overlay.
- Dimming or visually distinguishing isolated nodes from the primary component.
- Driving an "orphan cleanup" suggestion in the UX (link or remove).

**Implementation site:** Add `Graph::orphan_node_keys() -> Vec<NodeKey>` that returns nodes whose
total degree (in + out) is 0. For richer isolation detection, use `connected_components` on an
undirected view to find components of size 1.

#### 2.1.3 Graph Health Diagnostics

`connected_components` count is a meaningful graph health metric — a highly fragmented graph (many
small components) suggests the user's browsing is siloed. Surface this count in the diagnostics
panel or session stats overlay.

---

### 2.2 Condensation (`petgraph::algo::condensation`)

**Function:** `condensation(graph, make_acyclic)` — collapses each SCC to a single node, producing
a DAG of "super-nodes."

**Graphshell application: Browsing Loop Detection**

HTTP browsing sessions often contain loops — the user navigated A→B→C→A. These loops show up as
SCCs in the traversal-derived edge subgraph. The condensation DAG reveals the high-level flow of a
browsing session stripped of loops.

**Uses:**

- **Visual:** Render condensed super-nodes at a zoom-out level (atlas view) as cluster representatives.
  Planned in `canvas/2026-03-05_hybrid_graph_view_overview_atlas_plan.md`.
- **History:** In the history timeline subsystem (`subsystem_history/history_timeline_and_temporal_navigation_spec.md`),
  condensation provides a natural "canonical path" through a session by following the condensation
  DAG topologically.
- **Cleanup suggestion:** SCCs with only `TraversalDerived` edges and no `Hyperlink` edges are
  pure browsing loops; the UI can offer to collapse them to a single node.

**Implementation note:** Condensation is O(V+E) via Kosaraju; for graphshell's expected graph sizes
(hundreds to low thousands of nodes) this runs in microseconds and can be computed on demand.

---

### 2.3 Topological Sort (`petgraph::algo::toposort`)

**Function:** `toposort(&graph, None)` — returns nodes in topological order if the graph is a DAG,
or `Err(Cycle { node_id })` if it contains a cycle.

**Graphshell applications:**

#### 2.3.1 Session Replay Ordering

The WAL replay (`services/persistence`) must add nodes before edges. Within a batch of `AddNode`
deltas, ordering matters only when edges create dependencies. `toposort` provides the correct replay
order for a batch of nodes+edges: add nodes in topological order so that `AddEdge` calls always find
both endpoints present.

Currently the WAL is ordered by insertion time, which is correct for sequential sessions but may
not be for reconstructed or merged snapshots. `toposort` on the pending replay batch provides a
safe, correct ordering independent of time.

#### 2.3.2 Import Ordering for Filesystem Ingest

In `viewer/2026-03-02_filesystem_ingest_graph_mapping_plan.md`, files are mapped to nodes and
directory containment creates edges. Directory hierarchies are DAGs. `toposort` gives the correct
node creation order for ingest: create leaf files before directories, or vice versa, depending on
edge direction convention.

---

### 2.4 Shortest Path Between Two Nodes (`petgraph::algo::astar` / `dijkstra`)

**Graphshell application: Primary ↔ Secondary Selection Path**

When the user has two nodes selected (primary + secondary), the shortest undirected path between
them is a natural navigation primitive:

- **Visual:** Highlight the path edges in the canvas.
- **Command:** "Open path" — open all nodes along the shortest path as panes.
- **Omnibar:** Show path length as a signifier in search results ("3 hops away").

**Implementation:**

```rust
use petgraph::algo::astar;

fn shortest_path(graph: &Graph, from: NodeKey, to: NodeKey) -> Option<Vec<NodeKey>> {
    let undirected = petgraph::visit::AsUndirected(&graph.inner);
    astar(
        &undirected,
        from,
        |n| n == to,
        |_| 1_usize,                        // unit edge cost
        |_| 0_usize,                        // zero heuristic = Dijkstra
    ).map(|(_, path)| path)
}
```

**Memoization:** Cache the result keyed on `(primary, secondary)`. Invalidate on selection change
or graph structural change.

---

### 2.5 Path Existence (`petgraph::algo::has_path_connecting`)

**Function:** `has_path_connecting(&graph, from, to, None)` — boolean reachability test.

**Graphshell applications:**

#### 2.5.1 "Open Connected" Scope Predicate

In `gui_frame.rs`, `connected_targets_for_open` currently computes all candidates via BFS and then
checks membership. `has_path_connecting` is a cleaner predicate when only the boolean is needed:

```rust
// Is 'candidate' reachable from 'source' in at most 2 hops?
// (has_path_connecting doesn't support depth limits, so explicit 2-hop expansion is still
//  better for the capped case. Use has_path_connecting only for uncapped reachability.)
```

**Better use:** When checking whether to dimm or gray out a node on the canvas (i.e., "is this node
connected to the selection at all?"), `has_path_connecting` is O(V+E) in the worst case but exits
early on success. For large sparse graphs this is faster than building the full distance map.

#### 2.5.2 Edge Add Validation

Before adding a `UserGrouped` edge between two nodes that are already connected, check if a path
already exists. If a path exists and the new edge would create a duplicate semantic connection, the
UI can warn or merge.

---

### 2.6 Minimum Spanning Tree (`petgraph::algo::min_spanning_tree`)

**Function:** Returns an iterator of `Element::Node` / `Element::Edge` forming the MST (Kruskal's
algorithm).

**Graphshell application: Physics Cold-Start Warm Seed**

When a workspace loads and nodes have no committed positions (first load, imported graph, or a graph
where all positions are zero/uniform), the FR physics engine starts from a degenerate state: all
nodes at center, maximum repulsion, maximum layout work before convergence. The result is visually
jarring — nodes scatter randomly and settle slowly.

An MST warm seed produces a topologically grounded initial arrangement before the first physics tick:

1. Extract a `f32` edge weight per edge: `w = 1.0 / (1.0 + total_navigations as f32)`, making
   heavily-traversed edges "short" in the MST so frequently co-visited nodes start close together.
2. Compute the MST with `min_spanning_tree` (Kruskal's, O(E log E)).
3. Lay out the MST tree using a simple radial or BFS-level layout (root = highest-degree node,
   children placed at equal angular intervals per depth level).
4. Write resulting positions to `node.committed_position` for all nodes in the MST. Orphan nodes
   (not in MST because they have no edges) are placed on an outer ring around the MST bounding box.

*Guard:* Only apply when all nodes have `committed_position == Point2D::zero()` or equivalent
sentinel. If any node has a non-zero committed position (i.e., persisted from a previous session),
skip the seed entirely — the user's spatial memory matters more than a clean initial layout.

*Scope:* This is a pre-physics pass in the workspace initialization path, not a `Layout<S>` impl
or an `ExtraForce`. It runs once, writes `committed_position`, then physics takes over normally.

*Implementation note:* `min_spanning_tree` operates on `NodeWeight`/`EdgeWeight` pairs via the
`IntoEdgeReferences` + `EdgeWeight: PartialOrd + Copy` constraint. Extract edge weights into a
temporary `HashMap<EdgeKey, ordered_float::NotNan<f32>>` (or use a wrapper) before passing to the
algorithm, then map the resulting `Element::Edge` keys back to node positions.

---

### 2.7 Per-Component Gravity Loci (`kosaraju_scc` / `connected_components`)

**The problem with single-center gravity on fragmented graphs:**

The current `CenterGravity` extra in the FR physics pipeline applies a single gravity well at
canvas center. When the graph has multiple weakly connected components — a common state when the
user has several unrelated browsing sessions loaded — all components are pulled toward the same
center. The result is that unrelated clusters collide and overlap, and the user cannot visually
distinguish separate research threads.

The single-center model is only correct when the graph is a single connected component.

**Per-component gravity loci:**

Replace the single `CenterGravity` extra with a `ComponentGravityLoci` `ExtraForce` that:

1. Computes weakly connected components each time the graph structure changes (not every frame —
   cached, invalidated by `apply_graph_delta_and_sync`).
2. Assigns each component a stable locus position. Initial locus positions are spread on a grid or
   ring to ensure components start spatially separated.
3. Applies per-node attraction toward the component's locus rather than canvas center.
4. As a component's locus converges toward its natural stable position, the locus itself can be
   updated to track the component's actual centroid (lerped, not snapped, to avoid oscillation).

```rust
// graph/forces/component_gravity.rs

pub struct ComponentGravityLoci;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentGravityParams {
    /// Map from component representative (lowest NodeKey in component) to locus position.
    pub loci: HashMap<NodeKey, egui::Pos2>,
    /// Map from NodeKey to its component representative.
    pub membership: HashMap<NodeKey, NodeKey>,
    /// Gravity strength per component (same value for all; could be made per-component).
    pub strength: f32,
}

impl ExtraForce for ComponentGravityLoci {
    type Params = ComponentGravityParams;
    fn apply<...>(params: &Self::Params, g, indices, disp, area, k) {
        for (i, idx) in indices.iter().enumerate() {
            let rep = params.membership.get(idx).copied().unwrap_or(*idx);
            let locus = params.loci.get(&rep).copied().unwrap_or(egui::Pos2::ZERO);
            let pos = g.node(*idx).location();
            let delta = locus - pos;
            disp[i] += delta.to_vec2() * params.strength * k;
        }
    }
}
```

**Recomputation site** — `Graph::weakly_connected_components()` is O(V+E) via Kosaraju on an
undirected view. Called once after each `apply_graph_delta_and_sync` that changes graph structure.
The resulting `ComponentGravityParams` is written into `GraphPhysicsState` before the next frame.

**Locus placement for new components:**

When a new component appears (node added with no edges, or an isolated cluster is loaded), its
initial locus must be placed so it does not overlap existing components. A simple strategy:
place new loci on an outward spiral from the canvas origin, stepping by `2 * max_radius_of_existing_components`.

**Relationship to existing `CenterGravity`:**

`CenterGravity` should be **disabled** (Extra flag `false`) when `ComponentGravityLoci` is enabled.
Running both simultaneously would pull single-component graphs correctly but double-attract
multi-component graphs toward center, undoing the separation benefit.

The `PhysicsProfile` gains a flag `per_component_gravity: bool`. When true, `CenterGravity` is
disabled and `ComponentGravityLoci` is enabled in `apply_to_state()`. The default for new profiles
should be `per_component_gravity: true` — it degrades gracefully to single-center behavior when the
graph has exactly one component (the locus is then just canvas center).

**Priority:** P2 — this is a correctness fix for a real layout defect, not a polish feature. Any
workspace with multiple unconnected browsing sessions currently has overlapping clusters.

---

### 2.8 Hierarchical and Timeline Layouts as Level 3 Candidates

The physics extensibility plan (`2026-02-24_physics_engine_extensibility_plan.md`) identifies
`LayoutHierarchical` (egui_graphs Sugiyama-style) and a future `TimelineLayout` as `ActiveLayout`
enum variants. Petgraph provides the preprocessing those layouts need.

#### DAG check and topological order for `LayoutHierarchical`

Before switching a graph pane to `LayoutHierarchical`, the layout engine must verify the graph is
a DAG (or extract a DAG subgraph). `petgraph::algo::toposort` returns `Err(Cycle { node_id })` if
the graph contains a cycle. The canonical pre-switch check:

```rust
match petgraph::algo::toposort(&graph.inner, None) {
    Ok(order) => {
        // Safe to apply hierarchical layout.
        // `order` is the topological node sequence — pass to LayoutHierarchical as the layer hint.
    }
    Err(cycle) => {
        // Graph has cycles. Options:
        // 1. Run condensation first, layout the condensation DAG, expand SCCs in place.
        // 2. Fall back to force-directed.
        // 3. Offer user "collapse loops" action first.
    }
}
```

For graphs with traversal-derived edges (browsing history), cycles are common (A→B→A). The correct
response is condensation: collapse each SCC to a super-node, apply `LayoutHierarchical` to the
condensation DAG, then expand super-nodes back into their constituent nodes arranged locally via FR.
This gives a global timeline structure with local loop clusters.

#### `TimelineLayout` preprocessing

A `TimelineLayout` (planned in the `ActiveLayout` enum) maps nodes to horizontal positions by
their logical time-ordering and vertical positions by depth in the traversal DAG. Prerequisites:

1. `toposort` on the `TraversalDerived` edge subgraph gives the temporal order of nodes.
2. Condensation collapses browsing loops into single timeline events.
3. Dominator tree (`petgraph::algo::dominators::simple_fast`) identifies which nodes are navigation
   entry points — these become the "chapter markers" in the timeline view.

None of these require a new crate dependency; all are in petgraph.

---

### 2.10 Dominators (`petgraph::algo::dominators`)

**Function:** `dominators::simple_fast(&graph, root)` — computes the dominator tree rooted at a
given node for a directed graph.

**Graphshell application: Traversal Funnel Analysis**

In traversal-derived edges (history edges), the dominator tree identifies which nodes are "entry
points" — every path from the root (session start) passes through a dominator. This identifies:

- **Navigation funnels:** A highly dominating node is a hub that the user always passes through.
- **Dead ends:** Nodes dominated only by themselves with no successors.

This maps directly to the history timeline spec
(`subsystem_history/history_timeline_and_temporal_navigation_spec.md`), which calls for "session
flow analysis." The dominator tree is the formal tool for this.

**Implementation note:** The `dominators` module requires a directed graph and a start node.
Restrict to `TraversalDerived` edge subgraph for pure browsing analysis.

---

### 2.11 Isomorphism / Subgraph Matching (`petgraph::algo::is_isomorphic_matching`)

**Graphshell application: Pattern-Based Workspace Templates**

Two workspace graphs are structurally equivalent if their node/edge topology matches. This enables:

- Detecting when a user's current browsing pattern matches a saved workspace template.
- Suggesting "you seem to be in a research pattern — apply Research template?"

This is a future/advanced feature, not near-term. Mentioned for completeness; isomorphism checking
is NP-hard in general but polynomial for bounded-degree graphs, which browsing graphs typically are.

---

## 3. Implementation Priority

| Priority | Item | Section | File | Effort | Payoff |
| -------- | ---- | ------- | ---- | ------ | ------ |
| P0 | Memoize `connected_hop_distances_for_context` cache | §1.1 | `graph_app.rs` + omnibar | Medium | Eliminates O(matches) BFS per frame |
| P0 | Replace `connected_hop_distances_for_context` with `dijkstra` | §1.1 | `toolbar_omnibar.rs` | Small | Code clarity + correctness |
| P1 | Replace `connected_candidates_with_depth` with 2-round expansion + `AsUndirected` | §1.2 | `gui_frame.rs` | Small | Code clarity |
| P1 | Add `Graph::neighbors_undirected(key)` accessor | §1.3 | `model/graph/mod.rs` | Tiny | Eliminates `out+in` duplication |
| P2 | `ComponentGravityLoci` ExtraForce + disable `CenterGravity` for multi-component graphs | §2.7 | `graph/forces/`, `graph_app.rs` | Medium | Fixes cluster overlap — correctness fix |
| P2 | `Graph::weakly_connected_components()` + component cache in `GraphWorkspace` | §2.7 | `model/graph/mod.rs` | Small | Prerequisite for §2.7 ExtraForce |
| P2 | `Graph::orphan_node_keys()` + canvas callout | §2.1 | `model/graph/mod.rs`, render | Small | Useful UX signal |
| P2 | Shortest path between primary/secondary selection | §2.4 | `graph_app.rs`, canvas render | Medium | Navigation primitive |
| P2 | `has_path_connecting` for canvas dimming predicate | §2.5 | render layer | Small | Replaces full distance map |
| P3 | MST warm seed for cold-start layout | §2.6 | workspace init path | Medium | Better first-load layout experience |
| P3 | SCC-based "Open Connected" scope | §2.1 | `gui_frame.rs` | Medium | Better default scope |
| P3 | Condensation for atlas view cluster representatives | §2.2 | atlas render | Medium | Roadmap prerequisite |
| P3 | Topological sort for WAL replay batch ordering | §2.3 | `services/persistence` | Small | Correctness for merged snapshots |
| P4 | Hierarchical layout with toposort preprocessing | §2.8 | physics engine, `ActiveLayout` | Large | Level 3 layout for DAG topologies |
| P4 | Dominator tree for history funnel analysis | §2.10 | history subsystem | Large | Roadmap: history timeline |
| P5 | Condensation DAG for history canonical path + timeline layout | §2.2, §2.8 | history subsystem | Large | Roadmap: history timeline |

---

## 4. Specific Code Contracts

### 4.1 `Graph` accessor additions required

```rust
impl Graph {
    /// Undirected neighbors of `key` (both in-edges and out-edges).
    pub fn neighbors_undirected(&self, key: NodeKey) -> impl Iterator<Item = NodeKey> + '_ {
        self.inner.neighbors_undirected(key)
    }

    /// Hop distances from `source` to all reachable nodes (undirected BFS, unit weights).
    pub fn hop_distances_from(&self, source: NodeKey) -> HashMap<NodeKey, usize> {
        petgraph::algo::dijkstra(
            &petgraph::visit::AsUndirected(&self.inner),
            source,
            None,
            |_| 1_usize,
        )
    }

    /// Nodes with no edges (in-degree + out-degree == 0).
    pub fn orphan_node_keys(&self) -> Vec<NodeKey> {
        self.inner.node_indices()
            .filter(|&n| self.inner.edges(n).next().is_none()
                      && self.inner.edges_directed(n, Direction::Incoming).next().is_none())
            .collect()
    }

    /// Shortest undirected path between two nodes (unit weights), if one exists.
    pub fn shortest_path(&self, from: NodeKey, to: NodeKey) -> Option<Vec<NodeKey>> {
        petgraph::algo::astar(
            &petgraph::visit::AsUndirected(&self.inner),
            from,
            |n| n == to,
            |_| 1_usize,
            |_| 0_usize,
        ).map(|(_, path)| path)
    }

    /// Whether `to` is reachable from `from` in the undirected graph.
    pub fn is_reachable(&self, from: NodeKey, to: NodeKey) -> bool {
        petgraph::algo::has_path_connecting(
            &petgraph::visit::AsUndirected(&self.inner),
            from,
            to,
            None,
        )
    }

    /// Weakly connected components. Returns each component as a Vec of NodeKeys.
    pub fn weakly_connected_components(&self) -> Vec<Vec<NodeKey>> {
        petgraph::algo::kosaraju_scc(&petgraph::visit::AsUndirected(&self.inner))
    }

    /// Strongly connected components (directed).
    pub fn strongly_connected_components(&self) -> Vec<Vec<NodeKey>> {
        petgraph::algo::kosaraju_scc(&self.inner)
    }
}
```

### 4.2 Hop distance cache in `GraphWorkspace`

```rust
/// Cached hop-distance map from the primary selection node.
/// Keyed on NodeKey; None means stale (recompute before next omnibar render).
pub hop_distance_cache: Option<(NodeKey, HashMap<NodeKey, usize>)>,
```

**Invalidation:**

- Set to `None` in `apply_graph_delta_and_sync` (any structural change).
- Set to `None` when `workspace.selected_nodes.primary()` changes.

**Access pattern in omnibar:**

```rust
fn get_or_compute_hop_distances<'a>(
    workspace: &'a mut GraphWorkspace,
    graph: &Graph,
) -> Option<&'a HashMap<NodeKey, usize>> {
    let primary = workspace.selected_nodes.primary()?;
    let cache = workspace.hop_distance_cache.get_or_insert_with(|| {
        (primary, graph.hop_distances_from(primary))
    });
    if cache.0 != primary {
        *cache = (primary, graph.hop_distances_from(primary));
    }
    Some(&cache.1)
}
```

This ensures BFS runs at most once per selection+graph-state combination, regardless of how many
matches the omnibar renders.

---

### 4.3 Per-component gravity loci — `GraphWorkspace` additions

```rust
/// Cached weakly-connected component membership.
/// Recomputed after each apply_graph_delta_and_sync that touches structure.
/// Key: NodeKey → component representative (lowest NodeKey in component).
pub component_membership_cache: Option<HashMap<NodeKey, NodeKey>>,

/// Stable locus positions per component representative.
/// Updated when new components appear; existing loci are preserved across recomputations
/// so in-progress physics convergence is not disrupted.
pub component_loci: HashMap<NodeKey, egui::Pos2>,
```

**Recomputation:**

```rust
fn recompute_component_membership(graph: &Graph) -> HashMap<NodeKey, NodeKey> {
    let components = petgraph::algo::kosaraju_scc(
        &petgraph::visit::AsUndirected(&graph.inner)
    );
    let mut membership = HashMap::new();
    for component in &components {
        // Representative = lowest index in component for stability across recomputes.
        let rep = *component.iter().min().expect("component is non-empty");
        for &node in component {
            membership.insert(node, rep);
        }
    }
    membership
}
```

**Locus placement for new components** (called from `apply_graph_delta_and_sync` after
`recompute_component_membership`):

```rust
fn assign_new_component_loci(
    old_membership: &HashMap<NodeKey, NodeKey>,
    new_membership: &HashMap<NodeKey, NodeKey>,
    loci: &mut HashMap<NodeKey, egui::Pos2>,
) {
    let old_reps: HashSet<NodeKey> = old_membership.values().copied().collect();
    let new_reps: HashSet<NodeKey> = new_membership.values().copied().collect();
    let added_reps = new_reps.difference(&old_reps);
    let existing_count = loci.len();
    for (i, &rep) in added_reps.enumerate() {
        // Spiral outward: radius grows with component count, angle evenly spaced.
        let idx = existing_count + i;
        let angle = idx as f32 * std::f32::consts::TAU / 6.0;  // 6 per ring
        let radius = 300.0 * (1 + idx / 6) as f32;
        loci.entry(rep).or_insert_with(|| egui::Pos2::new(
            radius * angle.cos(),
            radius * angle.sin(),
        ));
    }
    // Remove loci for components that have merged or disappeared.
    loci.retain(|rep, _| new_reps.contains(rep));
}
```

**`apply_to_state` integration** in `PhysicsProfile`:

```rust
if self.per_component_gravity {
    // Disable single-center gravity; enable per-component loci.
    state.extras.0.enabled = false;   // CenterGravity off
    state.extras.N.enabled = true;    // ComponentGravityLoci on
    state.extras.N.params.loci = workspace.component_loci.clone();
    state.extras.N.params.membership = workspace.component_membership_cache
        .clone()
        .unwrap_or_default();
    state.extras.N.params.strength = self.gravity_strength;
} else {
    state.extras.0.enabled = true;    // CenterGravity on
    state.extras.0.params.c = self.gravity_strength;
    state.extras.N.enabled = false;   // ComponentGravityLoci off
}
```

---

## 5. Imports Reference

All algorithms needed are in the existing petgraph dependency:

```rust
use petgraph::algo::{
    astar, condensation, dijkstra, has_path_connecting, kosaraju_scc,
    min_spanning_tree, toposort,
};
use petgraph::algo::dominators;
use petgraph::data::Element;          // for min_spanning_tree Element::Node / Element::Edge
use petgraph::visit::{AsUndirected, Bfs, Walker};
use petgraph::Direction;
```

`dominators::simple_fast(&graph, root)` is the entry point for dominator tree computation (§2.10).

---

## 6. Out of Scope

- **`petgraph::algo::is_isomorphic_matching`** — subgraph matching. Mentioned in §2.11 as a future
  capability; not planned for any current roadmap lane.
- **`petgraph::algo::bellman_ford`** — negative-weight shortest paths. Graphshell edge weights are
  non-negative traversal counts; Dijkstra/A* suffice.
- **`petgraph::algo::floyd_warshall`** — all-pairs shortest paths. Graph size makes this impractical
  to cache; compute on demand with single-source Dijkstra instead.

---

## 7. Execution Checklist (PR-Sized)

This section defines the first implementation slices as small, reviewable PRs.

### PR-1 (P0): Hop-Distance Cache + `dijkstra` Replacement

**Goal:** Remove hot-path repeated BFS in omnibar and replace with petgraph shortest-path primitive.

**Files expected:**
- `model/graph/mod.rs`
- `graph_app.rs`
- `shell/desktop/ui/toolbar/toolbar_omnibar.rs`

**Changes:**
1. Add `Graph::hop_distances_from(source)` using `petgraph::algo::dijkstra` + `AsUndirected`.
2. Add `hop_distance_cache: Option<(NodeKey, HashMap<NodeKey, usize>)>` to `GraphWorkspace`.
3. Add helper in omnibar path to fetch cached distances or recompute once.
4. Invalidate cache when graph structure changes (`apply_graph_delta_and_sync`).
5. Invalidate cache when primary selection changes.

**Acceptance criteria:**
1. No direct `VecDeque` BFS remains in `connected_hop_distances_for_context`.
2. Omnibar signifier and query filtering share one hop map per frame/selection.
3. Behavior parity: displayed hop counts remain unchanged for same graph state.

**Validation:**
1. Targeted tests for omnibar match/signifier behavior (existing and new).
2. `cargo check -q`.

---

### PR-2 (P1): Depth-2 Connected Candidate Cleanup

**Goal:** Replace hand-rolled depth-capped BFS loop in `gui_frame` with two-round expansion.

**Files expected:**
- `model/graph/mod.rs`
- `shell/desktop/ui/gui_frame.rs`

**Changes:**
1. Add `Graph::neighbors_undirected(key)` accessor.
2. Replace `connected_candidates_with_depth` queue loop with explicit depth-1 + depth-2 expansions.
3. Preserve current ordering and cap behavior (`MAX_CONNECTED_OPEN_NODES` logic unchanged).

**Acceptance criteria:**
1. No queue-based BFS loop remains in that function.
2. Output set/depth semantics remain equivalent for both `Neighbors` and `Connected` modes.

**Validation:**
1. Existing `gui_frame`/connected-open tests pass.
2. Add regression test for depth-2 expansion dedupe and depth annotation.
3. `cargo check -q`.

---

### PR-3 (P2): Graph Accessor Foundation

**Goal:** Land reusable graph API surface for upcoming P2/P3 features.

**Files expected:**
- `model/graph/mod.rs`

**Changes:**
1. Add `orphan_node_keys()`.
2. Add `shortest_path(from, to)` (A* with unit weights).
3. Add `is_reachable(from, to)`.
4. Add `weakly_connected_components()` and `strongly_connected_components()` helpers.

**Acceptance criteria:**
1. Accessors compile and return stable deterministic outputs.
2. Unit tests cover empty graph, disconnected graph, and cyclic graph cases.

**Validation:**
1. New graph unit tests.
2. `cargo test -q model::graph::tests -- --nocapture`.

---

### PR-4 (P2): Component Membership Cache + Gravity Loci State

**Goal:** Introduce state/cache plumbing for per-component gravity without switching physics behavior yet.

**Files expected:**
- `graph_app.rs`
- physics profile/state files where extras are configured

**Changes:**
1. Add `component_membership_cache` and `component_loci` to workspace state.
2. Recompute membership only on structural graph updates.
3. Add locus assignment utility for new/merged components.
4. Gate new behavior behind profile flag, default OFF in this PR.

**Acceptance criteria:**
1. No behavior change when flag is disabled.
2. Cache updates are deterministic and do not churn each frame.

**Validation:**
1. Unit tests for membership/loci recomputation and merge/remove handling.
2. `cargo check -q`.

---

### PR-5 (P2): Enable `ComponentGravityLoci` Force

**Goal:** Activate per-component gravity and disable single-center gravity when enabled.

**Files expected:**
- `graph/forces/component_gravity.rs` (new)
- physics profile/state integration points

**Changes:**
1. Implement `ComponentGravityLoci` `ExtraForce`.
2. Wire profile toggle: when enabled, disable `CenterGravity` and enable `ComponentGravityLoci`.
3. Seed reasonable defaults for new profiles.

**Acceptance criteria:**
1. Multi-component graphs remain visually separated under physics.
2. Single-component behavior remains stable.

**Validation:**
1. Focused physics tests and/or deterministic snapshot tests where available.
2. Manual smoke in multi-component workspace.

---

### Notes

1. Keep each PR independently shippable.
2. Preserve terminology from `design_docs/TERMINOLOGY.md` in code/docs.
3. Avoid bundling unrelated changes into algorithm replacement PRs.

---

*This spec is the authoritative reference for petgraph algorithm utilization in graphshell. Update it when new use sites are identified or implementations are completed.*
