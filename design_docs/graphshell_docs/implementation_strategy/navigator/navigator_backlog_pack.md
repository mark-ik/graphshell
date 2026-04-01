<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Navigator Backlog Pack

**Date**: 2026-03-17
**Status**: Planning / handoff pack
**Scope**: Dependency-ordered backlog for Navigator as a projection surface over
graph truth and workbench arrangement state.

**Related docs**:

- [NAVIGATOR.md](NAVIGATOR.md) — Navigator domain spec and authority boundaries
- [navigator_interaction_contract.md](navigator_interaction_contract.md) — canonical Navigator click grammar and graph/workbench resolution rules
- [../../technical_architecture/graphlet_model.md](../../technical_architecture/graphlet_model.md) — canonical graphlet semantics consumed by Navigator projection
- [../../technical_architecture/domain_interaction_scenarios.md](../../technical_architecture/domain_interaction_scenarios.md) — canonical cross-domain scenario IDs (`DI01`-`DI06`)
- [../domain_interaction_acceptance_matrix.md](../domain_interaction_acceptance_matrix.md) — compact review matrix for cross-domain scenario evidence
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — workbench domain ownership and arrangement authority
- [../workbench/graph_first_frame_semantics_spec.md](../workbench/graph_first_frame_semantics_spec.md) — graph/workbench frame semantics
- [../graph/graph_backlog_pack.md](../graph/graph_backlog_pack.md) — graph-side backlog dependencies used by Navigator projections

## Tracker mapping

- Hub issue: #306 (`Hub: five-domain architecture adoption — Shell host, graphlet model, cross-domain scenarios`)
- Primary implementation issue: #304 (`Adopt the canonical graphlet model across Navigator and Workbench`)
- Review/evidence issue: #305 (`Operationalize cross-domain scenario IDs and acceptance evidence`)

---

## Landed Delta (2026-03-18)

Confirmed slices now landed in code:

- ✅ Navigator intent surface canonicalization:
     active reducer/view-action paths use `SetNavigator*` and
     `RebuildNavigatorProjection`; legacy `SetFileTree*` and
     `RebuildFileTreeProjection` paths are removed.
     Advances: `NV01`, `NV06`.
- ✅ Containment projection source is now graph-backed from
     `ContainmentRelation` edges.
     Advances: `NV04`, `NV18`.
- ✅ Projection refresh now triggers from graph deltas affecting containment
     (node add/remove and URL updates).
     Advances: `NV10`.

These deltas are partial closures and should be referenced when preparing the
full milestone closure receipt (`NV25`).

---

## Wave 1

1. `NV01` Navigator Projection Boundary. Depends: none. Done gate: one canonical doc defines Navigator as a projection surface, not its own truth store.
2. `NV02` Navigator Row-Type Inventory. Depends: `NV01`. Done gate: every row type is listed and tagged as node, frame, tile, section, family, or derived structural row.
3. `NV03` Navigator Row Identity Contract. Depends: `NV02`. Done gate: every Navigator row has a stable identity source and no ad hoc ephemeral row IDs remain.
4. `NV04` Navigator Section Mapping Audit. Depends: `NV01`, `NV02`. Done gate: each section is mapped to graph truth, relation-family projection, workbench projection, or recency projection.
5. `NV05` Navigator Expansion State Contract. Depends: `NV03`, `NV04`. Done gate: expansion/collapse state is defined as projection/session state with persistence rules.
6. `NV06` Navigator Click Grammar Lock. Depends: `NV02`, `NV04`. Done gate: row-type-specific single-click and double-click behavior is canonical.
7. `NV07` Navigator Selection Contract. Depends: `NV03`, `NV06`. Done gate: selection behavior maps cleanly onto the global mixed-selection model.
8. `NV08` Navigator Residency-Aware Navigation Contract. Depends: `NV06`, `NV07`. Done gate: double-click routes differently for live versus cold nodes and is documented in carrier terms.
9. `NV09` Navigator Structural Row Focus Rule. Depends: `NV06`, `NV07`. Done gate: structural rows expand/collapse and become command focus without pretending to be nodes.
10. `NV10` Navigator Projection Refresh Triggers. Depends: `NV01`, `NV04`. Done gate: graph, workbench, and state changes that refresh Navigator are routed through shared signals, not ad hoc observers.

## Wave 2

1. `NV11` Navigator Reveal Rule. Depends: `NV07`, `NV08`. Done gate: reveal-on-select only happens when the graph is visible and the selected node is offscreen.
2. `NV12` Navigator Multi-Selection Semantics. Depends: `NV07`. Done gate: `Ctrl+Click`, range/toggle behavior, and row focus rules are explicit.
3. `NV13` Navigator Command Applicability Audit. Depends: `NV07`, `NV12`. Done gate: navigator-invoked commands obey the same "valid for every selected object" rule.
4. `NV14` Navigator Recents Contract. Depends: `NV04`, `NV08`. Done gate: `Recent` is a recency-sorted projection with clear entry/exit semantics.
5. `NV15` Navigator Arrangement Projection Contract. Depends: `NV04`, `NV09`. Done gate: frames, tiles, and canonically defined graphlets project consistently as expandable arrangement objects.
6. `NV16` Navigator Search / Filter Model. Depends: `NV04`, `NV10`. Done gate: local filter/search semantics are explicit and do not mutate underlying truth.
7. `NV17` Navigator Dismiss / Open Actions Audit. Depends: `NV06`, `NV13`. Done gate: Navigator row actions route through graph/workbench carriers instead of direct callsites.
8. `NV18` Navigator Edge / Relation Surfacing Policy. Depends: `NV04`, `NV15`. Done gate: relation families shown in Navigator are explicit and tied to shared graph relation-family rules.
9. `NV19` Navigator Empty / Degraded State Contract. Depends: `NV10`, `NV16`. Done gate: no-data, filtered-empty, and projection-error states are distinct and diagnosable.
10. `NV20` Navigator Accessibility and Keyboard Model. Depends: `NV03`, `NV06`, `NV12`. Done gate: keyboard navigation, row focus, expand/collapse, and activate are specified.

## Wave 3

1. `NV21` Navigator Diagnostics Pack. Depends: `NV10`, `NV19`, `NV20`. Done gate: invalid row targets, stale projection entries, and routing failures emit diagnostics.
2. `NV22` Navigator Scenario Test Matrix. Depends: `NV06`-`NV21`. Done gate: tests/spec scenarios cover row-type clicks, selection, reveal, recents, search/filter, and arrangement rows.
3. `NV23` Workbench-Navigator Contract Sync Pass. Depends: `WB25`, `NV22`. Done gate: workbench pane/focus semantics and Navigator projection semantics no longer contradict each other.
4. `NV24` Graph-Navigator Contract Sync Pass. Depends: `NV04`, `NV18`, graph backlog equivalents. Done gate: Navigator sections and graph relation/view semantics align.
5. `NV24A` Graph Overview Host-Form Contract. Depends: `NV15`, `NV20`, `NV24`. Done gate: the first graph-overview Navigator host is explicitly list-first in sidebars, optional-swatch gating is defined by host geometry, and toolbar hosts degrade to compact strips/counters rather than minimap semantics.
6. `NV24B` Graph Overview Projection Density + Filters. Depends: `NV18`, `NV24A`. Done gate: archived graph views are hidden by default behind an explicit filter toggle, dense inter-view relationships degrade to aggregated hints, and overview-projection state remains distinct from graph truth.
7. `NV24C` Graph Overview Routing Parity. Depends: `NV17`, `NV24A`, graph backlog equivalents. Done gate: focus/reveal actions route identically across toolbar and sidebar hosts, while structural editing hands off to the graph-owned Overview Plane rather than mutating Navigator-local state.
8. `NV25` Navigator Milestone Closure Receipt. Depends: `NV01`-`NV24`, `NV24A`-`NV24C`. Done gate: one closure doc states what another agent can now safely build on for sections, click grammar, routing, and projection behavior.

---

## Scenario Track

- `NVS01` `DI01` Graph-first local exploration parity. Depends: `NV06`, `NV10`, `NV16`. Done gate: selection -> ego graphlet -> frontier transition flow is implemented or evidenced without Navigator absorbing graph truth.
- `NVS02` `DI02` Corridor transition parity. Depends: `NV08`, `NV18`, `NV24`. Done gate: anchor selection -> corridor graphlet -> graph/path emphasis flow is coherent and evidenced against the canonical scenario.
- `NVS03` `DI05` Shell overview graphlet reorientation handoff. Depends: `NV17`, `NV21`, `NV23`. Done gate: a Shell overview handoff back into Navigator graphlet context is explicit, routed, and diagnosable.
- `NVS04` Graph Overview swatch-to-plane handoff parity. Depends: `NV24A`, `NV24B`, `NV24C`. Done gate: compact Navigator overview surfaces orient and route successfully without pretending to be the full graph-view editor, and the handoff into Overview Plane is explicit and testable.
