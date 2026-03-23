# UX Migration Design Spec

**Date**: 2026-03-01  
**Status**: Draft — design target and research synthesis  
**Scope**: Full UX control scheme, event architecture, layout strategy, and
interaction model for Graphshell's split chrome and hosting paradigm
(Graph Bar + graph surfaces + contextual Workbench tile tree).

**Related**:
- `TERMINOLOGY.md` — canonical vocabulary
- `subsystem_ux_semantics/ux_tree_and_probe_spec.md` — UxTree construction contracts
- `subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` — subsystem overview
- `2026-02-26_composited_viewer_pass_contract.md` — surface composition
- `PLANNING_REGISTER.md` §1A / §1C — execution entry points
- `KEYBINDINGS.md` — current keybinding map
- `aspect_input/input_interaction_spec.md` — input context stack and routing priority
- `aspect_command/command_surface_interaction_spec.md` — command surface authority and radial constraints
- `aspect_control/settings_and_control_surfaces_spec.md` — settings/control surface routing model
- `canvas/graph_node_edge_interaction_spec.md` — graph/canvas/node/edge interaction ownership
- `canvas/layout_behaviors_and_physics_spec.md` — frame-affinity organizational behavior (legacy alias: magnetic zones), force injection, lens/physics binding
- `canvas/multi_view_pane_spec.md` — canonical vs divergent graph-view contracts
- `workbench/graph_first_frame_semantics_spec.md` — graph-first frame lifecycle and cross-tree sync
- `2026-03-01_ux_migration_feature_spec_coverage_matrix.md` — feature completeness tracker (spec mapping + three-tree gate)
- `2026-03-01_ux_migration_lifecycle_audit_register.md` — current/planned/speculative lifecycle gate and pre/post renderer/networking timing audit
- `subsystem_history/edge_traversal_spec.md` — `EdgePayload`, traversal recording, edge visual contracts
- `workbench/workbench_frame_tile_interaction_spec.md` — frame/tile routing and membership semantics

---

## 1. Purpose

This document defines the **design target** for Graphshell's UX layer: the
control scheme, event architecture, layout strategy, and interaction model that
the UxTree must ultimately describe.

Graphshell is a fundamentally novel UI paradigm — a spatial graph browser
combined with a tiling workbench. No existing application provides a direct
template. This spec synthesizes external research with existing Graphshell
architecture to define a coherent UX target that:

1. Covers every user-visible interaction with **no gaps** for the average user.
2. Provides **Fitts's-law-optimized** interaction for common operations.
3. Exposes the **full power** of the graph-workbench paradigm without
   overwhelming new users.
4. Is **machine-testable** via the UxTree / UxContract system.
5. Is **user-configurable** and shareable as `WorkbenchProfile` presets.

Hierarchy note:

- the Graph Bar names and steers graph-owned targets (`GraphId`, `GraphViewId`)
- the graph/workbench surfaces below it render or host contextual leaves for that target
- the workbench tile tree is therefore a contextual presentation structure, not a peer semantic owner beside the graph

---

## 2. External Research Synthesis

### 2.1 DOM Event Model (WHATWG Living Standard)

**Source**: https://dom.spec.whatwg.org/

The DOM event dispatch algorithm provides a proven three-phase propagation
model for tree-structured UIs:

| Phase | Constant | Direction | Purpose |
|-------|----------|-----------|---------|
| Capture | `CAPTURING_PHASE` (1) | Root → Target | Global interception (shortcuts, access keys) |
| At Target | `AT_TARGET` (2) | — | Direct handling at the addressed node |
| Bubble | `BUBBLING_PHASE` (3) | Target → Root | Delegation and fallback handling |

Key design patterns applicable to Graphshell:

- **`composedPath()`** — the ordered list of nodes an event traverses. For
  Graphshell, this maps directly to a UxTree path from `Workbench` root to
  leaf `UxNode`.
- **`stopPropagation()` / `stopImmediatePropagation()`** — control flow
  mechanisms allowing any handler to prevent further dispatch. Essential for
  modal surfaces (dialogs, Radial Palette Mode) that must consume input.
- **`preventDefault()` / `cancelable`** — separating "observation" from
  "action." A handler can observe an event without preventing its default
  behavior, or cancel the default while allowing propagation to continue.
- **`EventTarget.addEventListener(options)`** — `capture`, `passive`, `once`,
  `signal` options. The `passive` flag (handler promises not to cancel) enables
  scroll/zoom optimization, directly relevant to graph canvas pointer events.
- **Synthetic dispatch via `dispatchEvent()`** — enables test injection of
  events without physical input devices. Maps to `UxBridgeCommand::InvokeUxAction`.

**Design application**: The UxTree event routing layer should implement a
three-phase dispatch model. Capture phase handles global shortcuts (Ctrl+Z,
F2, F3) at the Workbench level. Target phase handles surface-specific
interactions. Bubble phase allows unhandled events to delegate upward to
parent containers. This replaces ad-hoc input routing with a single,
predictable, testable path.

### 2.2 Faceted Classification (Ranganathan / Information Science)

**Source**: https://en.wikipedia.org/wiki/Faceted_classification

Ranganathan's PMEST formula decomposes subjects into five independent facets:
**Personality** (core identity), **Matter** (physical/material properties),
**Energy** (processes/actions), **Space** (location), **Time** (temporal
context). Unlike hierarchical (enumerative) classification which forces a
single ordering, faceted systems allow any combination of axes to describe an
object.

**Design application**: Graph nodes in Graphshell carry multiple independent
classification axes:

| Facet | Graphshell equivalent | Source |
|-------|----------------------|--------|
| **Personality** | `AddressKind` (Http, File, Data, Clip, Directory) | Node creation |
| **Matter** | `mime_hint`, content type, viewer binding | Detection pipeline |
| **Energy** | `EdgeKind` (UserGrouped, TraversalDerived), traversal count | Edge data |
| **Space** | `Frame` membership, frame-affinity region, spatial position, community cluster | Layout / user |
| **Time** | Creation timestamp, last traversal, `Traversal` history | Temporal data |

A **faceted filter system** over graph nodes replaces flat search with
multi-axis queries: "show me all PDF nodes in frame 'Research' created this
week with more than 3 traversals." Each facet is an independent filter
dimension; combining them does not require a pre-enumerated taxonomy. New
facets (UDC class, sentiment score, AI tag) can be added without disrupting
existing filter UI.

This maps to the `KnowledgeRegistry` and `Lens` system: a Lens already
composes layout + theme + physics + filters. Faceted filters become a
first-class Lens component.

### 2.3 Graph Layout Readability Metrics (Haleem et al., 2018)

**Source**: arXiv:1808.00703v1

This paper defines ten quantitative readability metrics for force-directed
graph layouts, then trains a CNN to predict them directly from graph images.
The metrics provide Graphshell with a **machine-evaluable quality measure**
for layout output.

**Metrics relevant to Graphshell**:

| Metric | Symbol | What it measures | Graphshell relevance |
|--------|--------|-----------------|---------------------|
| Node Spread | N_sp | Average distance of nodes from community centroid | Frame-affinity cohesion quality |
| Node Occlusions | N_oc | Overlapping nodes within threshold | LOD threshold validation |
| Edge Crossings | E_c | Pairwise edge intersection count | Layout algorithm quality |
| Edge Crossings Outside Communities | E_c.outside | Cross-community edge crossings | Frame-region separation quality |
| Minimum Angle | M_a | Angular distribution of incident edges | Visual clutter indicator |
| Edge Length Variation | M_l | Deviation of edge length from mean | Spring-force tuning feedback |
| Group Overlap | G_o | Overlapping community convex hulls | Frame-region overlap detection |

**Design application**: These metrics can be computed periodically (not per-frame)
and used for:

1. **Automatic layout quality scoring** — diagnostic channel
   `canvas:readability_score` emits a composite score, enabling regression
   detection when physics parameters or layout algorithms change.
2. **Readability-driven layout adaptation** — if `E_c` exceeds a threshold,
   suggest edge bundling or layout algorithm switch. If `N_oc` is high,
   increase repulsion force or switch to a sparser physics preset.
3. **LOD-aware metric computation** — metrics computed only for nodes at
   LOD ≥ Compact (per UxTree contract C5), avoiding meaningless computation at
   full-zoom-out `Point` LOD.

### 2.4 Force-Directed Graph Drawing (Foundational)

**Source**: https://en.wikipedia.org/wiki/Force-directed_graph_drawing

Force-directed layout is the foundational algorithm family for Graphshell's
graph canvas. Key properties:

**Force model**: Spring-like attractive forces (Hooke's law) on edges + 
electrically-repulsive forces (Coulomb's law) on all node pairs. The system
reaches mechanical equilibrium where edges are roughly uniform-length and
non-connected nodes are separated.

**Algorithm families**:
- **Eades (1984) / Fruchterman-Reingold (1991)**: Classic spring-electric.
  O(n²) per iteration (all-pairs repulsion). Good for small-medium graphs.
- **Kamada-Kawai (1989)**: Spring-only, ideal length proportional to
  graph-theoretic distance. Better global structure but slower convergence.
- **Barnes-Hut / FADE (2001)**: N-body simulation using spatial partitioning.
  O(n log n) per iteration. Required for graphs beyond ~1,000 nodes.
- **Stress majorization**: Monotonically convergent minimization of layout
  stress. Mathematically elegant, guaranteed local minimum.

**Combined approach** (Collberg et al.): Use Kamada-Kawai for initial global
layout, then Fruchterman-Reingold to refine local placement. Graphshell can
implement this as a `CanvasLayoutAlgorithmPolicy` strategy.

**Graphshell physics presets map to force-model tuning**:

| Physics Preset | Graphshell term | Force characteristics |
|---------------|----------------|----------------------|
| **Solid** | Grid-like | High attraction, high repulsion, low temperature → rigid |
| **Liquid** | Viscous | Moderate attraction, moderate repulsion, damping → flowing |
| **Gas** | Expansive | Low attraction, variable repulsion, high temperature → exploratory |

**Additional forces in Graphshell**:
- `SemanticGravity`: Attractive force between UDC-similar nodes (from
  `KnowledgeRegistry`). O(N) via centroid optimization.
- `FrameAffinity` bias: Soft force toward frame-affinity centroid, not a hard constraint.
- Gravity toward canvas center: Prevents disconnected components from flying
  away (already standard in force-directed systems).

### 2.5 SketchLay: User-Guided Force-Directed Layout

**Source**: https://github.com/nickoala/d3-panzoom (original:
https://github.com/.... — SketchLay, HKUST)

SketchLay allows users to guide force-directed layout via freehand sketches.
The system:

1. **Skeletonizes** the user's sketch using medial axis transform.
2. **Generates placement constraints** from the skeleton:
   - `relativePlacementConstraint` — "node A should be left of node B"
   - `alignmentConstraint` — "these nodes should be horizontally aligned"
   - `fixedNodeConstraint` — "these nodes should stay near position (x,y)"
3. **Feeds constraints** to fCoSE or CoLa layout algorithms (Cytoscape.js
   compatible).

**Design application**: Graphshell's canvas interaction model should support
**sketch-to-constraint** layout hints:

- User draws a rough boundary → system generates a frame-affinity region with the
  sketch as boundary hint.
- User draws a line → system generates alignment constraints for nearby nodes.
- User draws a circle around nodes → system generates a relative placement
  constraint keeping those nodes clustered.

These translate to **constraint overlays** on the force-directed simulation,
not a replacement for it. Constraint-based modifications feed into the
existing `CanvasLayoutAlgorithmPolicy` as an additional force/constraint layer.

This is a future-phase feature, but the architecture must accommodate it:
the `CanvasRegistry` needs a constraint injection API, distinct from the
force-parameter tuning that `PhysicsProfileRegistry` presets provide.

### 2.6 Radial / Pie Menus (Fitts's Law Optimization)

**Sources**: Wikipedia — Pie menu, Fitts's law

**Key findings**:
- Pie menus are **15% faster** and produce fewer selection errors than linear
  menus (Callahan, Hopkins, Weiser, Shneiderman, 1988).
- Selection depends on **direction**, not distance — Fitts's law is optimized
  because slices are large and close to the pointer (the "prime pixel").
- **3-12 items** per ring; beyond 12, sub-rings or nested menus are required.
- **Muscle memory** enables expert-mode gesture selection without looking at
  the menu.
- Marking menus extend pie menus with gesture tolerance, enabling expert
  shortcut paths.

**Graphshell application**: The existing Radial Palette Mode (`F3`) already implements
a pie menu. Design targets:

1. **Context-sensitivity**: Radial Palette Mode items must change based on
   selection state (no selection → graph-level actions; single node →
   node-specific actions; multi-select → group actions; edge selected →
   edge actions). The UxTree provides the context: query `UxState.selected`
   on children of the current `GraphView` node.
2. **Sector count**: Limit to **8 primary sectors** with sub-rings for
   overflow. The 8-sector layout maps perfectly to 8 compass directions,
   enabling keyboard acceleration via numpad or arrow keys.
3. **Marking menu mode**: After the user has memorized sector positions, allow
   flick-selection without rendering the menu (a straight-line gesture in the
   sector direction triggers the action). This is a future optimization.
4. **Fitts's law compliance**: Pop up centered on the pointer. All sectors
   equidistant from center. No sector should require more than one
   direction-of-movement to reach.

### 2.7 Zoomable User Interface (ZUI) Paradigm

**Source**: https://en.wikipedia.org/wiki/Zooming_user_interface

Graphshell's graph canvas is a ZUI: an infinite 2D surface with pan and zoom,
where objects change representation based on scale. Key ZUI design principles:

- **Semantic zoom**: At different zoom levels, objects show different
  representations rather than just scaling proportionally. Graphshell's LOD
  system (Point → Compact → Expanded) is exactly semantic zoom.
- **Infinite desktop**: The graph canvas has no fixed bounds. Content appears
  directly on the surface, not in windows.
- **Recursive nesting**: Graphshell's graph nodes can themselves contain
  viewable content (web pages, documents), creating a two-level ZUI — zoom
  into the graph to see nodes, "zoom into" a node (open its viewer pane)
  to see content.
- **Post-WIMP**: ZUIs are considered successors to the windowing model.
  Graphshell is a hybrid: ZUI for the graph canvas, WIMP-adjacent for the
  tile-tree workbench — the two paradigms coexist.

### 2.8 Fitts's Law (Paul Fitts, 1954)

**Source**: https://en.wikipedia.org/wiki/Fitts%27s_law

$MT = a + b \cdot \log_2\left(\frac{D}{W} + 1\right)$

Movement time increases logarithmically with the ratio of distance to target
width. UI implications for Graphshell:

| Principle | Graphshell application |
|-----------|----------------------|
| **Large targets** | Graph nodes should have generous hit areas, especially at Compact LOD. Edge hit areas should be wider than visual stroke width. |
| **Short distances** | Contextual command surfaces (Search/Context/Radial Palette modes) pop up at the pointer, not at a fixed screen location. |
| **Prime pixel** | Radial Palette Mode is centered on the activation point. Every sector is equidistant. |
| **Magic corners** | Top-level chrome (Omnibar in the Graph Bar, Workbench Sidebar edge targets) occupies screen edges, giving infinite-edge targeting in one dimension. |
| **Passive handlers** | Scroll/zoom event handlers should be registered as passive (no `preventDefault()`) to avoid blocking the compositor. In Graphshell, the `CanvasNavigationPolicy` scroll handler should never need to cancel default scroll behavior — it *is* the default. |

---

## 3. Three-Tree Architecture

Graphshell has three distinct tree structures that the UX layer must relate:

The Graph Bar sits one UI level above this section's tile-tree discussion: it
names the active graph target and view scope, while the structures below describe
how that target is presented, hosted, and probed at runtime.

```
┌─────────────────────────────────────────────────────────┐
│                      UxTree                              │
│  (per-frame semantic projection — read-only)             │
│                                                          │
│  uxnode://workbench                                      │
│  ├── /omnibar/location-field                             │
│  ├── /workbench-chrome/frame[frame-0]                    │
│  ├── /tile[graph:uuid-a]/graph-canvas                    │
│  │   ├── /node[42]   (LOD ≥ Compact only — C5)          │
│  │   ├── /node[43]                                       │
│  │   └── /status-indicator                               │
│  ├── /tile[node:uuid-b]/viewer-content                   │
│  └── /tile[tool:diagnostics]/diagnostics-panel           │
└──────────────────────┬──────────────────────────────────┘
                       │ projected from
                       ▼
┌─────────────────────────────────────────────────────────┐
│                    Tile Tree                              │
│  (egui_tiles::Tree<TileKind> — mutable, layout-owned)    │
│                                                          │
│  Root TileId                                             │
│  ├── Split (Vertical)                                    │
│  │   ├── TabGroup → Graph(GraphViewId-A)                 │
│  │   └── TabGroup → Pane(PaneState) / Node(NodePaneState)│
│  └── TabGroup → Tool(ToolPaneState::Diagnostics)         │
└──────────────────────┬──────────────────────────────────┘
                       │ renders data from
                       ▼
┌─────────────────────────────────────────────────────────┐
│                  Graph Data Model                        │
│  (Graph + Nodes + Edges — mutation via GraphIntent)      │
│                                                          │
│  GraphId → Graph                                         │
│  ├── Node(42) ─EdgePayload→ Node(43)                     │
│  ├── Node(43) ─EdgePayload→ Node(44)                     │
│  └── Node(44)                                            │
└─────────────────────────────────────────────────────────┘
```

### 3.1 Relationship Rules

| From | To | Relationship | Authority |
|------|----|-------------|-----------|
| UxTree | Tile Tree | Read-only projection (C1). Each visible `TileKind` pane → at least one `UxNode` (C3). | UxTreeBuilder |
| UxTree | Graph Data | Graph nodes at LOD ≥ Compact are emitted as `UxNode` children of their `GraphView` parent (C5). | UxTreeBuilder |
| Tile Tree | Graph Data | `TileKind::Graph(GraphViewId)` → hosts a presentation of a graph-owned scoped view already named by Graph Bar chrome. `TileKind::Pane(PaneState)` → no graph node yet. `TileKind::Node(NodePaneState)` → renders a specific `NodeKey`. | Workbench Authority |
| Graph Data | Tile Tree | `GraphIntent::OpenNode` → Workbench Authority creates new `TileKind::Node` pane. `GraphIntent::RouteGraphViewToWorkbench` hosts an existing graph-owned `GraphViewId` in workbench context. `Pane` promotion creates the `Node`, then upgrades `TileKind::Pane` to `TileKind::Node`. | Two-authority routing |
| Tile Tree | UxTree | No direct dependency. Tile Tree does not read UxTree. | — |
| Graph Data | UxTree | No direct dependency. Graph does not read UxTree. | — |

### 3.2 Event Propagation Path

When the user performs an action (click, key, gesture) on a graph node:

```
Physical Input
    │
    ▼
InputRegistry (captures raw input, maps to ActionId)
    │
    ▼
UxTree Event Path (three-phase dispatch)
    │
    ├── CAPTURE: uxnode://workbench (global shortcuts: Ctrl+Z, F2, F3)
    │           uxnode://workbench/tile[graph:uuid-a] (pane-level capture)
    │
    ├── TARGET:  uxnode://workbench/tile[graph:uuid-a]/graph-canvas/node[42]
    │            (node-specific handlers: click-select, contextual palette invocation)
    │
    └── BUBBLE:  uxnode://workbench/tile[graph:uuid-a]/graph-canvas
                 (canvas-level fallback: drag-pan, deselect)
                 uxnode://workbench/tile[graph:uuid-a]
                 (pane-level fallback: focus management)
                 uxnode://workbench
                 (workbench-level fallback: unhandled input logging)
    │
    ▼
ActionRegistry.execute(ActionId) → Vec<GraphIntent>
    │
    ▼
Graph Reducer (data mutations) or Workbench Authority (tile mutations)
```

This three-phase model replaces the current direct `InputRegistry → ActionId →
ActionRegistry` pipeline with an intermediate **UxTree-routed dispatch** that
provides:

- **Testability**: Every event has a deterministic path through the UxTree,
  observable by UxProbes and UxContracts.
- **Context sensitivity**: The target `UxNode` determines which actions are
  available (`UxAction` list).
- **Modal isolation**: A dialog or Radial Palette Mode surface at `AT_TARGET` phase can
  `stopPropagation()` to consume input, preventing it from reaching the
  canvas.
- **Accessibility**: Screen readers and keyboard navigation use the same
  event path as pointer input.

### 3.3 Authority Trajectory: UxTree as UX Source of Truth

Current architecture remains a two-authority runtime:

- **Graph authority**: graph truth (`Node`, `EdgePayload`, traversal state)
- **Workbench authority**: arrangement interaction/session mutation truth (tile tree / frame structure), with durable arrangement able to be graph-rooted where specified

`UxTree` is the authoritative **UX semantic projection** over both authorities.
It does not yet replace either data structure. It is the runtime truth for:

- discoverable actions (`UxAction`),
- focus/navigation contracts,
- accessibility semantics,
- automation and probe assertions.

Long-term direction: evolve toward Graphshell-owned canvas/workbench runtime
structures that reduce framework-semantic ownership in `egui_graphs` and
`egui_tiles`. This migration spec treats that as a convergence roadmap, not a
near-term rewrite. Near-term contract: `UxTree` coordinates and operationalizes
the existing graph + tile authorities without violating two-authority
invariants.

---

## 4. Faceted Object Model

### 4.1 Facet Schema

Every graph node exposes a **facet projection** — a set of independent
classification axes derived from durable node truth plus graph/workbench/runtime
state, queryable for filtering, grouping, and Lens composition.

This projection is not the canonical node datastructure. The source-of-truth
split is:

- `NodeRecord` / node data fields: durable graph truth (`NodeId`/`NodeKey`,
  address, title, `mime_hint`, history, provenance, semantic tags, user
  overrides)
- `NodeFacetProjection`: PMEST-aligned query/group/route projection derived
  from node truth and other authorities
- `PresentationFacet`: document/schematic/timeline/dependency/metadata view
  mode chosen at runtime without changing node identity

Representative shape:

```rust
/// PMEST-aligned facet projection for a graph node.
/// Each facet is independently filterable and combinable.
struct NodeFacetProjection {
    // ── Personality (core identity) ──
    address_kind: AddressKind,          // derived from node address
    title: String,
    domain: Option<String>,             // derived from address

    // ── Matter (content properties) ──
    mime_hint: Option<MimeType>,
    viewer_binding: ViewerKind,         // derived from ViewerRegistry / pane attachment
    content_length: Option<u64>,        // derived or cached content metadata

    // ── Energy (activity/process) ──
    edge_kinds: HashSet<EdgeKind>,      // graph projection
    traversal_count: u32,               // graph/history projection
    in_degree: u32,                     // graph projection
    out_degree: u32,                    // graph projection

    // ── Space (position/grouping) ──
    frame_memberships: Vec<FrameId>,    // workbench projection
    frame_affinity_region: Option<FrameId>, // workbench projection
    spatial_cluster: Option<ClusterId>, // layout/knowledge projection
    udc_classes: Vec<UdcTag>,           // projected from canonical semantic tags

    // ── Time (temporal context) ──
    created_at: Timestamp,
    last_traversal: Option<Timestamp>,
    lifecycle: NodeLifecycle,           // Active, Warm, Cold, Tombstone
}
```

`NodeFacetProjection` exists to support PMEST filtering/routing. It should not
accumulate every possible node field. New durable node fields belong on the
node record first; they become facet keys only when they are meaningful as a
query/group/route axis.

### 4.2 Faceted Filter UI

The faceted filter is a **Lens component** — part of the Lens composition
pipeline in `LensCompositor`. Filters are additive:

```
Lens = Layout Algorithm × Theme × Physics Profile × Faceted Filter Set
```

Filter operations (implemented as `GraphIntent` variants or direct Lens
mutations):

- `FilterByFacet(facet_key, predicate)` — show only nodes matching predicate
- `GroupByFacet(facet_key)` — visually cluster nodes by facet value (creates
  temporary frame-affinity regions)
- `ColorByFacet(facet_key)` — map facet values to node badge/color (via
  ThemeRegistry)
- `SortEdgesBy(facet_key)` — order edges by facet value in list views

**Facet composition**: Filters can be combined with AND/OR logic. The filter
pane presents each active facet as an independent row with toggle/slider
controls, following the faceted search pattern from library science.

### 4.3 Facet Rail Navigation and Node-Specific Facet Panes

When exactly one node is selected, Graphshell exposes a **Facet Rail**
interaction. The user can arrow through PMEST facets and press Enter to open
the node-specific pane for that facet.

| Facet | Enter action | Destination pane |
|------|---------------|------------------|
| **Personality** | Route/open to node address identity | `NodePane` in identity/address mode |
| **Matter** | Show node internals and content metadata | `NodePane` in details mode (viewer binding, MIME, metadata) |
| **Energy** | Show relationship/process details | Edge + traversal pane (edge kinds, traversal count, dominant direction, trigger breakdown) |
| **Space** | Show structural memberships and tags | Membership pane (frame memberships, frame-affinity region, UDC tags) |
| **Time** | Show temporal history/version stream | Timeline pane (node audit history + traversal-linked events) |

Energy is intentionally scoped as weaker/optional in first implementation:
minimum contract is traversal + edge-kind summary; advanced analytics can be
added later.

**General-pane pattern**: These are generic pane types parameterized by
`NodeKey`, not one-off node-specific views.

---

## 5. Complete Control Scheme

This section defines the **target control scheme** — every interaction a user
can perform with the graph canvas and the workbench tile tree. This replaces
the current partial keybinding map with a comprehensive, gap-free specification.

### 5.1 Selection

| Action | Primary Input | Alternate Input | UxAction |
|--------|--------------|----------------|----------|
| Select single node | Left-click on node | Enter (when node focused) | `Invoke` |
| Add to selection | Ctrl+Left-click on node | Shift+Enter (when node focused) | `Invoke` |
| Select all nodes | Ctrl+A | — | `Invoke` |
| Deselect all | Escape (when selection exists) | Left-click on empty canvas | `Invoke` |
| Select connected | Ctrl+Shift+A (expands selection to neighbors) | — | `Invoke` |
| Lasso select (replace) | Right-drag (default) | — | `Invoke` |
| Lasso add to selection | Right+Shift-drag | — | `Invoke` |
| Lasso toggle selection | Right+Alt-drag | — | `Invoke` |

**Design notes**:
- **Lasso geometry**: Freeform polygon. The selection test is
  point-in-polygon for node centers, not bounding boxes.
- **Selection order**: Multi-select maintains insertion order. The first
  selected node is the "source" and the last is the "target" for edge
  operations.
- **Selection feedback**: Selected nodes receive `UxState.selected = true`.
  The active selection count and source/target pair are displayed in the
  status bar.

### 5.1A Single-Node Facet Navigation

| Action | Primary Input | Alternate Input | Behavior |
|--------|--------------|----------------|----------|
| Focus facet rail | `F` (single node selected) | Radial → "Inspect Facets" | Enters facet-rail mode on selected node |
| Next facet | `Right Arrow` | `Down Arrow` | Advances Personality → Matter → Energy → Space → Time |
| Previous facet | `Left Arrow` | `Up Arrow` | Reverse cycle |
| Open selected facet pane | `Enter` | Double-click facet chip | Opens node-specific pane for active facet |
| Exit facet rail | `Escape` | — | Returns to normal node selection mode |

**Input contract alignment**: In facet-rail mode, arrow keys are consumed by
facet navigation (context-specific binding), not camera pan, matching
`aspect_input/input_interaction_spec.md` context-priority semantics.

### 5.2 Target-Locked Zoom

| Action | Primary Input | Alternate Input | Behavior |
|--------|--------------|----------------|----------|
| Zoom in | Scroll wheel up / `+` | Pinch-out (touch) | Pointer-relative: zoom toward cursor |
| Zoom out | Scroll wheel down / `-` | Pinch-in (touch) | Pointer-relative: zoom away from cursor |
| Zoom reset | `0` | — | Reset to default zoom level |
| Camera zoom-fit lock | `Z` | — | Toggle zoom follow-fit lock for active graph view |
| Camera position-fit lock | `C` | — | Toggle position follow-fit lock for active graph view |
| Zoom to node | Double-click on node | — | Center and zoom to Compact LOD for that node (zoom scale ≥ 0.55) |
| Lock zoom to selection | `Ctrl+L` | — | Auto-fit maintains selection in viewport as graph moves |

**Pointer-relative zoom**: The point under the cursor stays fixed during zoom.
This is the universally expected behavior for ZUI applications and avoids the
disorienting "zoom to center" pattern. Implemented in `CanvasNavigationPolicy`.

The zoom handler should be registered as **passive** (per Fitts's law / DOM
event model insight) — it does not need to cancel any browser default behavior
because it *is* the behavior.

### 5.3 Node Manipulation

| Action | Primary Input | Alternate Input | GraphIntent |
|--------|--------------|----------------|-------------|
| Create new node | `N` | Radial Palette Mode → "New Node" | `CreateNode` |
| Delete selected nodes | `Delete` | Context Palette Mode → "Delete" | `RemoveNode` |
| Clear entire graph | `Ctrl+Shift+Delete` | — | `ClearGraph` |
| Move node (drag) | Left-drag on node | Arrow keys (when node focused) | — (direct position update) |
| Pin node (toggle) | `L` | Context Palette Mode → "Pin" | `TogglePin` |
| Pin selected nodes | `I` | — | `PinNodes` |
| Unpin selected nodes | `U` | — | `UnpinNodes` |
| Duplicate selection | `Ctrl+D` | — | `DuplicateNodes` |
| Group-move selection | Drag any selected node | — | Moves all selected as unit |

**Group manipulation**: When multiple nodes are selected and the user drags one
of them, the entire selection translates as a rigid body. The relative
positions of selected nodes are preserved. This is a standard graph editor
interaction.

### 5.4 Edge Management

| Action | Primary Input | Alternate Input | GraphIntent |
|--------|--------------|----------------|-------------|
| Connect source → target | `G` (with 2+ selected) | Radial → "Connect" | `CreateEdge` |
| Connect bidirectional | `Shift+G` | — | `CreateEdge` (×2) |
| Remove edge | `Alt+G` (with 2 selected) | — | `RemoveEdge` |
| Walk edge (forward) | `Tab` (on edge focus) | — | Navigate to edge target |
| Walk edge (backward) | `Shift+Tab` (on edge focus) | — | Navigate to edge source |
| Show edge traversal history | Hover on edge for 500ms | — | Tooltip with traversal list |
| Cycle edge types | `E` (with edge selected) | — | Cycle `EdgeKind` |

**Edge event register**: All edge creation and traversal events are recorded
in the `Traversal` history. The edge traversal history is available in the
History Manager and as a diagnostic channel (`graph:edge_traversal`).

**Edge walking / traversal**: When a node has focus and the user presses Tab,
focus moves to the next edge. Pressing Tab again traverses the edge to the
connected node. Shift+Tab reverses direction. This provides a graph-theoretic
Tab traversal complementary to the tile-tree F6 region cycling.

### 5.4A Traversal Event Stream Interaction

Traversal semantics are event-first:

- **Traversal is a directed event** in the temporal stream.
- **Edge is a durable relationship record** that stores traversal-derived state.

Canonical interaction model:

1. A navigation action appends a directed traversal event
   (`from_node`, `to_node`, `timestamp`, `trigger`, `direction`).
2. The reducer projects traversal events into `EdgePayload` state for the node
   pair.
3. The first traversal on a pair promotes relationship state to include
   `TraversalDerived`.
4. Additional traversals enrich metrics/timeline without changing identity.

Important distinctions:

- Edge direction in UI is a **derived summary** (dominant traversal direction),
  not the fundamental identity of the edge.
- An edge may exist without traversals (`UserGrouped` assertion), then later
  accumulate traversals and become both asserted and traversal-active.
- Traversal stream and workbench-structure stream remain separate sources,
  merged only in history surfaces for user inspection.

### 5.5 Physics Simulation Control

| Action | Primary Input | Alternate Input | Behavior |
|--------|--------------|----------------|----------|
| Toggle physics | `T` | Radial → "Physics On/Off" | Start/stop simulation |
| Reheat simulation | `R` | — | Reset temperature from current positions |
| Physics preset cycle | `Shift+T` | Settings → Physics | Cycle Solid → Liquid → Gas |
| Physics settings panel | `P` | — | Toggle physics parameter UI |

### 5.6 Command Surfaces

Graphshell command invocation is unified around one **Command Palette shell** with multiple modes.

| Surface | Trigger | Modality | Optimal use case |
|---------|---------|----------|-----------------|
| **Search Palette Mode** | `F2` | Search-first command surface with scope dropdown | Large action sets, scoped query/command flows |
| **Radial Palette Mode** | Right-click contextual summon / profile preference | Radial, 2-tier category→option rings | High-frequency spatial actions |
| **Omnibar** | Click / `Ctrl+L` | Text input + completions | Navigation, URL entry, search |
| **Context Palette Mode** | Right-click contextual summon / profile preference | Tier-1 horizontal categories + Tier-2 vertical options | Precise contextual browsing |

**Unification strategy**:

1. **One palette shell, multiple modes**: right-click summons the contextual
  command palette shell; profile decides whether it opens in Search Palette
  Mode, Context Palette Mode, or Radial Palette Mode.

2. **Right-click search-first option**:
  - in Search Palette Mode, right-click opens a search bar at pointer context,
  - search bar includes a scope dropdown (for example: current target, active pane, active graph, workbench),
  - scope changes filter/rank behavior without changing command authority.

3. **Shared two-tier semantics across contextual modes**:
  - Tier 1 = category selection,
  - Tier 2 = options for selected category,
  - category/option ordering, pinning, and availability are shared across modes.

4. **Context Palette Mode**:
  - Tier 1 is a horizontally scrollable category strip,
  - Tier 2 is a vertically scrollable option list for selected category,
  - categories are pinnable and editable.

5. **Radial Palette Mode**:
  - hub-circle outline appears at summon point,
  - Tier 1 category buttons sit on periphery rail,
  - selecting category opens Tier 2 option ring,
  - buttons are compact by default and expand on hover for clickability,
  - labels are bounded radial text fields (hidden until hover; gentle in-field reveal),
  - supports both drag-gesture and click/hover selection.

6. **Omnibar → navigation/search companion**: The Omnibar remains a navigation
  and address-entry companion, but command/search flows must also be reachable
  from Search Palette Mode so graph-focused fullscreen workflows do not depend
  on omnibar visibility.

7. **Search Palette Mode → universal fallback**: `F2` opens the full searchable
  palette with scope selection.
   All `ActionRegistry` entries are exposed. Filter by typing. Context-first
   ordering (actions relevant to current selection at the top).

8. **Overflow and fallback rules**:
  - category and option rings page deterministically when over capacity,
  - no button/label stacking at same radial lane,
  - if radial non-overlap cannot be satisfied at current sizes, degrade to Context Palette Mode with explicit notice.

**Keyboard acceleration for radial palette**: The radial palette's visible sectors map to
compass directions. When Radial Palette Mode is open, `1-8` (numpad) or arrow-key
combinations select sectors without pointer movement. This provides the
muscle-memory efficiency of marking menus via keyboard.

**Radial readability contract**:

- radial labels are bounded in-field and non-overlapping,
- second-tier entries are not z-stacked,
- sector hit regions meet minimum target-size constraints at desktop DPI,
- overflow behavior is deterministic and test-covered (`radial_menu_structural`).

### 5.7 Frame Management

| Action | Primary Input | Alternate Input | WorkbenchIntent |
|--------|--------------|----------------|-----------------|
| Create frame from selection | `Ctrl+Shift+N` | — | Creates new Frame containing selected nodes |
| Switch frame | Workbench Sidebar frame selector / `Ctrl+Tab` | — | Activate Frame by ordering |
| Close frame | Workbench Sidebar frame action / `Ctrl+W` | — | Remove Frame |
| Split pane horizontal | `Ctrl+Shift+H` | Drag to edge | SplitPane(Horizontal) |
| Split pane vertical | `Ctrl+Shift+V` | Drag to edge | SplitPane(Vertical) |
| Promote to tab | Drag node to tab bar | — | Opens node viewer as tab in target tab group |
| Dock pane | `Ctrl+Shift+D` | — | Toggle PanePresentationMode to Docked |
| Cycle focus between regions | `F6` / `Shift+F6` | — | Move focus between panes |

**Frame membership region contract**:

- each Frame gets a stable color token for visual membership overlays,
- nodes with membership in the active frame render inside a colored pull region,
- when a tile is added to a frame, associated nodes are assigned frame membership
  and receive a soft pull toward the frame region centroid,
- this pull is a visual/workbench affinity signal, not identity duplication.

### 5.7A Graph-First Frame Semantics (Cross-Tree Organizational Contract)

Frames are treated as graph-level organizational entities first, with optional
workbench handles.

Core semantics:

- a Frame can exist in graph scope without being open in the workbench,
- opening a Frame in the workbench creates a view/handle, not the Frame itself,
- adding/removing a node via tile manipulations must update graph-side frame
  membership truth,
- closing/deleting a frame handle in the workbench must not delete the
  underlying graph Frame,
- deleting graph Frame identity is a separate explicit destructive action.

Analogy contract:

- closing a node pane is like closing an open file view,
- closing a frame handle is like closing an open folder view,
- neither operation destroys the underlying graph object.

Transitional note: `MagneticZone` is legacy terminology only, not an implemented
runtime authority. The canonical model is `Frame` / `Frame membership` / `Frame-affinity region`
as defined in `workbench/graph_first_frame_semantics_spec.md §3`. The visual canvas
backdrop for a frame's members is a frame-affinity region; the term `MagneticZone`
must not appear in new code or docs. Closing a frame handle (`CloseFrameHandle`) is
non-destructive; `DeleteFrame` is the explicit destructive path requiring confirmation.

### 5.8 Multiple Graph Views

| Action | Primary Input | Description |
|--------|--------------|-------------|
| Open new Canonical view | `Ctrl+Shift+G` | New `GraphLayoutMode::Canonical` pane (shared layout) |
| Open Divergent view | `Ctrl+Alt+G` | New `GraphLayoutMode::Divergent` pane (independent layout) |
| Sync cameras | `Ctrl+Shift+S` | Toggle camera sync between two graph views |
| Link selection | toggle in view menu | Two views share selection state |

**Multiple views per Workbench**: A single Workbench (`GraphId`) can host
multiple `TileKind::Graph` panes. `Canonical` views share node positions (one
physics simulation). `Divergent` views have independent `LocalSimulation`
instances. Cameras are per-view (independent pan/zoom). Selection can be
optionally linked.

### 5.9 Multiple Workbenches

| Action | Primary Input | Description |
|--------|--------------|-------------|
| Create new Workbench | `Ctrl+Alt+N` | New Workbench with empty graph |
| Switch Workbench | `Ctrl+Alt+Tab` | Cycle between Workbenches (Inter-Workbench Scope) |
| Merge graphs | Drag node from one workbench to another | Import node+edges into target graph |

### 5.10 User Configuration

The entire control scheme is **user-configurable**:

```
Workflow = Lens × WorkbenchProfile

WorkbenchProfile = {
    keybindings: HashMap<InputSequence, ActionId>,
    radial_menu_layout: RadialMenuConfig,
    command_palette_ordering: PaletteOrdering,
    mouse_button_mapping: MouseButtonConfig,
    scroll_zoom_requires_ctrl: bool,
    lasso_button: MouseButton,
    physics_preset_default: PhysicsPresetId,
    focus_mode: FocusBehavior,
    ...
}
```

**Profile sharing**: `WorkbenchProfile` and `Lens` configurations are
serializable (rkyv). Users can export and import profiles. Profiles can be
distributed as community presets via Verse (`BlobType::WorkflowProfile`).

**Default profiles**:

| Profile | Optimized for | Key behaviors |
|---------|--------------|---------------|
| **Standard** | Mouse + keyboard on desktop | Left-click select, right-drag lasso, scroll zoom |
| **Laptop** | Trackpad, no mouse | Two-finger scroll zoom, tap select, three-finger lasso |
| **Accessibility** | Keyboard + screen reader | Tab navigation primary, all Radial Palette Mode actions have keyboard equivalents, high-contrast LOD thresholds |
| **Touch** | Tablet / touch screen | Tap select, long-press context, pinch zoom, two-finger pan |
| **Power User** | Keyboard-centric workflow | Vim-style modal navigation, all actions via keyboard, no pointer required |

---

## 6. Layout Strategy

### 6.1 Layout Modes

The physics presets (Solid, Liquid, Gas) define force-model parameters. But
Graphshell also needs **layout modes** that control higher-level arrangement
policy:

| Mode | Metaphor | Behavior | Use case |
|------|----------|----------|----------|
| **Force-directed** (default) | Physics simulation | Nodes settle under spring-electric forces | General exploration |
| **Grid / Solid** | Spreadsheet | Nodes snapped to grid positions, no physics | Organization, classification |
| **Hierarchical** | Tree/DAG | Nodes arranged by traversal depth/direction | Navigation path analysis |
| **Radial from focus** | Target diagram | Selected node at center, neighbors in rings | Ego-network exploration |
| **Community-clustered** | Frame affinity regions | Force-directed with strong intra-frame attraction | Community structure analysis |

Layout modes are `CanvasLayoutAlgorithmPolicy` entries registered in
`LayoutRegistry`. The `CanvasRegistry` resolves the active algorithm from the
Lens.

### 6.2 Readability-Driven Adaptations

Using the readability metrics from Haleem et al., the layout system can
**automatically suggest** adaptations:

| Condition | Metric | Adaptation |
|-----------|--------|------------|
| High edge crossings | E_c > threshold | Suggest edge bundling or hierarchical layout |
| High node occlusion | N_oc > 0 | Increase repulsion force, switch to Gas preset |
| Poor community separation | G_o > 0 | Increase frame-affinity attraction, add inter-region repulsion |
| High edge length variation | M_l > threshold | Suggest uniform-spring layout (Kamada-Kawai) |
| Low minimum angle | M_a < threshold | Increase angular separation force |

These are **suggestions**, not automatic changes. The diagnostic channel
`canvas:layout_readability` emits metric snapshots and suggestions. The user
can accept or dismiss.

### 6.3 LOD and Semantic Zoom

| LOD Level | Zoom Threshold | Node Representation | UxTree Emission |
|-----------|---------------|---------------------|-----------------|
| **Point** | < 0.55 | Colored dot | NOT emitted (C5) — `StatusIndicator` instead |
| **Compact** | 0.55 – 1.10 | Label + icon badge | Emitted, minimal state |
| **Expanded** | ≥ 1.10 | Full label + badges + edge labels + metadata | Emitted, full state |

**Authority note**: These 3 LOD levels and thresholds are canonical per `canvas/graph_node_edge_interaction_spec.md §4.8`. The previous 4-level table (Point/Compact/Standard/Detail) is superseded. `CanvasStylePolicy` defines the thresholds; the accessibility profile may shift them to expose more nodes at lower zoom.

LOD thresholds are defined in `CanvasStylePolicy` and are user-configurable
via the Lens. The accessibility profile uses lower thresholds (more content
visible at lower zoom levels) to ensure screen-reader users have more nodes
available in the UxTree.

---

## 7. Event Architecture Detail

### 7.1 Event Types

Inspired by the DOM model, Graphshell defines the following event taxonomy:

```rust
enum UxEventKind {
    // ── Pointer events ──
    PointerDown { button: MouseButton, position: Vec2, modifiers: Modifiers },
    PointerUp { button: MouseButton, position: Vec2, modifiers: Modifiers },
    PointerMove { position: Vec2, delta: Vec2, modifiers: Modifiers },
    PointerEnter,   // hover-in
    PointerLeave,   // hover-out
    
    // ── Keyboard events ──
    KeyDown { key: Key, modifiers: Modifiers },
    KeyUp { key: Key, modifiers: Modifiers },
    
    // ── Scroll / Zoom ──
    Scroll { delta: Vec2, modifiers: Modifiers },  // passive by default
    PinchZoom { scale_delta: f32, center: Vec2 },
    
    // ── Focus ──
    FocusIn,    // received focus (bubbles)
    FocusOut,   // lost focus (bubbles)
    Focus,      // received focus (does not bubble)
    Blur,       // lost focus (does not bubble)
    
    // ── Custom / Synthetic ──
    Action { action_id: ActionId },
    UxBridgeCommand(UxBridgeCommand),  // test injection
}
```

### 7.2 Dispatch Algorithm

```
dispatch(event, target_path: Vec<UxNodeId>):
    // target_path = composedPath() from root to target node
    
    // Phase 1: CAPTURE (root → target)
    for node in target_path[..target_path.len()-1]:
        call capture handlers on node
        if event.propagation_stopped: return
    
    // Phase 2: AT_TARGET
    call handlers on target_path.last()
    if event.propagation_stopped: return
    
    // Phase 3: BUBBLE (target → root)
    if event.bubbles:
        for node in target_path[..target_path.len()-1].iter().rev():
            call bubble handlers on node
            if event.propagation_stopped: return
    
    // Phase 4: DEFAULT ACTION
    if !event.default_prevented:
        execute default action for event kind + target
```

### 7.3 Modal Isolation

Modals (dialogs, Radial Palette Mode, command palette) implement isolation by:

1. **Capture handler at Workbench level** checks if a modal is active.
2. If modal is active, all events are redirected to the modal's UxNode
   subtree.
3. Events that don't hit the modal subtree are consumed
   (`stopPropagation()`) — they don't reach the canvas.
4. `Escape` is always handled at the modal level, dismissing it and
   returning focus to the previous surface.

---

## 8. Migration Strategy

### 8.1 What Changes

| Current behavior | Migration target | Breaking? |
|-----------------|-----------------|-----------|
| `InputRegistry → ActionId → ActionRegistry` (direct) | Three-phase UxTree event dispatch | No — old pipeline becomes the Phase 2/Phase 4 handler |
| Separate contextual popup menu | Context-sensitive Command Palette | Yes — right-click behavior changes |
| Fixed keybindings | `WorkbenchProfile`-configurable bindings | No — defaults match current |
| No lasso modifier variants | Right+Shift/Alt/Ctrl variants | No — additive |
| Single Graph View per workbench | Multiple Canonical/Divergent views | No — additive |
| No faceted filtering | Lens-integrated faceted filters | No — additive |
| No layout readability metrics | Periodic metric computation + suggestions | No — additive |

### 8.2 Implementation Phases

| Phase | Dependencies | Deliverables |
|-------|-------------|-------------|
| **Phase A: Event architecture** | UxTree (current Phase 0-2 of #251) | Three-phase dispatch, `UxEventKind` enum, dispatch algorithm, capture/bubble handlers |
| **Phase B: Control scheme audit** | Phase A | Full keybinding map as `WorkbenchProfile`, all `ActionId` entries for §5.1–5.9, default profile |
| **Phase C: Command surface unification** | Phase B | Context-sensitive Command Palette, Radial Palette Mode context awareness, Search Palette scope dropdown, Omnibar slash-commands |
| **Phase D: Faceted model** | KnowledgeRegistry | `NodeFacets` struct, faceted filter Lens component, filter UI surface |
| **Phase E: Layout modes** | CanvasRegistry layout injection | Additional `CanvasLayoutAlgorithmPolicy` entries, readability metrics, grid/hierarchical/radial layouts |
| **Phase F: Multi-view & multi-workbench** | Phase B | Multiple `GraphView` panes per workbench, Divergent mode, camera sync, workbench switching |
| **Phase G: User configuration** | Phase B + C | `WorkbenchProfile` serialization, profile import/export, default profile presets, Verse sharing |

### 8.3 Invariants Preserved

Throughout the migration, these invariants must hold:

- **C1-C5**: UxTree construction contracts remain binding.
- **Two-authority model**: Graph Reducer owns data mutations; Workbench
  Authority owns tile-tree mutations. The event dispatch layer routes to the
  correct authority.
- **Domain sequencing**: Layout resolves before Presentation.
- **Mod-first principle**: Core seed functionality remains complete without mods.
- **S-series structural invariants** (from UxContract register): unique
  UxNodeIds, no orphan nodes, complete visibility coverage.
- **Accessibility**: Every UxNode reachable via event dispatch is also
  reachable via keyboard navigation (F6 region cycling + Tab within regions).

### 8.4 Spec Promotion Candidates (Split from this Migration Spec)

This migration spec now defines multiple concerns that should become dedicated
component specs once initial alignment lands.

| Candidate spec | Why promote | Owning component |
|---------------|-------------|------------------|
| **Faceted Filter Surface Spec** | Filter grammar, facet operators, ranking, result surfacing are substantial enough for independent acceptance criteria | Canvas + Command aspects |
| **Facet Pane Routing Spec** | Single-node facet rail (`Arrow`/`Enter`) and node-specific pane routing deserves its own input/command/focus contract | Input + Command + Viewer |
| **Radial Palette Geometry & Overflow Spec** | Ring layout, overflow, and readability constraints need geometry-level invariants and tests | Command aspect |
| **Graph-First Frame Semantics Spec** | Defines frame identity lifecycle across graph/workbench trees (open/close/delete distinctions and membership sync) | Workbench + Canvas + System |
| **UxTree Convergence Roadmap** | Long-term migration away from framework-owned semantics should be explicitly staged and risk-scoped | UX Semantics subsystem |
| **Layout Algorithm Portfolio Spec** | Heuristic portfolio, switch strategy, and metric thresholds are larger than UX control mapping | Canvas subsystem |

Operational tracker: use `2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
as the canonical feature-to-spec completeness matrix for this migration.

Lifecycle/timing tracker: use `2026-03-01_ux_migration_lifecycle_audit_register.md`
as the canonical current/planned/speculative audit plus pre/post
renderer/WGPU/networking gate register.

### 8.5 Canonical-vs-Stale Doc Handling (Applied in this revision)

For this migration spec, interaction framing is grounded in canonical specs
dated 2026-02-27 through 2026-03-01. Older canvas/control plans are treated as
historical input unless reaffirmed by their canonical successor specs.

Review set applied:

- `subsystem_history/edge_traversal_spec.md`
- `canvas/graph_node_edge_interaction_spec.md`
- `canvas/layout_behaviors_and_physics_spec.md`
- `canvas/multi_view_pane_spec.md`
- `aspect_input/input_interaction_spec.md`
- `aspect_command/command_surface_interaction_spec.md`
- `aspect_control/settings_and_control_surfaces_spec.md`
- `workbench/workbench_frame_tile_interaction_spec.md`

### 8.6 Lifecycle Gate Integration (Current/Planned/Speculative)

Migration closure now requires two synchronized planning artifacts:

1. **Coverage matrix** (`2026-03-01_ux_migration_feature_spec_coverage_matrix.md`)
  for feature-to-spec ownership and three-tree completeness.
2. **Lifecycle register** (`2026-03-01_ux_migration_lifecycle_audit_register.md`)
  for delivery stage (`Current`/`Planned`/`Speculative`) and timing gates
  (pre/post renderer/WGPU, pre/post networking).

Feature work is considered migration-complete only when both artifacts reflect
the same closure state and UxTree/UxProbe/UxHarness readiness is explicitly
captured for the feature family.

---

## 9. Open Questions

1. **Gesture vs. click for Radial Palette Mode**: Should Radial Palette Mode support
   press-hold-flick gesture (marking menu style) in addition to
   click-to-open? Marking menus enable expert acceleration but add
   implementation complexity.

2. **Lasso on left vs. right button**: The current right-drag lasso conflicts
  with standard right-click contextual invocation expectations. Should lasso move to
   left-drag-on-empty-canvas? This trades discoverability for convention.

3. **Edge focus model**: How should edges participate in keyboard focus
   traversal? Options: (a) Tab from node to adjacent edges, then to connected
   nodes. (b) Edges only focusable via explicit edge-walk mode (`E` to
   enter edge focus, Tab to cycle). (c) Edges never have focus, only
   selectable via pointer or Command Palette.

4. **Readability metric integration depth**: Should readability metrics drive
   *automatic* layout adaptation or remain purely advisory/diagnostic?
   Automatic adaptation risks surprising the user; advisory mode risks being
   ignored.

5. **Constraint-based layout timeline**: SketchLay-style sketch-to-constraint
   is powerful but complex. Should it be a named Phase in the UX automation
   backlog (#251), or deferred beyond Phase 7?

6. **Facet rail default binding**: Should facet-rail entry use `F`, `Tab`
  within single-node mode, or automatic activation when one node is selected?

7. **Canvas terminology convergence timing**: How quickly should all canvas
  specs be updated to frame-affinity terminology where they currently discuss
  organizational clustering semantics?

---

## 10. References

1. WHATWG DOM Living Standard — https://dom.spec.whatwg.org/
2. Faceted Classification — https://en.wikipedia.org/wiki/Faceted_classification
3. Haleem, H., Wang, Y., Puri, A., Wadhwa, S., Qu, H. (2018). "Evaluating
   the Readability of Force Directed Graph Layouts: A Deep Learning Approach."
   arXiv:1808.00703v1
4. Force-Directed Graph Drawing — https://en.wikipedia.org/wiki/Force-directed_graph_drawing
5. SketchLay — https://github.com/nickoala/d3-panzoom (related project);
   Constraint-based graph layout via user sketching
6. Pie Menu / Radial Menu — https://en.wikipedia.org/wiki/Pie_menu
7. Fitts, P. M. (1954). "The information capacity of the human motor system
   in controlling the amplitude of movement." *Journal of Experimental
   Psychology*, 47(6), 381–391.
8. Zooming User Interface — https://en.wikipedia.org/wiki/Zooming_user_interface
9. Callahan, J., Hopkins, D., Weiser, M., Shneiderman, B. (1988). "An
   empirical comparison of pie vs. linear menus." *Proceedings of ACM CHI*.
10. Fruchterman, T. M. J., Reingold, E. M. (1991). "Graph Drawing by
    Force-Directed Placement." *Software: Practice and Experience*, 21(11),
    1129–1164.
11. Kamada, T., Kawai, S. (1989). "An algorithm for drawing general undirected
    graphs." *Information Processing Letters*, 31(1), 7–15.
12. Ranganathan, S. R. (1962). *Colon Classification*. 6th ed.

---

## 11. Disagreements and Conflict Log (Component-Owned)

This section records framing conflicts discovered during cross-spec review.
Each conflict is owned by the chief component responsible for resolution.

| Conflict | Competing framing | Owning component | Resolution target |
|---------|-------------------|------------------|-------------------|
| Right-click semantics | Some viewer/workbench docs still describe explicit context-menu flows, while command canonical docs retire Context Menu as first-class Graphshell surface | Command aspect | Keep command authority in `ActionRegistry`; treat embedder context menus as adapters that invoke contextual palette mode |
| Edge traversal interaction depth | Edge traversal spec permits edge traversal primary action; migration spec adds keyboard edge-walk model that may overfit power users | History + Canvas | Validate against UX research agenda and settle default edge-focus model via targeted tests |
| ~~Frame-first organization vs older zone terminology~~ | **Resolved 2026-03-23**: `MagneticZone` is deprecated as legacy alias; canonical model is `Frame` / `Frame membership` / `Frame-affinity region` per `workbench/graph_first_frame_semantics_spec.md`. `CloseFrameHandle` / `DeleteFrame` semantics and cross-tree membership sync are now explicit in `workbench/workbench_frame_tile_interaction_spec.md §2.4A` and the terminology lock section. | Workbench + Canvas | **Closed** (`#268`) |
| UxTree convergence ambition | Aspirational replacement of `egui_tiles` / `egui_graphs` responsibilities vs current two-authority contracts and near-term delivery constraints | UX Semantics subsystem | Stage as roadmap with explicit gates; keep C1–C5 + two-authority invariants intact during migration |
| Radial overflow UX | Current radial second-ring implementation can stack labels; command spec requires readable directional surface | Command aspect | Ship dedicated radial geometry/overflow spec and acceptance tests before declaring radial primary |

