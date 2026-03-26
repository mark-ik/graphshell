<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Navigator Backlog Pack

**Date**: 2026-03-17
**Status**: Planning / handoff pack
**Scope**: Dependency-ordered backlog for Navigator as a projection surface over
graph truth and workbench arrangement state.

**Related docs**:

- [navigator_graph_isomorphism_spec.md](navigator_graph_isomorphism_spec.md) - canonical Navigator click grammar and graph/workbench resolution rules
- [WORKBENCH.md](WORKBENCH.md) - workbench domain ownership and sidebar context
- [graph_first_frame_semantics_spec.md](graph_first_frame_semantics_spec.md) - graph/workbench frame semantics
- [../graph/graph_backlog_pack.md](../graph/graph_backlog_pack.md) - graph-side backlog dependencies used by Navigator projections

---

## Landed Delta (2026-03-18)

Confirmed slices now landed in code:

- ✅ Navigator intent surface canonicalization:
     active reducer/view-action paths use `SetNavigator*` and
     `RebuildNavigatorProjection`; legacy `SetFileTree*` and
     `RebuildFileTreeProjection` paths are removed.
     Advances: `NV01`, `NV06`.
- ✅ Containment projection source is now graph-backed from
     `ContainmentRelation` edges (rather than URL-only ad hoc reconstruction).
     Advances: `NV04`, `NV18`.
- ✅ Projection refresh now triggers from graph deltas affecting containment
     (node add/remove and URL updates), reducing manual refresh dependence.
     Advances: `NV10`.

These deltas do not close Wave-level milestones by themselves; they are partial
closures that should be referenced by future `NV25` receipt work.

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

11. `NV11` Navigator Reveal Rule. Depends: `NV07`, `NV08`. Done gate: reveal-on-select only happens when the graph is visible and the selected node is offscreen.
12. `NV12` Navigator Multi-Selection Semantics. Depends: `NV07`. Done gate: `Ctrl+Click`, range/toggle behavior, and row focus rules are explicit.
13. `NV13` Navigator Command Applicability Audit. Depends: `NV07`, `NV12`. Done gate: navigator-invoked commands obey the same "valid for every selected object" rule.
14. `NV14` Navigator Recents Contract. Depends: `NV04`, `NV08`. Done gate: `Recent` is a recency-sorted projection with clear entry/exit semantics.
15. `NV15` Navigator Arrangement Projection Contract. Depends: `NV04`, `NV09`. Done gate: frames, tiles, and graphlets project consistently as expandable arrangement objects.
16. `NV16` Navigator Search / Filter Model. Depends: `NV04`, `NV10`. Done gate: local filter/search semantics are explicit and do not mutate underlying truth.
17. `NV17` Navigator Dismiss / Open Actions Audit. Depends: `NV06`, `NV13`. Done gate: Navigator row actions route through graph/workbench carriers instead of direct callsites.
18. `NV18` Navigator Edge / Relation Surfacing Policy. Depends: `NV04`, `NV15`. Done gate: relation families shown in Navigator are explicit and tied to shared graph relation-family rules.
19. `NV19` Navigator Empty / Degraded State Contract. Depends: `NV10`, `NV16`. Done gate: no-data, filtered-empty, and projection-error states are distinct and diagnosable.
20. `NV20` Navigator Accessibility and Keyboard Model. Depends: `NV03`, `NV06`, `NV12`. Done gate: keyboard navigation, row focus, expand/collapse, and activate are specified.

## Wave 3

21. `NV21` Navigator Diagnostics Pack. Depends: `NV10`, `NV19`, `NV20`. Done gate: invalid row targets, stale projection entries, and routing failures emit diagnostics.
22. `NV22` Navigator Scenario Test Matrix. Depends: `NV06`-`NV21`. Done gate: tests/spec scenarios cover row-type clicks, selection, reveal, recents, search/filter, and arrangement rows.
23. `NV23` Workbench-Navigator Contract Sync Pass. Depends: `WB25`, `NV22`. Done gate: workbench pane/focus semantics and Navigator projection semantics no longer contradict each other.
24. `NV24` Graph-Navigator Contract Sync Pass. Depends: `NV04`, `NV18`, graph backlog equivalents. Done gate: Navigator sections and graph relation/view semantics align.
25. `NV25` Navigator Milestone Closure Receipt. Depends: `NV01`-`NV24`. Done gate: one closure doc states what another agent can now safely build on for sections, click grammar, routing, and projection behavior.
