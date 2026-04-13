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
- `../../../archive_docs/checkpoint_2026-03-28/graphshell_docs/implementation_strategy/graph/2026-03-27_lens_decomposition_and_view_policy_plan.md` — archived implementation history for lens decomposition, view-policy authority, and provenance metadata
- `petgraph_algorithm_utilization_spec.md` — algorithmic analysis capabilities shared across surfaces
- `2026-03-11_graph_enrichment_plan.md` — automated and user-initiated graph enrichment
- `2026-04-01_swatch_spec_extraction_plan.md` — reusable compact graph projection contract for embedded swatch surfaces
- `2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md` — canonical scene-substrate plan for Vello world rendering, projected view modes, Parry query/editing, and Rapier `Simulate` behavior
- `2026-04-11_graph_canvas_crate_plan.md` — phased extraction of the Graphshell-owned `graph-canvas` crate
- `../aspect_render/2026-04-12_rendering_pipeline_status_quo_plan.md` — code-verified rendering-pipeline status; clarifies that the current graph renderer remains the active `egui_graphs` path while `graph-canvas` is still an extraction target
- `workbench_frame_tile_interaction_spec.md`
- `2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`
- `../../technical_architecture/graph_canvas_spec.md` — technical API design for the future `graph-canvas` crate
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

Archive note:

- The lens decomposition / view-policy migration plan has been completed and
  archived at `../../../archive_docs/checkpoint_2026-03-28/graphshell_docs/implementation_strategy/graph/2026-03-27_lens_decomposition_and_view_policy_plan.md`.
- Treat that document as implementation history and rationale, not an active
  execution plan.

### External pattern note (2026-04-01): RustGrapher / WasmGrapher

External graph-rendering libraries are a useful reminder that the canvas should be treated as a subsystem, not just a widget. The Graph domain should own simulation policy, scene derivation, interaction semantics, and diagnostics contracts; rendering backends should consume those contracts rather than define them.

This supports the current Graph-versus-Workbench boundary and the deferred custom-canvas direction: backend replacement should not change graph truth or graph-space semantics.

As of 2026-04-10, the active scene-substrate direction for projected graph and
scene rendering is defined in
`2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`.
Treat that document as the canonical execution anchor for Vello-backed
world rendering, Parry-owned scene queries/editor geometry, and Rapier-backed
`Simulate` behavior.

Current implementation note: the shipped graph renderer remains the `egui_graphs`
plus retained `EguiGraphState` path. See
`../aspect_render/2026-04-12_rendering_pipeline_status_quo_plan.md` for the
code-verified renderer status and execution order.

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

## 9. Canvas Spec: `graph_canvas_spec.md`

**Status**: Active architecture design; implementation remains pending.

The technical canvas subsystem spec now exists at
`../../technical_architecture/graph_canvas_spec.md`.

The implementation/extraction strategy now exists at
`2026-04-11_graph_canvas_crate_plan.md`.

**Naming direction (2026-04-11)**:

The intended subsystem/crate name for this future product-owned custom canvas is
`graph-canvas`.

That name is preferred over `graph-render` because the subsystem is expected to
own not only drawing, but also:

- scene derivation,
- camera and projection rules,
- interaction and hit testing,
- backend selection,
- and canvas diagnostics.

The active architectural anchor for that direction is
`2026-04-10_vello_scene_canvas_rapier_scene_mode_architecture_plan.md`.

### What the active spec and plan now cover

- product-owned canvas subsystem boundaries
- `graph-canvas` crate identity and API shape
- `ProjectedScene` packet seam
- camera/projection ownership for `TwoD`, `TwoPointFive`, and `Isometric`
- interaction and hit-testing contracts
- backend selection and Vello alignment
- phased extraction from the current `render/canvas_*` path

Note: Node glyph resolution — how the system selects and composes a node's visual
form — is now specified separately in `2026-04-03_node_glyph_spec.md`. The future
`canvas_render_pipeline_spec.md` will consume resolved glyphs as input to its draw
architecture.

---

## 10. Node Glyph Authority

`2026-04-03_node_glyph_spec.md` defines the **node glyph**: the visual form of a node
on the canvas. It owns glyph anatomy (body, content imagery, state rendering, LOD
presentation), glyph resolution (rule-matching pipeline from node data through theme
application to resolved shapes), and user-authored glyph rules.

The glyph spec is orthogonal to PMEST facets — facets describe what a node IS; the
glyph describes how it APPEARS. It is also distinct from the badge system, which composes
atop the resolved glyph.
