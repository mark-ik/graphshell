# Graphlet Model

**Date**: 2026-03-25
**Status**: Architecture reference
**Scope**: Canonical graphlet semantics across Graph, Navigator, Workbench, and Shell overview surfaces.

**Related**:

- `unified_view_model.md` — five-domain model and graph-bearing surfaces
- `../implementation_strategy/graph/petgraph_algorithm_utilization_spec.md` — petgraph-backed graphlet intelligence
- `../implementation_strategy/navigator/NAVIGATOR.md` — Navigator domain responsibilities
- `../implementation_strategy/workbench/graphlet_projection_binding_spec.md` — Workbench binding and linked/detached arrangement behavior
- `../implementation_strategy/workbench/WORKBENCH.md` — Workbench arrangement authority
- `../implementation_strategy/shell/shell_overview_surface_spec.md` — Shell overview of graph/workbench/runtime state

---

## 1. Purpose

Graphshell needs one canonical answer to the question:

> what is a graphlet, and how do graphlets participate in navigation, analysis, arrangement, and overview?

This doc answers that question.

It exists because graphlets are no longer only a Workbench grouping concern. They are now part of:

- Graph-side analysis,
- Navigator-side projection and traversal,
- Workbench-side staged arrangements,
- Shell-side overview and orientation.

---

## 2. Canonical Definition

A **graphlet** is a bounded, meaningful graph subset used for navigation, understanding, comparison, or staged work.

A graphlet is defined by:

- a node set,
- an edge set,
- one or more anchors,
- a derivation or membership rule,
- a boundary/frontier,
- optional ranking metadata,
- optional presentation hints.

Graphlets are not synonymous with:

- a frame,
- a tile group,
- a saved view,
- a filter preset,
- a weakly connected component only.

Some graphlets are connected components. Others are paths, ranked frontiers, filtered subsets, or session projections.

---

## 3. Ownership Model

Graphlet semantics are intentionally split across domains.

| Concern | Owner |
|---|---|
| graph truth, graph algorithms, durable graphlet promotion | Graph |
| graphlet derivation, graphlet transitions, scoped search, navigation-oriented presentation | Navigator |
| graphlet binding to tile groups or frames | Workbench |
| shell-level summary of active graphlets and cross-domain state | Shell |

Rules:

1. Graph owns the data and algorithms a graphlet is derived from.
2. Navigator owns the user's current graphlet as a navigation object.
3. Workbench may link an arrangement to a graphlet, but does not become the owner of graphlet truth.
4. Shell may summarize graphlets, but does not define them.

---

## 4. Graphlet Kinds

There are two top-level graphlet kinds.

| Kind | Meaning |
|---|---|
| **Derived graphlet** | ephemeral graphlet computed from anchors, filters, algorithms, or traversal rules |
| **Pinned graphlet** | named, reopenable graphlet that may carry saved presentation hints and may be referenced by Workbench arrangements |

`Pinned` does not automatically mean graph truth mutation. A pinned graphlet may still be a reusable projection artifact rather than a durable graph edge family. Promotion into Graph-owned truth is a separate explicit step.

---

## 5. Useful Graphlet Shapes

Graphshell should treat these graphlet shapes as first-class conceptual targets.

### 5.1 Ego graphlet

Anchor plus radius-bounded neighborhood.

Best for:

- local exploration,
- focus-node inspection,
- compact radial or concentric layouts.

### 5.2 Corridor graphlet

Shortest path or near-shortest path family between anchors.

Best for:

- explaining why two regions are related,
- `open path`,
- relation audits.

### 5.3 Component graphlet

Weakly connected component containing an anchor.

Best for:

- thread-sized navigation,
- `open connected`,
- shell overview of disconnected work streams.

### 5.4 Loop graphlet

SCC or condensed loop cluster.

Best for:

- browsing loop inspection,
- atlas or collapse suggestions,
- repeated-circuit diagnostics.

### 5.5 Frontier graphlet

Current graphlet plus ranked candidate expansion boundary.

Best for:

- “what should I pull in next?”,
- recommendation UI,
- context-aware search scope.

### 5.6 Facet graphlet

Subgraph filtered by tags, edge families, trust state, recency, address kind, or other graph facet.

Best for:

- filtered investigation,
- semantic review,
- cleanup or trust inspection.

### 5.7 Session graphlet

Traversal-derived working thread or browsing session slice.

Best for:

- timeline views,
- history replay,
- shell overview of current task flow.

### 5.8 Bridge graphlet

Connector nodes and edges between two otherwise separate regions.

Best for:

- synthesis,
- “what connects these?”,
- cross-thread discovery.

### 5.9 Workbench-correspondence graphlet

Graph induced by currently open panes plus optional nearby context.

Best for:

- mapping open panes back to graph truth,
- shell overview,
- Workbench-local graph-bearing panes.

---

## 6. Derivation Model

Graphlet derivation should be explicit.

Suggested model:

```rust
pub struct GraphletSpec {
    pub kind: GraphletKind,
    pub anchors: Vec<NodeId>,
    pub scope: GraphletScope,
    pub selectors: Vec<RelationSelector>,
    pub ranking: Option<RankingPolicy>,
}

pub enum GraphletKind {
    Ego { radius: u8 },
    Corridor,
    Component,
    Loop,
    Frontier,
    Facet,
    Session,
    Bridge,
    WorkbenchCorrespondence,
}

pub struct ResolvedGraphlet {
    pub spec: GraphletSpec,
    pub members: Vec<NodeId>,
    pub edges: Vec<EdgeId>,
    pub frontier: Vec<NodeId>,
}
```

The important rule is not the exact byte shape. It is that graphlet derivation remains explicit and inspectable, not a side effect hidden inside one UI surface.

---

## 7. Petgraph-Backed Graphlet Intelligence

Petgraph is one of the best utility layers for graphlet derivation and enrichment.

High-value algorithmic inputs:

- hop-distance maps,
- shortest path / A* corridor computation,
- weakly connected components,
- SCCs and condensation DAGs,
- toposort,
- reachability,
- dominators,
- orphan detection.

These should feed a shared graph projection / graphlet cache under Graph authority and above renderers.

---

## 8. UI Expressions Of Graphlets

Graphlets must not be trapped in one UI form.

### 8.1 Graph UI

Graph uses graphlets for:

- neighborhood focus,
- cluster highlighting,
- path highlighting,
- component emphasis,
- frontier suggestion overlays,
- graph-side management commands.

### 8.2 Navigator UI

Navigator uses graphlets for:

- breadcrumb and context projection,
- graphlet switching,
- scoped search,
- ranked expansion lists,
- specialty layouts such as radial, corridor, timeline, atlas, and hierarchical views.

### 8.3 Workbench UI

Workbench uses graphlets for:

- linked tile groups,
- graph-bearing panes that show the current graphlet,
- open-path and open-connected flows,
- correspondence views between open panes and graph truth.

### 8.4 Shell UI

Shell uses graphlets for:

- active graphlet summary,
- cross-domain orientation,
- identifying what current work context the user is in,
- surfacing thread/cluster-level diagnostics or attention cues.

---

## 9. Graphlet Layout Guidance

Graphlets may be presented with different layouts depending on purpose.

| Layout | Best graphlet use |
|---|---|
| **Radial / concentric** | ego graphlets, local orientation |
| **Corridor / path** | corridor graphlets, bridge explanation |
| **Timeline / temporal** | session graphlets |
| **Hierarchical** | DAG-like dependency or citation graphlets |
| **Atlas / component** | component and loop graphlets |
| **Compact force / Barnes-Hut** | correspondence or exploratory graphlets |

Layouts are graph-projection policies, not canvas-only features.

---

## 10. Graphlet And Arrangement

Graphlets and Workbench arrangements are separate concerns.

- graphlet answers: which nodes and edges belong to the current meaningful subset?
- arrangement answers: how are panes, frames, and tiles currently staged?

An arrangement may be:

- **linked** to a graphlet,
- **detached** from any graphlet,
- **relinked** later.

The binding mechanics are specified in [graphlet_projection_binding_spec.md](../implementation_strategy/workbench/graphlet_projection_binding_spec.md).

---

## 11. Cross-Domain Working Model

The intended default flow is:

1. Graph establishes anchors or selected targets.
2. Navigator derives the active graphlet.
3. Workbench may open or relink an arrangement around that graphlet.
4. Viewer realizes requested facets from nodes inside that graphlet.
5. Shell summarizes the resulting state as one coherent working context.

This is the core collaboration model for the five domains.

---

## 12. Acceptance Criteria

The graphlet model is coherent when:

1. Graphlet definition is not buried inside Workbench-only docs.
2. The system can distinguish derived vs pinned graphlets.
3. Graphlets are not restricted to weakly connected component semantics.
4. Graphlet derivation can be explained in terms of explicit anchors, selectors, and algorithms.
5. Graphlet presentation can vary by surface without changing graphlet truth.
6. Workbench linkage remains explicit rather than implicit.
7. Shell overview can name the active graphlet without becoming the owner of graphlet truth.
