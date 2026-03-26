# Unified View Model

**Date**: 2026-03-25
**Status**: Architecture reference
**Scope**: How the Shell host and the Graph, Navigator, Workbench, and Viewer domains relate. This document is the canonical correction to the older tile-first and multi-host framing.

**Related**:

- `graphlet_model.md` — canonical graphlet semantics across domains
- `../implementation_strategy/graph/layout_algorithm_portfolio_spec.md` — layout algorithms available to graph-bearing surfaces
- `../implementation_strategy/graph/petgraph_algorithm_utilization_spec.md` — petgraph as shared projection intelligence
- `../implementation_strategy/graph/GRAPH.md` — Graph domain spec; the canvas is its primary implementation surface
- `../implementation_strategy/navigator/NAVIGATOR.md` — Navigator domain spec
- `../implementation_strategy/workbench/workbench_layout_policy_spec.md` — Workbench layout and anchoring rules
- `../implementation_strategy/shell/SHELL.md` — Shell domain spec and host/orchestration semantics
- `../implementation_strategy/shell/shell_overview_surface_spec.md` — concrete shell overview UI and cross-domain summary model
- `../research/2026-03-24_tool_comparison_product_lessons.md` — comparable product lessons

---

## 1. Executive Model

Graphshell should not be modeled as a workbench full of tiles where one tile happens to be a graph.

That model is too weak for the product being built. The graph is the primary orientation space. The Workbench is an invoked arrangement system for detailed work. The Shell is the only host.

The correct model is:

1. **Shell** is the unconditional application host and the orchestration boundary for user intent and app-level control.
2. **Graph** is the canonical truth and graph-space management domain; the **canvas** is its primary surface, not its domain name.
3. **Navigator** is the navigation-oriented projection domain that turns graph truth into scoped, task-shaped graphlets and other navigable projections.
4. **Workbench** is the invoked arrangement system for panes, frames, splits, overlays, and detailed working state.
5. **Viewer** is the content realization authority that chooses backend, fallback, and render strategy for a requested facet or content kind.

This means:

- not every meaningful surface is a tile,
- the graph does not only or primarily exist as a Workbench guest,
- the absence of any tile tree is still a valid interactive application state,
- the tile tree describes the Workbench subsystem rather than the whole UI,
- `Viewer` remains a real authority and must not disappear into generic panes or weak `Lens` terminology.

---

## 2. Why This Model Exists

This architecture is the cleanest synthesis of the product pressures already surfaced in the research and planning docs.

### 2.1 TheBrain

TheBrain validates a graph-first product where the graph is the user's world-space rather than a widget embedded inside an editor shell.

### 2.2 VS Code

VS Code validates a strong outer shell that owns commands, status, and app-level control without pretending that all user-visible structure is one tile system.

### 2.3 Notion, Anytype, Obsidian, Logseq

These tools validate multi-projection truth: graph, tree, table, timeline, kanban, and filtered sets are projections over shared truth rather than separate truth stores.

Graphshell combines those lessons as follows:

- graph-first orientation from TheBrain,
- explicit outer-host clarity from VS Code,
- projection-over-shared-truth discipline from Notion, Anytype, Obsidian, and Logseq.

---

## 3. Host And Domain Model

Host and authority are not the same thing.

There is one host:

```text
SHELL HOST
  mounts and orchestrates:
    graph surfaces
    navigator hosts
    optional workbench surfaces
    shell/control/status surfaces
```

There are five principal domains:

| Domain | Why it exists | Owns | Does not own |
|---|---|---|---|
| **Shell** | gives the app one coherent executive layer | command interpretation, app-level control, ambient status, subsystem exposure, top-level composition | graph truth, graph projection semantics, pane arrangement, content realization |
| **Graph** | keeps truth and graph-space management authoritative | node/edge identity, graph mutation, graph analysis, graph-space targeting, graphlets when promoted to durable graph structures | workbench arrangement, pane lifecycle, viewer backend choice |
| **Navigator** | lets the user traverse graph meaning without collapsing everything into canvas or panes | navigation-oriented projection, graphlet derivation, scoped search, breadcrumbs/context, reveal semantics, path/corridor/component/loop projections | graph truth, pane arrangement, viewer rendering |
| **Workbench** | makes arrangement and invocation explicit product features | pane lifecycle, frames, splits, tab strips, overlays, foregrounding, staging | graph truth, projection truth, viewer backend policy |
| **Viewer** | keeps realization policy separate from graph and pane logic | backend selection, render/fallback policy, content realization strategy, per-content interaction model | graph identity, arrangement, shell command routing |

Supporting authorities such as runtime, diagnostics, storage, accessibility, history, and security remain real, but they surface through these five domains rather than replacing them.

---

## 4. Shell As Host And Orchestration Boundary

`Shell` is the only host.

That means:

1. Shell owns top-level mounting and composition of major surfaces.
2. Shell owns command and control entrypoints such as the omnibar, command palette, global status, and subsystem access.
3. Shell is the orchestration boundary for user intent and app-level control.

It does **not** mean:

1. Shell is the semantic owner of graph truth.
2. Shell is the owner of pane truth.
3. Shell is the owner of content truth.
4. All interdomain reads and writes must literally flow through Shell as a data bus.

The correct reading is: Shell decides how user intent enters the application and how app-level state is exposed, while the owning domain still decides what the intent means and how the state changes.

---

## 5. Graph Domain And The Canvas Surface

`Graph` is the right domain name. `Canvas` is a surface.

The Graph domain owns:

- graph truth,
- graph mutation authority,
- graph-space interaction meaning,
- graph analysis and management,
- graph-side target selection and facet targeting context,
- graphlets when they are promoted into durable graph structures or named saved subsets.

The canvas is the primary Graph surface because it is where graph truth becomes spatial, interactive, and manageable. But the Graph domain is not reducible to the canvas any more than Workbench is reducible to tab strips.

This correction matters because it prevents surface names from becoming accidental domain names.

---

## 6. Navigator As A Real Peer Domain

Navigator looked thin only when defined as tree or breadcrumb UI.

Navigator becomes a real peer domain when it owns navigation-oriented projections of graph truth.

Navigator owns:

- graphlet derivation,
- scoped and context-aware search,
- structural navigation and orientation,
- breadcrumb and context projection,
- local reveal behavior,
- expansion, sectioning, grouping, and ranking models,
- projection-local navigation commands,
- graph-to-workbench and workbench-to-graph handoff contracts,
- specialty graph layouts whose purpose is navigation rather than general graph-space editing.

Navigator is therefore not a second graph truth store and not a sidebar version of Workbench. It is the domain that answers: what local world is the user traversing right now?

---

## 7. Graphlet As A First-Class Concept

Graphshell needs a first-class `graphlet` concept.

A graphlet is a bounded, meaningful graph subset used for navigation, understanding, comparison, or staged work.

Two forms are useful:

| Graphlet kind | Meaning |
|---|---|
| **Derived graphlet** | an ephemeral subgraph computed from anchors, filters, algorithms, or traversal rules |
| **Pinned graphlet** | a named, reopenable graphlet that may also carry saved presentation hints or be linked to a Workbench arrangement |

A graphlet typically includes:

- a node set,
- an induced or filtered edge set,
- one or more anchors,
- a derivation rule,
- a frontier or boundary,
- optional ranking metadata,
- optional saved presentation hints.

This is more expressive than bare selection and lighter than treating every local world as a full workspace.

### 7.1 Useful graphlet shapes

- **Ego graphlet**: anchor plus radius-bounded neighborhood
- **Corridor graphlet**: shortest path or near-shortest path family between anchors
- **Component graphlet**: weakly connected component containing an anchor
- **Loop graphlet**: SCC or condensed loop cluster
- **Frontier graphlet**: current graphlet plus ranked expansion boundary
- **Facet graphlet**: subgraph filtered by tags, edge families, trust state, recency, or address kind
- **Session graphlet**: traversal-derived working thread
- **Bridge graphlet**: connectors between two regions
- **Workbench correspondence graphlet**: graph induced by currently open panes and their nearby context

---

## 8. Graph-Bearing Surfaces And Layout Policy

Layouts are not owned by the canvas as a surface. They are graph-projection policies that may be used by any graph-bearing surface.

A **graph-bearing surface** is any surface that renders graph truth or a derived graphlet. That includes:

- the primary canvas,
- Navigator specialty panes or hosts,
- Workbench panes rendering graphlets or graph correspondence views,
- Shell overview surfaces that summarize graph state.

The key split is:

| Concern | Owner |
|---|---|
| graph truth and layout algorithms | Graph |
| choosing which graphlet or navigation projection to render | Navigator |
| hosting the surface as a pane/frame/overlay | Workbench |
| top-level exposure and overview placement | Shell |

This lets pane-hosted graph layouts exist without making the graph a Workbench-owned thing.

### 8.1 Highest-value specialty layouts

These are the strongest candidates for Navigator or pane-hosted graph presentations:

- **Radial / concentric** for local neighborhood and focus-node inspection
- **Timeline / temporal** for session, traversal, and clip chronology
- **Hierarchical** for DAG-like dependency or citation graphlets
- **Component / atlas** for cluster overview and shell-level summarization
- **Path / corridor** for relation explanation between anchors
- **Workbench correspondence** for open-pane to graph mapping

These are useful but more situational:

- **Phyllotaxis** for ranked or recency-heavy working sets
- **Grid / snapped** for highly disciplined structured views
- **Kanban / column** for tag- or status-driven graph-backed projections
- **Map / geospatial** when the data actually supports it

These are worth research but should remain secondary:

- **Penrose / aperiodic**
- **L-system / fractal**
- **full 3D inside small panes**

---

## 9. Petgraph As Shared Projection Intelligence

`petgraph` is one of the best places to make Navigator and graphlets materially smarter.

The Graph domain should expose petgraph-backed projection facts. Navigator, canvas, Workbench, and Shell overview surfaces may consume them.

```rust
pub struct GraphProjectionKey {
    pub graph_id: GraphId,
    pub projection_kind: GraphProjectionKind,
    pub root: Option<NodeId>,
    pub scope: ProjectionScope,
    pub edge_families: EdgeFamilyMask,
    pub params_hash: u64,
}
```

Pipeline boundary:

```text
Graph truth
  -> GraphProjectionRequest / GraphProjectionKey
  -> petgraph algorithm execution + keyed cache
  -> graphlet / projection facts
  -> surface-specific presentation model
  -> renderer
```

High-value petgraph-backed navigation ideas:

- hop-distance maps for search signifiers and local relevance ranking,
- shortest path for corridor views and `open path`,
- SCCs for loop views and collapse suggestions,
- weakly connected components for thread or graphlet boundaries,
- condensation for atlas-level overview and loop simplification,
- toposort for timeline or hierarchical projections,
- dominators for session funnel analysis,
- reachability for dimming, gating, and relevance checks,
- orphan detection for cleanup and integration prompts.

Projection outputs are facts, not widgets. Graph and Navigator may present the same facts differently without duplicating truth.

---

## 10. Kinds Of State

The model stays coherent only if state families remain separate.

| State family | Examples | Authority |
|---|---|---|
| **Graph truth** | `NodeId`, `GraphId`, addresses, edge families, tags, traversal records | `GraphMutation` |
| **Projection state** | graphlet scope, graph layout positions, Navigator sectioning, local sort, expansion, ranking | Graph surface or Navigator surface state |
| **Workbench state** | pane lifecycle, frame layout, split ratios, tab selection, overlays | `WorkbenchAction` |
| **Viewer state** | viewer backend choice, reader mode, document-local scroll/zoom, render target | Viewer / runtime authority |
| **Runtime state** | `TileRenderMode`, `NodeLifecycle`, caches, residency, process attachment | `RuntimeEffect` + runtime policy |
| **Shell/control state** | command mode, command history, global settings, subsystem entrypoints, ambient status | Shell / control aspect |

Sync means shared identity and explicit handoff, not one global mutable view state.

---

## 11. Node Facets And Cross-Surface Verbs

A node is one durable graph object, but multiple **facets** of that object may be requested.

Examples:

- address/content facet,
- relations/edge facet,
- tags/metadata facet,
- history/traversal facet,
- trust/diagnostics facet,
- presentation/configuration facet.

This yields the correct flow:

1. Graph or Navigator selects a `NodeId`.
2. A surface or command chooses a `NodeFacet` to inspect.
3. Workbench may activate a surface for that facet.
4. Viewer or tool realization renders it.

Cross-surface verbs:

| Verb | Meaning | Authority / destination |
|---|---|---|
| **Select** | set the active graph identity target | shared graph-side selection truth |
| **Inspect facet** | choose what aspect of a node is requested | Graph or Navigator command context, then handed to Workbench/Viewer |
| **Activate** | open or foreground a detail surface | `WorkbenchAction` |
| **Reveal** | make an already-known target visible in a local projection | local surface behavior; may require `WorkbenchAction` |
| **Scope** | change the graphlet or neighborhood being shown | projection-local state |
| **Arrange** | change pane/frame/split placement | `WorkbenchAction` |
| **Mutate** | change durable graph truth | `GraphMutation` |
| **Configure / control** | change app or subsystem behavior | Shell / control aspect |

This is why Graph owns targeting semantics while Workbench owns pane activation.

---

## 12. Reveal, Sync, And Handoff

Opening a node detail surface is not a requirement that the graph become a tile. It is a handoff from graph-first orientation into invoked detail work.

Invariants:

1. The same `NodeId` is preserved across Graph, Navigator, Workbench, and Viewer surfaces.
2. Choosing a facet is not the same thing as activating a pane.
3. Activating, foregrounding, docking, splitting, or overlaying a detail surface is `WorkbenchAction`.
4. If the graph is visible, reveal-in-graph should restore enough local projection state to orient the user.
5. If the Workbench is absent, Shell plus Graph remain a valid primary product state.
6. Local layout and scope state remain local unless explicitly handed off.

Sync should mean:

- shared identity,
- explicit scope or graphlet handoff,
- consistent reveal behavior across surfaces.

Sync should not mean:

- forcing Navigator expansion to mirror graph camera state,
- forcing multiple graph-bearing surfaces to share one layout,
- treating pane open or close as graph mutation.

---

## 13. Not Everything Is A Tile

The old tile-first model is explicitly rejected.

The following may exist without being tiles:

- Shell host surfaces,
- the primary graph canvas,
- Navigator hosts,
- ambient system status and command/control surfaces,
- shell overview surfaces.

The following are usually Workbench concerns:

- panes,
- frames,
- splits,
- tab bars,
- invoked overlays.

Consequences:

1. The tile tree is the Workbench subsystem, not the whole UI.
2. A graph-first zero-Workbench state is valid and should feel complete.
3. Workbench-hosted graph surfaces are supported, but they are one hosting mode rather than the canonical existence of the graph.

---

## 14. Shell-Level Overview

Graphshell needs a shell-level overview that does not lie by pretending graph nodes, tabs, and host state are one abstraction.

The overview should show correspondences among three truths:

- **Graph truth**: what exists and how it is related,
- **Workbench truth**: what is currently opened, staged, and foregrounded,
- **Shell truth**: what the application is doing right now.

High-value overview elements:

- active graphlet,
- primary and secondary targets,
- open pane count and current arrangement summary,
- current viewer backends and fallback state in play,
- background operations and diagnostics,
- trust, sync, or runtime warnings that affect the current working state.

This is a Shell concern because it summarizes the whole application state without becoming the semantic owner of the underlying domains. The concrete UI model for this surface is defined in `../implementation_strategy/shell/shell_overview_surface_spec.md`.

---

## 15. Omnibar Contract

The omnibar is a composite contract rather than a single-owner widget.

| Concern | Authority |
|---|---|
| input, parsing, command dispatch, command history | Shell |
| breadcrumb, containment ancestry, scope token, graphlet context | Navigator |
| active target identity feeding the current address view | shared focus/selection plus Graph/Workbench context |

The omnibar should therefore show:

1. the current active target address,
2. the current scope or graphlet token where relevant,
3. stable containment ancestry when available,
4. explicit path-inspection results only when the user requests them.

Shortest path is useful for graph explanation commands, not as the default breadcrumb source.

---

## 16. Default Product Composition

The default product composition should be:

1. Shell always present,
2. Graph surface primary,
3. one Navigator host present by default as structured complement,
4. Workbench appearing when detail work is invoked,
5. additional graph-bearing panes, Navigator hosts, and shell overview surfaces treated as layout options rather than separate product modes.

This prevents two failure modes:

- the graph becoming a decorative secondary view,
- the app requiring an invoked Workbench/editor state before the user has a coherent place to be.

---

## 17. What This Changes

This document now explicitly:

- enforces `Shell` as the only host,
- uses `Graph` as the domain name and `canvas` as a surface name,
- defines Navigator as a projection and graphlet domain rather than breadcrumb/tree UI,
- treats Workbench as an invoked arrangement system instead of the universal substrate,
- preserves Viewer as an independent realization authority,
- permits pane-hosted graph layouts without making layout canvas-only,
- establishes graphlets as a first-class architectural concept,
- makes the tile tree Workbench-scoped rather than app-scoped.

What does not change:

- `NodeId`, `OpId`, `GraphId`, and `Address` remain the identity model,
- `GraphMutation / WorkbenchAction / RuntimeEffect` remain the authority split,
- `TileRenderMode` and `NodeLifecycle` remain runtime concerns,
- graph and Navigator still operate as projections over shared truth.

---

## 18. Immediate Doc And Implementation Consequences

| Gap | What it requires | When |
|---|---|---|
| Shell-only host language everywhere | update docs that still imply multiple hosts or tile-first primacy | now |
| `Graph` domain naming | replace lingering `Canvas`-as-domain wording with `Graph`-as-domain and `canvas`-as-surface wording | now |
| Navigator strengthening | document graphlets, specialty layouts, scoped search, and petgraph-backed navigation responsibilities | now |
| Tile-tree scope correction | keep tile-tree language Workbench-scoped in terminology and strategy docs | now |
| Shared projection cache boundary | expose keyed graph projection and graphlet caches below surfaces | phase 1 |
| Zero-Workbench validity | ensure runtime and UI support graph-first operation without fake empty tile scaffolding | phase 1 |
| Optional graph-bearing panes | support pane-hosted graph surfaces as one hosting mode rather than a special exception | phase 2 |
| Shell overview surface | implement overview of graph truth, Workbench truth, and shell/runtime truth | phase 2 |

---

## 19. Backing References

This document is the backing architecture for:

| Backlog item | Connection |
|---|---|
| NV24 — Graph-Navigator Contract Sync Pass | defines sync as shared identity plus explicit scope and graphlet handoff |
| `petgraph_algorithm_utilization_spec.md` | frames petgraph outputs as shared projection intelligence |
| `layout_algorithm_portfolio_spec.md` | frames layouts as graph-projection policies usable by graph-bearing surfaces |
| Tool comparison research doc §0-§2 | converts comparable-product lessons into explicit host and authority structure |
| Foundation contract §7 | grounds views and handoff in shell-host plus five-domain semantics |
