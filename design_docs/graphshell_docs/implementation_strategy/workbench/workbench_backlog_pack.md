<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workbench Backlog Pack

**Date**: 2026-03-17
**Status**: Planning / handoff pack
**Scope**: Dependency-ordered backlog for the Workbench authority, tile tree,
pane lifecycle, focus, persistence, and surface routing.

**Related docs**:

- [WORKBENCH.md](WORKBENCH.md) - workbench domain ownership and graph/workbench bridge rules
- [pane_chrome_and_promotion_spec.md](pane_chrome_and_promotion_spec.md) - pane promotion and chrome rules
- [pane_presentation_and_locking_spec.md](pane_presentation_and_locking_spec.md) - pane lock and presentation authority
- [workbench_frame_tile_interaction_spec.md](workbench_frame_tile_interaction_spec.md) - frame/tile interaction semantics
- [tile_view_ops_spec.md](tile_view_ops_spec.md) - tile/view operation contract
- [graph_first_frame_semantics_spec.md](graph_first_frame_semantics_spec.md) - graph/workbench first-frame rules
- [../domain_interaction_acceptance_matrix.md](../domain_interaction_acceptance_matrix.md) - compact review matrix for cross-domain scenario evidence
- [../../technical_architecture/domain_interaction_scenarios.md](../../technical_architecture/domain_interaction_scenarios.md) - canonical cross-domain scenario IDs (`DI01`-`DI06`)

## Tracker mapping

- Hub issue: #306 (`Hub: five-domain architecture adoption — Shell host, graphlet model, cross-domain scenarios`)
- Primary implementation issue: #304 (`Adopt the canonical graphlet model across Navigator and Workbench`)
- Review/evidence issue: #305 (`Operationalize cross-domain scenario IDs and acceptance evidence`)

---

## Landed Delta (2026-03-18)

The following slices are now landed in code and should be treated as backlog progress:

- ✅ Intent-carrier cleanup for navigator/workbench routing:
     legacy `FileTree*` variants were removed from active reducer/view-action paths,
     with canonical `Navigator*` carriers used at app/runtime boundaries.
     Advances: `WB06`, `WB08`.
- ✅ Navigator projection reset and state ownership paths now resolve through
     `NavigatorProjectionState` naming consistently across runtime/workspace state.
     Advances: `WB03`, `WB05`.
- ✅ Node pane now hosts collapsible `Node History` and `Node Audit` sections,
     backed by history query helpers instead of ad hoc pane-local state.
     Advances: `WB17`, `WB21`.

Remaining backlog IDs keep their existing done-gates unless explicitly closed by
a dedicated closure receipt.

---

## Wave 1

1. `WB01` Workbench Core Boundary. Depends: none. Done gate: one canonical doc distinguishes workbench authority from graph truth and renderer/session caches.
2. `WB02` Tile Tree Ownership Audit. Depends: `WB01`. Done gate: every tile-tree mutation path is inventoried and tagged workbench-owned, graph-backed, or legacy violation.
3. `WB03` Pane / Tile / Frame Glossary Lock. Depends: `WB01`. Done gate: `Pane`, `Tile`, `Frame`, `Tab`, `ToolPane`, and `ArrangementObject` each have one canonical definition.
4. `WB04` Stable `PaneId` Coverage Audit. Depends: `WB02`, `WB03`. Done gate: every pane variant that can be opened, focused, split, or closed has a stable `PaneId`.
5. `WB05` Workbench Truth vs Session State Contract. Depends: `WB01`, `WB04`. Done gate: durable arrangement truth is separated from session-only focus/open state.
6. `WB06` Workbench Intent Inventory. Depends: `WB02`. Done gate: every current workbench action is mapped to `WorkbenchIntent`, graph bridge, or legacy helper.
7. `WB07` Legacy Workbench Mutation Diagnostics. Depends: `WB06`. Done gate: direct tile-tree mutations outside workbench authority emit explicit diagnostics in development paths.
8. `WB08` Open / Close / Focus Carrier Cleanup Plan. Depends: `WB06`, `WB07`. Done gate: every pane open, close, focus, split, and reorder path has one canonical carrier.
9. `WB09` Workbench Contract Test Harness. Depends: `WB02`. Done gate: a test seam exists for split, close, focus, promotion, and restore behavior.
10. `WB10` Single Workbench Mutation Path Closure Slice. Depends: `WB07`, `WB09`. Done gate: at least one major legacy workbench mutation cluster is removed and proven to flow only through workbench authority.

## Wave 2

1. `WB11` Tile Tree Semantic Contract. Depends: `WB03`, `WB05`. Done gate: tiles, tabs, and frames have explicit semantic ownership and lifecycle rules.
2. `WB12` Tab Semantics Lock. Depends: `WB11`. Done gate: tabs are defined as entries within tiles, not a separate primary ontology.
3. `WB13` Pane Promotion Contract. Depends: `WB11`, `WB12`. Done gate: promotion and demotion between solo, grouped, and tabbed states are defined with explicit carriers.
4. `WB14` Split Semantics Contract. Depends: `WB11`. Done gate: horizontal and vertical split behavior, including resulting focus rules, is canonical.
5. `WB15` Close Pane Contract. Depends: `WB11`, `WB14`. Done gate: close behavior is defined for graph, node, and tool panes with restore and focus fallthrough rules.
6. `WB16` Restore and Reopen Contract. Depends: `WB13`, `WB15`. Done gate: restore/open previous focus behavior is explicit and testable.
7. `WB17` Tool Pane Authority Audit. Depends: `WB03`, `WB06`. Done gate: settings, command palette, history, and similar panes all use workbench-owned routes.
8. `WB18` Locking and Mutability Contract. Depends: `WB11`, `WB17`. Done gate: pane and frame lock states are defined with allowed and blocked operations.
9. `WB19` Workbench Persistence Schema Audit. Depends: `WB05`, `WB11`. Done gate: durable workbench layout state is separated from ephemeral session state.
10. `WB20` Workbench WAL Coverage Audit. Depends: `WB19`. Done gate: every durable workbench mutation is either WAL-logged or explicitly marked non-durable.

## Wave 3

1. `WB21` Focus Model Contract. Depends: `WB11`, `WB15`. Done gate: pane focus, tab focus, and frame focus are distinguished and routed consistently.
2. `WB22` Selection-to-Workbench Targeting Rule. Depends: `WB21`. Done gate: selected objects that map to workbench actions become explicit workbench targets.
3. `WB23` Workflow Activation Contract. Depends: `WB19`, `WB21`. Done gate: workflow/session-mode activation is explicit about runtime-owned versus persisted-default behavior.
4. `WB24` Workbench Surface Diagnostics Pack. Depends: `WB18`, `WB21`, `WB23`. Done gate: blocked close, blocked split, focus failure, and restore failure all emit diagnostics.
5. `WB25` Workbench Milestone Closure Receipt. Depends: `WB01`-`WB24`. Done gate: one closure doc states what workbench authority is canonical, what remains transitional, and what downstream lanes can safely assume.

---

## Scenario Track

- `WBS01` `DI03` Linked arrangement around graphlet. Depends: `WB08`, `WB11`, `WB16`. Done gate: Workbench can create or focus a graphlet-linked arrangement while keeping graphlet truth external to arrangement ownership.
- `WBS02` `DI04` Viewer fallback in workbench-heavy session. Depends: `WB15`, `WB21`, `WB24`. Done gate: viewer fallback/degraded-state handling preserves pane placement and focus context and produces diagnosable evidence.
- `WBS03` `DI05` Shell overview frame/pane reorientation. Depends: `WB21`, `WB24`. Done gate: Shell overview handoff to focused frame/pane state is explicit, routed, and does not bypass workbench authority.
