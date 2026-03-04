# Per-Control Audit Grid

**Date**: 2026-03-04
**Status**: Active / Baseline
**Purpose**: Provide the control-level audit matrix requested by UX Semantics planning: `surface -> region -> object -> trigger -> semantic result -> focus result -> visual result -> degradation result -> owner -> diagnostics -> verification -> standards mapping -> implementation status`.

**Related**:
- `2026-02-28_ux_contract_register.md`
- `2026-03-04_model_boundary_control_matrix.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`

---

## 1. Status scale

- `Implemented`: behavior is wired in runtime and covered by at least one targeted test.
- `Partial`: behavior is wired, but ownership/diagnostics/verification is incomplete.
- `Missing`: required contract behavior not yet wired.
- `Nonstandard`: behavior exists but diverges from canonical contract or adopted standards.

---

## 2. Audit grid

| Surface | Region | Object | Trigger | Semantic result | Focus result | Visual result | Degradation result | Owner | Diagnostics | Verification | Standards mapping | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Graph | Camera policy | Position-fit lock | `C` key | Toggles per-view `position_fit_locked` | Graph pane remains focus owner | Camera translation policy updates on next fit command | Falls back to no-op when no target view | Graph reducer | `ux:navigation_transition` on state change | `input::tests` + reducer tests | WCAG 2.2, OTel | Implemented |
| Graph | Camera policy | Zoom-fit lock | `Z` key | Toggles per-view `zoom_fit_locked` | Graph pane remains focus owner | Zoom policy updates on next fit command | Falls back to no-op when no target view | Graph reducer | `ux:navigation_transition` via lock-affecting transitions | `input::tests` + reducer tests | WCAG 2.2, OTel | Implemented |
| Graph | Camera commands | Fit-to-screen | explicit intent/UI command | Enqueues deterministic fit command | Focus unchanged | View recenters to graph bounds | Command blocked when no target view | Graph reducer | `runtime.ui.graph.camera_command_blocked_missing_target_view` | graph_app diagnostics tests | WCAG 2.2, OTel | Implemented |
| File Tree (graph projection) | Projection source | Containment source selector | pane control change | Rebuilds graph-owned row projection from selected source | Tool pane remains focus owner | Row set updates to source-backed entries | Empty projection when source unavailable | Graph reducer | `ux:navigation_transition` on source change | reducer tests | WCAG 2.2, OTel | Implemented |
| File Tree (graph projection) | Projection rows | Row selection | row click | Sets selected row keys in graph-owned projection state | Selected row becomes active navigation target | Selected row highlight updates | Stale rows pruned during rebuild | Graph reducer | `ux:navigation_transition` on selection change | reducer + diagnostics tests | WCAG 2.2, OTel | Implemented |
| File Tree (graph projection) | Projection rows | Row expansion | disclosure toggle | Updates expanded row keys in graph-owned projection state | Focus remains in pane | Expanded/collapsed affordance updates | Stale expansion pruned during rebuild | Graph reducer | None specific yet | reducer tests | WCAG 2.2 | Partial |
| File Tree (graph projection) | Imported FS source | File-backed row projection | source switch/rebuild | Builds `fs:` rows from `file://` graph nodes | Focus unchanged | Imported rows listed with readable labels | Non-file rows excluded | Graph reducer | covered indirectly via source transition | reducer tests | WCAG 2.2, RFC 3986 | Implemented |
| Workbench | Tool pane lifecycle | Open File Tree pane | settings menu action | Opens/focuses `ToolPaneState::FileTree` pane | Focus transitions into tool pane | File Tree pane visible | No-op if open/focus route already active | Workbench orchestration | `ux:navigation_transition` path in orchestration | tile_behavior + orchestration tests | WCAG 2.2, OTel | Implemented |
| Viewer | Viewer fallback | Placeholder/degraded explanation | viewer unavailable | Presents explicit fallback state with reason | Focus remains deterministic in host pane | Placeholder/degraded affordance shown | Recovery action remains available | Viewer policy + reducer routing | Viewer diagnostics channels | viewer spec-linked tests (in progress) | WCAG 2.2, OTel, OSGi | Partial |
| Focus | Region cycle | Top-level region cycling | `F6` | Cycles semantic focus owner across top-level regions | New region gains focus deterministically | Focus ring/landmark updates | Violation channel on unresolved cycle | Workbench + Focus subsystem | `ux:navigation_transition` / `ux:navigation_violation` | orchestration diagnostics tests | WCAG 2.2, OTel | Implemented |

---

## 3. Immediate gaps (from this grid)

1. Add explicit diagnostics channel/receipt for File Tree expansion-state transitions (currently partial).
2. Complete Viewer fallback verification receipts per control-path in the same matrix shape.
3. Extend matrix rows for command palette/radial menu overflow and settings apply/revert once those slices close.

---

## 4. Governance note

This grid is bounded to applicable adopted standards only (WCAG 2.2, OTel, OSGi, RFC 3986 where relevant) and should not be expanded into non-adopted standards coverage.
