# WORKBENCH — Layout Domain Feature Area

**Date**: 2026-02-28
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `workbench_frame_tile_interaction_spec.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` — chrome split authority: WorkbenchLayerState, ChromeExposurePolicy, Graph Bar vs Workbench Sidebar
- `../canvas/2026-03-14_graph_relation_families.md` — ArrangementRelation as graph-edge backing for frame/tile-group membership
- `navigator_graph_isomorphism_spec.md` — canonical single-click/double-click isomorphism between Navigator rows and graph canvas nodes
- `../canvas/frame_graph_representation_spec.md` — how Frames render as spatial minimap bounding boxes on the graph canvas

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §3.5)):
- **WCAG 2.2 Level AA** — tile/pane interactive elements must meet SC 2.5.8 minimum target size; focus order within the tile tree must follow SC 2.4.3; focus appearance must meet SC 2.4.11

---

## 1. Purpose

This note defines the **Workbench** as an architectural subsystem of Graphshell.

It exists to make one boundary explicit:

- the workbench tile tree is a presentation and arrangement subsystem,
- not a graph-content subsystem,
- and not the owner of graph meaning.

---

## 2. What The Workbench Domain Feature Area Owns

The Workbench owns arrangement interaction/session mutation truth and presentation hosting:

- the tile tree
- frame branches and frame selection
- split geometry
- tab / tile ordering
- pane lifecycle
- destination selection after routing is requested
- visible arrangement context
- workbench-level focus handoff
- **Workbench Sidebar** (navigator, viewer controls, pane tree) — see `2026-03-13_chrome_scope_split_plan.md`

The Workbench is the canonical owner of where content is shown and how session
arrangement is structurally realized.

It is not the owner of graph identity, graph topology, or graph semantic truth.

**Chrome visibility** is governed by `WorkbenchLayerState` (`GraphOnly`, `GraphOverlayActive`, `WorkbenchActive`, `WorkbenchPinned`) — a derived state machine computed each frame. The Workbench Sidebar is visible only when the state is `WorkbenchActive` or `WorkbenchPinned`. The **Graph Bar** (search, lens chips, zoom controls) is separate from the Workbench Sidebar and persists across all states. See `subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md §7–8`.

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

- tile tree structure
- active frame
- active tile / pane
- frame membership of presentation surfaces as an interactive/session arrangement concern
- pane open / close / split / reorder state
- workbench-level focus transitions

### 4.2 State Workbench does not own

- node identity
- edge identity
- graph topology
- graph selection truth
- graph semantic camera target meaning

Those belong to the Canvas domain feature area.

---

## 5. Bridges To Other Areas

The Workbench interacts with other domains, aspects, and subsystems through explicit bridges.

### 5.1 Workbench <- Graph bridge

Used when graph content needs a presentation destination.

Examples:

- open node in pane
- focus existing node presentation
- route a graph action into a tile or frame

The Canvas provides semantic routing intent.
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
- Framework layout crates may compute geometry, but they must not become the semantic owner of routing, focus, or lifecycle policy.

---

## 7. Practical Reading

If a behavior answers:

- where content appears,
- how panes are arranged,
- which frame or tile is active,
- how work is structurally organized on screen,

it belongs primarily to the **Workbench**.
