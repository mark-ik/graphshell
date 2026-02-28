# WORKBENCH — Layout Domain Feature Area

**Date**: 2026-02-28
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `workbench_frame_tile_interaction_spec.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`

---

## 1. Purpose

This note defines the **Workbench** as an architectural subsystem of Graphshell.

It exists to make one boundary explicit:

- the workbench tile tree is a presentation and arrangement subsystem,
- not a graph-content subsystem,
- and not the owner of graph meaning.

---

## 2. What The Workbench Domain Feature Area Owns

The Workbench owns arrangement truth and presentation hosting:

- the tile tree
- frame branches and frame selection
- split geometry
- tab / tile ordering
- pane lifecycle
- destination selection after routing is requested
- visible arrangement context
- workbench-level focus handoff

The Workbench is the canonical owner of where content is shown.

It is not the owner of graph identity, graph topology, or graph semantic truth.

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` are not workbench-only settings.

They are a **cross-domain policy layer** that can influence:

- camera behavior
- layout behavior
- physics behavior

Within the Workbench, these presets may influence:

- workbench motion feel
- transition aggressiveness
- layout stabilization expectations
- how strongly the UI favors adaptive movement versus deterministic placement

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
- frame membership of presentation surfaces
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
