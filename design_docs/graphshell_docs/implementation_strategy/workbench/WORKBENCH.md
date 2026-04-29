# WORKBENCH — Domain Spec

**Date**: 2026-03-25
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` — chrome visibility authority: WorkbenchLayerState, ChromeExposurePolicy, and graph/workbench Navigator scope exposure
- `../canvas/2026-03-14_graph_relation_families.md` — ArrangementRelation as graph-edge backing for frame/tile-group membership
- `../navigator/NAVIGATOR.md` — Navigator domain spec (sidebar content authority; see §2 boundary note)
- `../canvas/frame_graph_representation_spec.md` — how Frames render as spatial minimap bounding boxes on the graph canvas

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §3.5)):

- **WCAG 2.2 Level AA** — tile/pane interactive elements must meet SC 2.5.8 minimum target size; focus order within the tile tree must follow SC 2.4.3; focus appearance must meet SC 2.4.11

---

## 1. Purpose

This note defines the **Workbench** as an architectural domain of Graphshell.

It exists to make one boundary explicit:

- the workbench tile tree is a presentation and arrangement subsystem,
- not a graph-content subsystem,
- and not the owner of graph meaning.

### Status update (2026-03-18)

Recent runtime/workbench alignment landed in code:

- Navigator projection carriers now use canonical `Navigator*` naming at the app/runtime boundary.
  Legacy `FileTree*` intent variants and adapters were removed from active intent paths.
- Containment projection rows are graph-backed from `ContainmentRelation` edges (not ad hoc URL-only projection).
  Derived containment relations are refreshed on node-set and URL-change deltas.
- Node-pane workbench surfaces now expose collapsible per-node `Node History` and `Node Audit`
  sections, backed by history-query helpers.

These changes tighten the Workbench/Canvas boundary: Workbench hosts and projects,
while graph/history truth stays in graph + persistence carriers.

---

## 2. What The Workbench Domain Feature Area Owns

The Workbench owns arrangement interaction/session mutation truth and presentation hosting **within one graph dataset (`GraphId`)**:

- the tile tree for one workbench instance
- split geometry within that tile tree
- tab / tile ordering within that tile tree
- pane lifecycle (open, close, promote, demote, dismiss)
- destination selection after routing is requested
- visible arrangement context within the workbench
- workbench-level focus handoff between tiles
- workbench-owned host layout chrome for Navigator surfaces (host bounds, resize handles, show/hide toggles), while Navigator content grammar remains owned by the Navigator domain; see `../navigator/NAVIGATOR.md`
- graph-bearing pane hosting for graph surfaces that participate in arrangement flow without changing Graph authority

The Workbench is the canonical owner of where content is shown and how session
arrangement is structurally realized **inside one workbench**.

It is not the owner of graph identity, graph topology, graph semantic truth, or viewer backend policy.

**Frame composition is not Workbench authority.** A Workbench is a *component
of* a Frame, not the owner of one. The Shell composes one or more Workbenches
into a Frame and owns frame switching, frame persistence, and the frametree.
The Workbench provides the per-graph tile tree that lives inside a Frame slot;
it does not decide which workbenches share a frame, how frames are switched,
or how frame snapshots persist. See [`../shell/SHELL.md §3`](../shell/SHELL.md)
and [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) Frame entry.

**Chrome visibility** is governed by `WorkbenchLayerState` (`GraphOnly`, `GraphOverlayActive`, `WorkbenchActive`, `WorkbenchPinned`) — a derived state machine computed each frame. Navigator hosts may expose graph scope, workbench scope, or both depending on host configuration and the active layer state. Workbench-owned chrome determines when each host is visible and how much edge space it occupies, while Navigator semantics determine what each visible host projects. See `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §7–8` and `../navigator/NAVIGATOR.md §12`.

**Frame membership** is graph-backed: frame/tile-group membership is stored as `ArrangementRelation` edges in the graph, not as workbench-only data structures. The workbench reads these edges to render the tile tree and navigator. Mutating durable frame membership emits `GraphIntent`s that assert or retract `ArrangementRelation` edges, while session-only tile/split structure remains under workbench mutation authority until promoted. See `canvas/2026-03-14_graph_relation_families.md §2.4`.

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` are not workbench-only settings.

They are a **cross-domain policy layer** that can influence:

- layout behavior
- physics behavior

Within the Workbench, these presets may influence:

- workbench motion feel
- transition aggressiveness
- layout stabilization expectations
- how strongly the UI favors adaptive movement versus deterministic placement

These presets must not directly control graph camera policy from Workbench semantics.
Camera lock/fit behavior remains owned by the graph camera policy surface.

The Workbench must consume these presets as Graphshell policy, not as framework-defined behavior.

That means:

- layout frameworks may compute geometry,
- but Graphshell decides how domain policy affects workbench semantics.

---

## 4. Ownership Mapping

### 4.1 Canonical Workbench-owned state

- tile tree structure within one workbench
- active tile / pane within that workbench
- pane open / close / split / reorder state
- workbench-level focus transitions between tiles

### 4.2 State Workbench does not own

- node identity
- edge identity
- graph topology
- graph selection truth
- graph semantic camera target meaning
- graphlet truth or graphlet derivation rules
- **frame composition** (which workbenches participate in a frame)
- **active frame** and frame switching
- **frame snapshot** persistence

Graph and Navigator domains own the first six. Shell owns the last three. The
Workbench may *participate* in a Frame as one of its composed workbenches and
may *render* the active frame's tile tree, but it does not decide composition.

---

## 5. Bridges To Other Areas

The Workbench interacts with other domains, aspects, and subsystems through explicit bridges.

### 5.1 Workbench <- Graph bridge

Used when graph content needs a presentation destination.

Examples:

- open node in pane
- focus existing node presentation
- route a graph action into a tile or frame

The Graph domain provides semantic routing intent.
The Workbench decides and maintains the destination structure.

### 5.2 Workbench -> Viewer bridge

Used when a pane needs a concrete rendering surface.

Examples:

- node viewer
- tool pane
- settings surface
- history surface

The Workbench hosts the pane.
The Viewer renders the pane content.

### 5.3 Policy -> Workbench bridge

Used when cross-domain presets influence workbench behavior.

Examples:

- `Liquid`: more adaptive motion and softer arrangement feel
- `Gas`: looser, more user-driven rearrangement feel
- `Solid`: deterministic placement and minimal passive movement

The policy layer supplies defaults.
The Workbench applies them to arrangement behavior and focus/transition policy.

---

## 6. Architectural Rules

- The Workbench must never redefine graph identity.
- Closing or moving a tile must not delete or mutate graph truth.
- The Graph subsystem may request where content should go, but the Workbench owns how that destination is structurally realized.
- A graph-bearing pane hosted by Workbench is still Graph-owned in truth and Navigator-owned in projection intent where applicable.
- Framework layout crates may compute geometry, but they must not become the semantic owner of routing, focus, or lifecycle policy.

---

## 7. The Five-Domain Model

Workbench is one of five application-level domains:

| Domain | Is | Owns |
|--------|----|------|
| **Shell** | Host + app-level control | top-level composition, **frame composition / switching / persistence**, command/control surfaces, ambient status |
| **Graph** | Truth + analysis + management | node/edge identity, graph-space interaction, graph analysis |
| **Navigator** | Projection + navigation | graphlet derivation, scoped search, breadcrumb/context projection |
| **Workbench** | Arrangement + activation | panes, splits, tab strips, overlays, staging — within one tile tree |
| **Viewer** | Realization | backend choice, fallback policy, render strategy |

Workbench makes detailed work structurally explicit. It is not the universal substrate of the app.

---

## 8. Practical Reading

If a behavior answers:

- where content appears within one workbench's tile tree,
- how panes are arranged inside a workbench,
- which tile is active within the workbench,
- how work is structurally organized on screen *inside one workbench*,

it belongs primarily to the **Workbench**.

If a behavior answers which workbenches compose the current working context,
which frame is active, how frames are switched, or how a frame snapshot
persists and restores, it belongs to the **Shell** — see
[`../shell/SHELL.md`](../shell/SHELL.md).
