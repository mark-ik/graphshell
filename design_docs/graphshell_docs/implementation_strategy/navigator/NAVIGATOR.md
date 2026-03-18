<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# NAVIGATOR — Domain Spec

**Date**: 2026-03-17
**Status**: Canonical / Active
**Scope**: Navigator as a first-class domain with its own authority boundary,
projection rules, and interaction contract.

**Related**:

- [navigator_backlog_pack.md](navigator_backlog_pack.md) — dependency-ordered implementation backlog
- [navigator_interaction_contract.md](navigator_interaction_contract.md) — click grammar, selection, reveal, dismiss
- [navigator_projection_spec.md](navigator_projection_spec.md) — section model, projection sources, refresh triggers
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — workbench domain (arrangement and activation authority)
- [../canvas/CANVAS.md](../canvas/CANVAS.md) — graph canvas domain (truth and context authority)
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical term definitions

---

## 1. What the Navigator Is

The Navigator is a **projection surface**.

It reads from graph truth and workbench arrangement state. It does not own
either. It presents a structured, navigable view of the objects and arrangements
the user is working with, and it routes user interactions back to the correct
authority.

The Navigator is not a second data model. It is not a sidebar version of the
workbench. It is a view — one that the user can use to locate, select, activate,
and route graph-backed objects without needing to find them on the graph canvas
or open the correct workbench frame first.

---

## 2. Why the Navigator Is Its Own Domain

The Navigator was previously documented under the Workbench domain. That was
accurate when the sidebar was primarily a tree of workbench arrangement state
(frames, tiles, panes). It is no longer accurate.

The Navigator's job now spans:

- **graph truth** — node identity, relations, provenance, tags, recency
- **workbench arrangement state** — which nodes are open, in which frames,
  in which tiles
- **projection logic** — what gets shown, in what sections, in what order
- **interaction routing** — how clicks, selections, and commands route back
  to the correct authority

No single existing domain owns all four of those. Placing the Navigator spec
under Workbench misleads: it implies the Navigator's source of truth is
arrangement state, when in fact it reads from both graph truth and arrangement
state and must never conflate them.

The Navigator therefore gets its own domain directory and canonical spec.

---

## 3. What the Navigator Owns

The Navigator domain owns:

- **Projection rules** — which objects appear in which sections, under what
  conditions, in what order
- **Section model** — the named sections (Recent, Frames, Graph, Relations,
  etc.) and their projection sources
- **Interaction contract** — click grammar (single-click = select, double-click
  = activate), reveal rules, dismiss routing, command applicability
- **Selection propagation** — how Navigator interactions set and read global
  graph selection truth
- **Expansion/collapse state** — session-scoped per-row expansion state
  (not persisted as graph truth)
- **Filter/search model** — local filter semantics that do not mutate
  underlying truth
- **Refresh triggers** — which graph and workbench state changes cause the
  Navigator projection to rebuild or update

---

## 4. What the Navigator Does Not Own

The Navigator explicitly does not own:

- **Node identity or graph structure** — owned by graph truth (graph domain)
- **Node lifecycle** (active / warm / cold) — owned by the runtime lifecycle
  subsystem
- **Tile tree structure or frame layout** — owned by workbench arrangement
- **Which node is open in which pane** — owned by workbench session state
- **Routing decisions** (which pane to open a node in) — owned by workbench
  routing
- **Persist/delete operations** — the Navigator may route these intents;
  it does not execute them

When the Navigator initiates an action (select, activate, dismiss), it emits
a graph intent or workbench intent and trusts the relevant authority to execute
it. The Navigator does not directly mutate graph state or workbench arrangement
state.

---

## 5. The Three-Domain Model

These three domains form the coherent application model:

| Domain | Is | Owns | Does Not Own |
|--------|----|------|--------------|
| **Graph** | Truth | Node identity, relations, provenance, durable state | Where or how nodes are displayed |
| **Workbench** | Arrangement and activation | Tile tree, frame layout, pane lifecycle, routing | What a node is or what its graph relations mean |
| **Navigator** | Projection and control | Projection rules, section model, interaction contract, selection propagation | Node identity, arrangement structure, routing execution |

A node is one durable object. All three domains agree on what that object is.
The Navigator shows it, the Workbench hosts it, the Graph stores it.

---

## 6. Interaction Invariants

These invariants hold across graph canvas, workbench, and navigator. If any
surface violates one, it is a bug, not a design choice.

### I1 — Identity invariant
A node is one object with one identity. Selecting, activating, or dismissing
it on any surface targets the same underlying node. There is no "graph copy"
and "navigator copy."

### I2 — State separation invariant
Existence, visibility, selection, and activation are four distinct states.
None implies another except by explicit intent:
- A node can exist without being visible in the current graph view.
- It can be visible without being selected.
- It can be selected without being activated (open in a pane).
- It can be activated without being the primary selection.

### I3 — Selection propagation invariant
Selecting a node on any surface sets graph selection truth. Surfaces project
that truth — they do not own their own selection copy. If the Navigator and
the graph canvas show different nodes as selected, one of them is wrong.

### I4 — Click grammar invariant
Across all surfaces:
- **Single-click** = select / give focus
- **Double-click** = activate / open
- This applies to node rows in the Navigator, node objects on the graph canvas,
  and node tabs in workbench tiles.
- Structural rows (Frame, Tile, Section) are not nodes; single-click
  expands/collapses them.

### I5 — Reveal invariant
Reveal (scrolling the graph canvas to show a node) is a side effect of
selection, not a guaranteed consequence. Reveal happens only when:
- the graph canvas is currently visible, and
- the selected node is outside the current viewport.

Reveal does not change selection, activation, or graph structure.

### I6 — Dismiss / delete invariant
- **Dismiss** = remove a node from its current surface context (tile, frame,
  view). The node still exists. Dismiss is recoverable.
- **Delete** = remove the node from graph truth. The node no longer exists.
  Delete is not recoverable without undo.

The Navigator may offer dismiss actions for nodes in arrangement contexts.
It must not offer delete as an equivalent or fallback to dismiss.

### I7 — Command applicability invariant
A command is available only if it validly applies to every object in the
current selection set. The Navigator must not silently narrow the target
to a subset or fall back to a single implicit primary target.

---

## 7. Section Model (Canonical)

Each Navigator section has a single projection source. Sections do not share
truth — a node may appear in multiple sections, but each appearance is
independently derived from its section's source.

| Section | Projection source | Entry condition | Exit condition |
|---------|------------------|-----------------|----------------|
| **Recent** | Graph recency index | Node becomes cold / leaves active tile context | Node is promoted into active tile context |
| **Frames** | Workbench arrangement state | Node is a member of at least one named frame | All frame memberships removed |
| **Graph** | Graph node set (filtered) | Node exists in graph | Node deleted |
| **Relations** | Graph relation families | Relation family has visible members | No visible members |
| **Import Records** | Import record index | Import record exists | Record deleted or suppressed |

Section membership is always derived. The Navigator never stores section
membership as its own truth.

---

## 8. Projection Sources and Authority

| Navigator reads from | Authority | How |
|---------------------|-----------|-----|
| Node identity, tags, relations | Graph domain | Read from `domain_graph()` |
| Node recency / lifecycle state | Runtime lifecycle | Read from `graph_runtime` state |
| Frame membership | Workbench session state | Read from `WorkbenchSessionState::node_workspace_membership` |
| Active tile contents | Workbench session state | Read from tile tree at projection time |
| Import records | Import record index | Read from domain state |

The Navigator reads these at projection time. It does not cache copies of
graph or workbench state independently — stale projection is diagnosed, not
silently tolerated.

---

## 9. Refresh Triggers

The Navigator projection rebuilds or updates when:

- A node is added to or removed from the graph
- A node's title, tags, or relations change
- A node's lifecycle state changes (active / warm / cold)
- Frame membership changes for any node
- The tile tree changes (pane open, close, move, split)
- An import record is added, deleted, or suppressed
- The user applies a local filter or search query

Refresh is routed through the shared signal path (`phase3_publish_workbench_projection_refresh_requested`), not through ad hoc observers.

---

## 10. Relationship to the Workbench Domain

The Workbench domain previously claimed the "Workbench Sidebar" as its own.
That claim is superseded by this spec.

Updated boundary:
- **Workbench** owns the sidebar's *chrome container* (the layout slot, its
  resize handle, its show/hide toggle) and the routing decisions that happen
  when a Navigator action requires opening a pane.
- **Navigator** owns everything inside the sidebar content area: the sections,
  the rows, the click grammar, the selection propagation, the projection rules.

This is the same relationship as graph canvas / graph domain: the workbench
chrome hosts the graph view slot; the graph domain owns what renders inside it.

The `WORKBENCH.md` sidebar ownership claim is updated to reflect this split.
