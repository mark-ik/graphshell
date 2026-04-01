# Hybrid Graph-View Management Plan (Graph Overview Plane + Navigator Atlas)

**Date**: 2026-03-05
**Status**: Active planning draft (narrowed 2026-04-01)
**Priority**: High (UX semantics)

**Related**:

- `multi_view_pane_spec.md`
- `../navigator/NAVIGATOR.md`
- `../core-interaction-model-plan.md`
- `graph_node_edge_interaction_spec.md`
- `../subsystem_ux_semantics/2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`

---

## 1. Goal

Define a practical UX model for graph-view management that keeps the "one canvas" feel without adding a second meta-graph truth layer.

This plan adopts a **hybrid** model:

1. **Graph Overview Plane** for structural graph-view editing.
2. **Navigator Atlas / overview swatch** for continuous orientation during normal work.

Both surfaces route through the same reducer intents and state contracts.

---

## 1A. Critical refinement (2026-04-01)

The original plan was directionally right but too loose about ownership.

Refined stance:

- The **Overview Plane** remains graph-owned and is the explicit authoring surface for graph-view regions.
- The **Atlas** is not a second graph manager. It is a Navigator-hosted orientation surface that may project graph views as swatches, lists, or strips depending on host form factor.
- **Sidebar** Navigator hosts may render a true overview swatch or minimap-like card when there is enough space.
- **Toolbar** Navigator hosts should degrade to compact chips, strips, and counts rather than forcing a tiny canvas that is too cramped for reliable manipulation.
- **Freefloating Navigator hosts are deferred**. The current host model is edge-mounted; a floating Navigator would require a separate Shell host-model expansion instead of being smuggled into this plan.
- "Zoom out to overview" remains the idiomatic graph-domain control for direct graph-view management. Navigator may help orient and route, but it must not become the only place where graph-view structure can be authored.

First-shipping posture:

- Build **H2 before H3**. The graph-owned Overview Plane must exist before Navigator attempts drag-transfer or structural graph-view management affordances.
- The **first sidebar-host Atlas ships list-first**, with graph-view/session lists as the primary surface and an optional overview swatch card added only once host sizing is stable.
- **Archived graph views are hidden by default** in compact Navigator surfaces and appear behind an explicit filter toggle.
- **High-density cross-view hints are aggregated**, using counts, occupancy badges, or subtle adjacency indicators instead of drawing detailed per-edge spaghetti in the Atlas.
- **Transfer animation is decorative only** and must never become the state-authoritative signal; reducer completion remains the authoritative outcome.
- **Auto-placement must prefer deterministic scan order** over nearest-slot heuristics for the initial slice.

---

## 2. Decision Summary

- Keep one `GraphId` as content truth.
- Keep multiple `GraphViewId` scopes with per-view camera and local layout/physics state.
- Do not introduce a persistent graph-of-graphs model in this slice.
- Treat "view as region" as UX semantics backed by view-scoped state, not a separate ontology.
- Split responsibilities cleanly: graph owns region authoring; Navigator owns ambient orientation and cross-surface summary.

Rationale:

- Delivers the intended mental model now, with lower architecture risk.
- Keeps reducer and persistence paths deterministic.
- Preserves an upgrade path to projection/copy/meta modes later.

---

## 3. Contract Decisions (Phase 0 lock)

### 3.1 Ownership

- Each node has exactly one owning `GraphViewId` in this phase.
- Multi-view projection is deferred.

### 3.2 Cross-view edges

- Cross-view edges are allowed at graph truth level.
- Rendering policy is constrained:
  - local pane: no heavy inter-view clutter by default
  - overview/atlas: show inter-view links in simplified form

### 3.3 Transfer semantics

- First-class transfer operation is `Move`.
- `Copy` and `Project` are deferred.

### 3.4 Boundary behavior

- View ownership boundaries are hard for default simulation.
- No implicit drift of node ownership across views.

### 3.5 Region overlap policy

- Overview Plane regions are non-overlapping.
- Move/resize conflicts resolve deterministically (reject or auto-place, but never silent overlap).

---

## 4. UX Surface Model

### 4.1 Overview Plane (explicit mode)

Entry is explicit (command/shortcut), not accidental zoom threshold crossing.

Primary operations:

- create region
- move region
- resize region
- rename region
- archive / restore region
- transfer selected nodes to destination region

This is the authoritative direct-manipulation surface for graph-view structure.
If a gesture changes region geometry, ownership, or archival state, this is the
surface that should own the primary interaction.

### 4.2 Navigator Atlas / Overview Swatch (host-dependent)

The atlas concept survives, but as a Navigator-hosted orientation surface
rather than as a second graph-management plane.

Primary operations:

- click region to focus view
- reveal the active graphlet, graph view, or selected node in a compact context map
- observe occupancy/relationship hints at low visual complexity
- in spacious hosts only, drag selected nodes onto a destination region

Form-factor policy:

- Sidebar hosts may render a minimap-like swatch plus adjacent lists for graph views, graphlets, nodes, and workbench sessions.
- Toolbar hosts should default to a compact graph-view strip or tab-like representation of workbench sessions and graph-view targets.
- Toolbar hosts should not be required to support precision drag-transfer across a tiny minimap.

First release recommendation:

- Ship the sidebar form as **list-first** with an optional swatch card gated by a host-width threshold.
- Treat the swatch as an orientation aid, not the sole information carrier; session strips, graph-view lists, and selected-node context should remain legible without it.
- Keep toolbar Atlas surfaces non-gestural in the first slice beyond focus/routing actions.

Parity rule:

- Any transfer initiated from Navigator Atlas must emit identical reducer intents to Overview Plane transfer.

Non-goal:

- Atlas does not become a second persisted region model or an alternate source of truth for graph-view layout.

---

## 5. Implementation Roadmap

### H0 - Decision Lock (docs only)

Goals:

- freeze ownership, transfer, overlap, and boundary rules
- define acceptance checks for UX parity

Done gates:

- [x] decisions in Section 3 are copied into canonical spec language — ownership rules (§3.1–3.4) are reflected in `multi_view_pane_spec.md §§3–6` (GraphViewId, per-view layout, slot lifecycle, routing semantics). Overlap policy (§3.5) is reflected in `multi_view_pane_spec.md §5.2` slot coordinate collision guard.
- [x] overview-plane vs. Navigator-Atlas role split and transfer parity are explicitly documented in canonical specs (`multi_view_pane_spec.md §5.3A` and `../navigator/NAVIGATOR.md §11.7`)

### H1 - Reducer Contracts and State

Goals:

- formalize/extend region state (`GraphViewSlot` and related layout metadata)
- formalize transfer intent contract for node move between views
- enforce non-overlap invariants

Done gates:

- [ ] reducer tests cover create/move/resize/archive/restore and overlap guardrails
- [ ] reducer tests cover node transfer move semantics
- [ ] persistence round-trip covers region layout manager state

### H2 - Overview Plane UX

Goals:

- implement explicit graph-owned Overview mode entry/exit
- support region manipulation affordances
- support transfer gesture from selected nodes to destination region

Done gates:

- [ ] command/shortcut opens and exits Overview mode deterministically
- [ ] region interactions produce reducer-authoritative intents only
- [ ] transfer gesture emits deterministic move intent

### H3 - Atlas UX

Goals:

- add Navigator-hosted Atlas/overview surface
- ship a list-first sidebar Atlas and compact toolbar degradation before attempting a swatch-heavy variant
- support click-to-focus everywhere and drag-to-transfer only where host geometry permits it
- keep intent parity with Overview operations

Done gates:

- [ ] Navigator Atlas click focuses target view deterministically
- [ ] first sidebar Atlas ships as list-first with optional swatch-card gating instead of requiring a minimap-first layout
- [ ] sidebar-host Atlas drag transfer uses same reducer intent path as Overview
- [ ] toolbar-host Atlas degrades to non-minimap list/strip semantics without losing routing parity
- [ ] archived graph views are hidden by default and surfaced through an explicit filter toggle in compact Navigator contexts
- [ ] high-density cross-view relationships degrade to aggregated hints rather than detailed inter-view edge rendering
- [ ] focused-view transitions are test-covered

### H4 - Diagnostics and Accessibility

Goals:

- expose region/transfer diagnostics channels
- provide keyboard-equivalent management and transfer flows
- keep visual feedback non-color-dependent

Done gates:

- [ ] diagnostics channels exist for region mutation and transfer outcomes
- [ ] keyboard path exists for core region + transfer actions
- [ ] regression coverage for disabled-state reason text and focus traversal

### H5 - Deferred Extensions

Deferred (not in this plan slice):

- projection mode (`Project`)
- copy mode (`Copy`)
- true graph-of-graphs browse mode

Done gates:

- [ ] deferred backlog items are tracked with explicit non-goals for current milestone

---

## 6. Issue-Sized Slice Templates

1. `H1.1`: region overlap guardrails and deterministic conflict handling.
2. `H1.2`: node transfer move contract and reducer tests.
3. `H2.1`: explicit Overview mode toggle and state wiring.
4. `H2.2`: region drag/resize affordances routed via intents.
5. `H3.1`: Atlas focus routing parity with Overview.
6. `H3.2`: sidebar Atlas drag transfer parity with Overview and toolbar degradation rules.
7. `H4.1`: diagnostics channels for region/transfer outcomes.
8. `H4.2`: keyboard and accessibility parity for hybrid surfaces.

---

## 7. Open Questions (resolve before H2)

Resolved for the first shipping slice:

1. Transfer animation is purely decorative and may follow reducer completion, but must not gate or define success.
2. High-density cross-view hints should use aggregated counts, occupancy, or adjacency indicators rather than detailed edge-line rendering.
3. Region auto-placement should use deterministic scan order first.
4. Archived regions should be hidden by default in Atlas and appear behind a filter toggle.
5. The first sidebar-host Atlas should ship as a list-first Navigator section with an optional swatch card once host sizing is stable.

Still open after H2/H3 entry:

1. Should aggregated cross-view hints differentiate semantic families, or remain view-level only in the first Atlas slice?
2. What host-width threshold should enable the optional swatch card without destabilizing Navigator layout density?
