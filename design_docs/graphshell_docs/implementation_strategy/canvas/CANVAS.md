# GRAPH — Domain Spec At The Canvas Surface

**Date**: 2026-03-25
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `graph_node_edge_interaction_spec.md`
- `2026-03-14_graph_relation_families.md` — canonical relation family vocabulary, persistence tiers, navigator projection
- `2026-03-14_canvas_behavior_contract.md` — canonical physics scenarios and computable behavioral invariants
- `2026-03-14_edge_visual_encoding_spec.md` — per-family edge stroke, color, opacity, and interaction affordances
- `2026-03-14_edge_operability_matrix.md` — per-family operability gap analysis and priority order
- `petgraph_algorithm_utilization_spec.md` — algorithmic analysis capabilities shared across surfaces
- `2026-03-11_graph_enrichment_plan.md` — automated and user-initiated graph enrichment
- `workbench_frame_tile_interaction_spec.md`
- `2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`
- `../../technical_architecture/unified_view_model.md` — unified view model; Graph is the truth + analysis + management domain and canvas is its primary surface
- `../shell/SHELL.md` — Shell domain (command interpretation and system control)
- `../navigator/NAVIGATOR.md` — Navigator domain (relationship projection and navigation)

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.9)):

- **WCAG 2.2 Level AA** — graph-space interaction targets (nodes, edges, affordances) must meet SC 2.5.8 minimum target size; focus indicators on canvas elements must meet SC 2.4.11
- **Fruchterman-Reingold 1991** — force-directed layout algorithm; parameter semantics for `Liquid`/`Gas`/`Solid` presets must be documented against this model

---

## 1. Purpose

This note defines the **Graph** domain at its primary **canvas** surface.

It exists to make one boundary explicit:

- the graph structure is a content and semantic domain,
- the canvas is its primary surface,
- and neither should be confused with Workbench arrangement.

---

## 2. What The Graph Domain Owns At The Canvas Surface

The Graph domain owns graph truth, graph meaning, and graph-space interactive
management. The canvas is the primary place where that ownership is surfaced:

### 2.1 Graph truth and identity

- node identity
- edge identity
- graph topology / relationship structure
- graphlets when promoted into durable graph structures or named saved subsets
- graph selection truth
- graph traversal semantics
- graph-side facet targeting semantics
- graph camera target semantics
- graph-space interaction meaning

### 2.2 Interactive management workspace

The graph canvas is not only a visualization surface. It is the primary
workspace where users actively manage and analyze graph truth. When the user
wants to understand, reorganize, or enrich their graph beyond what automatic
edge creation provides during normal browsing, they do so on the canvas.

Canvas-owned management capabilities:

- **Bulk operations** — selecting multiple nodes, grouping them into graphlets,
  tagging them, managing edges by user-chosen criteria. The user determines
  what edges matter for what nodes, whether scoped to one node, a graphlet, a
  whole graph view, or a selection of graph views
- **Algorithmic analysis tools** — petgraph-powered intelligence
  (`petgraph_algorithm_utilization_spec.md`) surfaced as interactive canvas
  tools: connected component grouping, betweenness centrality ranking,
  reachability filtering, shortest-path inspection, topological ordering.
  These are not just layout drivers — they are user-invocable analysis
  operations whose results are visible on the canvas and may also feed
  Navigator graphlets, specialty navigation layouts, Workbench graph-bearing
  panes, and Shell overview surfaces
- **Edge management** — creating, deleting, reclassifying, and filtering edges
  by family (`graph_relation_families.md`). The canvas is where the user goes
  to change graph truth beyond the automatic edge creation methods used as
  the tile tree grows during normal browsing
- **Cross-view operations** — applying operations that span graphlets or
  multiple graph views within the same graph. Graph views are scoped views
  of the graph (analogous to sections of a notebook); the canvas is where
  those views' contents are managed at the relationship level
- **Graph enrichment** — automated and user-initiated enrichment workflows
  (`2026-03-11_graph_enrichment_plan.md`): semantic tagging, derived edge
  creation, agent-assisted edge proposals
- **Collaboration visibility** — displaying the activities of guest
  contributors on shared graph views (Verse sync layer). The canvas is the
  surface where multi-user presence, concurrent edits, and trust boundaries
  are spatially visible

### 2.3 Ownership boundary

The Graph domain is the canonical owner of content relationships and the primary
workspace for graph-level management and analysis.

The canvas is its primary surface.

Graph is not the owner of tile layout, frame structure, or pane arrangement.

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` are not canvas-only settings.

They are a **cross-domain policy layer** that can influence:

- layout behavior
- physics behavior

Within the Canvas, these presets primarily affect:

- graph manipulation feel
- graph-space motion and convergence expectations
- force-parameter tuning and simulation stability

The Canvas must interpret these presets through Graphshell-owned policy, not through
implicit widget behavior.

That means:

- the framework may render and report events,
- but Graphshell decides what `Liquid`, `Gas`, and `Solid` mean for graph behavior.

---

## 4. Ownership Mapping

### 4.1 Canonical Graph-owned state

- graph data model
- node and edge identity
- graph selection set
- hovered or focused semantic graph target
- graph camera mode and camera target semantics
- graph interaction policy

### 4.2 State Graph does not own

- tile tree structure
- frame selection
- pane placement
- split geometry
- tab ordering

Those belong to the Workbench domain.

Navigator-local graphlets, scoped searches, path/corridor views, section ranking,
and specialty navigation layouts are also not Graph-owned truth. They are
Navigator projections over Graph-owned truth.

---

## 5. Bridges To Other Areas

The canvas surface interacts with other domains, aspects, and subsystems through explicit bridges.

### 5.1 Canvas -> Workbench bridge

Used when graph content must be shown somewhere.

Examples:

- open node
- focus existing presentation
- route a node into a pane, tile, or frame

The Canvas requests semantic intent.
The Workbench chooses and manages the destination arrangement.

### 5.2 Canvas -> Viewer bridge

Used when graph content needs a concrete presentation surface.

Examples:

- placeholder viewer
- embedded web content
- tool or document viewer

The Canvas identifies what should be shown.
The Viewer determines how it is rendered.

### 5.3 Graph -> Navigator bridge

Used when graph analysis results should be projected in the Navigator.

Examples:

- connected component groups driving Navigator section ordering
- betweenness centrality rankings informing hub-ranked sections
- reachability results filtering Navigator projection
- shortest-path and corridor results driving graphlet transitions
- SCC and condensation outputs driving loop or atlas views

Graph produces keyed graph projections (`unified_view_model.md §9`).
The Navigator consumes them for section structuring and sort ordering.

### 5.4 Graph -> Shell bridge

Used when graph operations are dispatched from Shell command surfaces.

Examples:

- omnibar-initiated node creation
- command palette graph mutations (tag, retag, delete)
- bulk operations triggered from Shell command entry

The Shell dispatches graph intents. Graph executes them as the graph
mutation authority.

### 5.5 Policy -> Canvas bridge

Used when cross-subsystem presets influence graph behavior.

Examples:

- `Liquid`: more fluid node motion and convergence
- `Gas`: higher-dispersion node motion and exploration
- `Solid`: more damped, rigid node motion

The policy layer supplies defaults.
The Canvas applies them to canvas-specific behavior.

Camera policy is separate:

- camera locks, fit behavior, zoom policy, and pan bindings are independent camera controls
- physics preset changes must not implicitly mutate camera state or camera locks

---

## 6. Architectural Rules

- The Graph domain must never derive graph truth from tile layout.
- Workbench arrangement must never redefine graph identity.
- Graph camera semantics must remain Graphshell-owned even when current rendering is driven by framework state.
- If canvas behavior is blocked or degraded, the Graph surface must surface that explicitly rather than relying on silent widget fallthrough.

---

## 7. The Five-Domain Model

Graph is one of five domains that form the coherent application model:

| Domain | Is | Owns |
|--------|----|------|
| **Shell** | Host + app-level control | command dispatch, top-level composition, settings surfaces, subsystem control, app-scope chrome |
| **Graph** | Truth + analysis + management | node identity, relations, topology, graph-space interaction, algorithmic analysis, graph enrichment |
| **Navigator** | Projection + navigation | graphlet derivation, projection rules, scoped search, interaction contract, relationship display, specialty navigation layouts |
| **Workbench** | Arrangement + activation | tile tree, frame layout, pane lifecycle, routing, split geometry |
| **Viewer** | Realization | backend choice, fallback policy, render strategy, content-specific interaction |

See `../shell/SHELL.md §5` for the full five-domain table and authority
boundaries.

---

## 8. Practical Reading

If a behavior answers:

- what content exists,
- how content relates,
- what a node or edge means,
- what graph target the camera should care about,
- how the user manages, analyzes, or enriches graph relationships,

it belongs primarily to the **Graph** domain at the **canvas** surface.

---

## 9. Deferred Spec: `canvas_render_pipeline_spec.md`

**Status**: Deferred — not yet written. Blocked on custom canvas paint callback stabilization.

A `canvas_render_pipeline_spec.md` should be created once the canvas rendering
architecture stabilizes past the `egui_graphs` custom canvas migration (see
`../aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`).

### Prerequisite

This spec is blocked on the custom canvas paint callback being established as the
stable draw entry point. Until that migration is complete, the canvas render
pipeline is partially owned by `egui_graphs` and cannot be fully specified.

### What the deferred spec must cover

When written, `canvas_render_pipeline_spec.md` must define the normative contract for:

- draw architecture: what primitives the canvas draws per frame and in what order,
- LOD (level of detail) tier thresholds: when nodes collapse to badges/thumbnails/icons
  based on camera zoom level — **must inherit and not redefine the canonical LOD tiers
  from `graph_node_edge_interaction_spec.md §4.8`** (Point / Compact / Expanded with
  `camera.scale` thresholds and hysteresis rules),
- batching policy: how draw calls for edges, node fills, badges, and labels are
  batched to minimize GPU command overhead,
- culling strategy: frustum and spatial-index culling rules for off-screen nodes/edges,
- frame-pass structure within the canvas render callback: what happens inside the
  canvas tile's composition pass (pre-pass, geometry pass, overlay pass),
- GPU resource lifecycle: buffer allocation, atlas management, texture upload policy
  for node thumbnails and badges,
- canvas-specific diagnostics channels for draw call counts, cull rates, and
  per-frame geometry budget.
