# Hybrid Graph-View Management Plan (Overview Plane + Atlas)

**Date**: 2026-03-05
**Status**: Active planning draft
**Priority**: High (UX semantics)

**Related**:

- `multi_view_pane_spec.md`
- `2026-02-22_multi_graph_pane_plan.md`
- `graph_node_edge_interaction_spec.md`
- `../subsystem_ux_semantics/2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`

---

## 1. Goal

Define a practical UX model for graph-view management that keeps the "one canvas" feel without adding a second meta-graph truth layer.

This plan adopts a **hybrid** model:

1. **Overview Plane** for structural graph-view editing.
2. **Atlas Mini-Map** for continuous navigation and transfer during normal work.

Both surfaces route through the same reducer intents and state contracts.

---

## 2. Decision Summary

- Keep one `GraphId` as content truth.
- Keep multiple `GraphViewId` scopes with per-view camera and local layout/physics state.
- Do not introduce a persistent graph-of-graphs model in this slice.
- Treat "view as region" as UX semantics backed by view-scoped state, not a separate ontology.

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
- transfer selected nodes to destination region

### 4.2 Atlas Mini-Map (always available)

Primary operations:

- click region to focus view
- drag selected nodes onto a destination region
- observe occupancy/relationship hints at low visual complexity

Parity rule:

- Atlas transfer and Overview transfer must emit identical reducer intents.

---

## 5. Implementation Roadmap

### H0 - Decision Lock (docs only)

Goals:

- freeze ownership, transfer, overlap, and boundary rules
- define acceptance checks for UX parity

Done gates:

- [ ] decisions in Section 3 are copied into canonical spec language
- [ ] parity rule between Overview and Atlas is explicitly documented

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

- implement explicit Overview mode entry/exit
- support region manipulation affordances
- support transfer gesture from selected nodes to destination region

Done gates:

- [ ] command/shortcut opens and exits Overview mode deterministically
- [ ] region interactions produce reducer-authoritative intents only
- [ ] transfer gesture emits deterministic move intent

### H3 - Atlas UX

Goals:

- add always-available Atlas panel/surface
- support click-to-focus and drag-to-transfer
- keep intent parity with Overview operations

Done gates:

- [ ] Atlas click focuses target view deterministically
- [ ] Atlas drag transfer uses same reducer intent path as Overview
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
6. `H3.2`: Atlas drag transfer parity with Overview.
7. `H4.1`: diagnostics channels for region/transfer outcomes.
8. `H4.2`: keyboard and accessibility parity for hybrid surfaces.

---

## 7. Open Questions (resolve before H2)

1. Should transfer animation be purely decorative or tied to reducer completion timing?
2. How should cross-view edge hints be rendered in Atlas at high density?
3. Should region auto-placement prefer nearest free slot or first deterministic scan order?
4. Should archived regions be visible in Atlas by default or behind a filter toggle?
