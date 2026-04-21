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
- [navigator_projection_spec.md](navigator_projection_spec.md) — canonical projection pipeline, composition, annotation, and diff contract
- [2026-04-21_navigator_projection_pipeline_plan.md](2026-04-21_navigator_projection_pipeline_plan.md) — producing plan and projection-pipeline findings
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
8. `NV24D` Navigator Annotation Registry Contract. Depends: `NV18`, `NV20`, `NV24`. Done gate: `NodeAnnotation` registry contributions, density classes, and cost/budget rules are canonical and row rendering no longer hard-codes every badge family.
9. `NV24E` Navigator Portal Gesture Contract. Depends: `NV11`, `NV17`, `NV24`. Done gate: `Locate`, `Reveal-in-place`, and `Lift` are named, routed through graph/workbench intents, and do not contradict reducer-only mutation rules.
10. `NV24F` Navigator Projection Diff Contract. Depends: `NV10`, `NV20`, `NV24`. Done gate: projection switches and refresh-trigger reruns animate via canonical diff identity and timing rules instead of teleporting rows.
11. `NV24G` Navigator Time-Axis Projection Contract. Depends: `NV10`, `NV14`, `NV24`. Done gate: time-axis is specified as a specialty projection consuming `mixed_timeline_entries`, with host-local cursor semantics and composition rules.
12. `NV24H` Navigator Layout Inheritance Contract. Depends: `NV15`, `NV24A`, `NV24C`. Done gate: every projection declares `own`, `canvas`, or `canvas-compressed` layout inheritance so host rendering and canvas-derived updates do not guess.
13. `NV24I` Navigator Projection Cost Classification. Depends: `NV10`, `NV24D`, `NV24H`. Done gate: every projection and annotation contributor declares `live`, `debounced`, or `on-demand` behavior plus any incremental-update requirement.
14. `NV25` Navigator Milestone Closure Receipt. Depends: `NV01`-`NV24`, `NV24A`-`NV24I`. Done gate: one closure doc states what another agent can now safely build on for sections, click grammar, routing, and projection behavior.

## Projection Pipeline Mapping (2026-04-21)

- `NV01` maps to the pipeline boundary and the rule that Navigator is projection-and-routing only.
- `NV04` maps to stage 1 Scope and section/source alignment.
- `NV10` maps to refresh-trigger routing for projection reruns.
- `NV14` maps to recency-scored projections and declared cost class.
- `NV15` maps to arrangement-backed scopes, graphlets, and layout inheritance.
- `NV18` maps to relation surfacing through annotation, not silent suppression.
- `NV24A`-`NV24C` map to host-specific presentation and overview-swatch behavior.
- `NV24D`-`NV24I` cover the missing pipeline-wide primitives introduced by `navigator_projection_spec.md`.

---

## Scenario Track

- `NVS01` `DI01` Graph-first local exploration parity. Depends: `NV06`, `NV10`, `NV16`. Done gate: selection -> ego graphlet -> frontier transition flow is implemented or evidenced without Navigator absorbing graph truth.
- `NVS02` `DI02` Corridor transition parity. Depends: `NV08`, `NV18`, `NV24`. Done gate: anchor selection -> corridor graphlet -> graph/path emphasis flow is coherent and evidenced against the canonical scenario.
- `NVS03` `DI05` Shell overview graphlet reorientation handoff. Depends: `NV17`, `NV21`, `NV23`. Done gate: a Shell overview handoff back into Navigator graphlet context is explicit, routed, and diagnosable.
- `NVS04` Graph Overview swatch-to-plane handoff parity. Depends: `NV24A`, `NV24B`, `NV24C`. Done gate: compact Navigator overview surfaces orient and route successfully without pretending to be the full graph-view editor, and the handoff into Overview Plane is explicit and testable.

---

## Navigator Chrome Drift Analysis — Spec vs Live Code

*Captured from implementation session. These are concrete code-level drift points where
the live code does not match the chrome scope split spec.*

### Drift Point 1 — `render_navigation_buttons` in workbench_host.rs (CRITICAL)

**Spec says** (`subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` §5.1):
"Disallowed here as primary command ownership: Back / Forward / Reload / StopLoad, Find in
page, Content zoom controls, Compat / backend toggle."

**Code does**: `workbench_host.rs:2281-2288` calls `render_navigation_buttons()` which
renders Back/Forward/Reload/StopLoad/ZoomIn/ZoomOut/ZoomReset buttons, all dispatched via
`toolbar_routing::run_nav_action()`.

**Fix**: Delete `render_navigation_buttons()` (lines 3528-3629) and its call site (lines
2281-2288).

### Drift Point 2 — `focused_content_status` parameter

**Spec says**: Navigator host projects read-only status badges only, not actionable viewer
state.

**Code does**: `render_workbench_host()` takes `focused_content_status: &FocusedContentStatus`
(line 1802) and `location_dirty: &mut bool` (line 1803). Both are ONLY consumed by
`render_navigation_buttons()`. No other code in the function reads them.

**Fix**: Remove both parameters from `render_workbench_host()` signature and from the call
site in `toolbar_dialog.rs`.

### Drift Point 3 — Dead imports after removal

**Code has**: `use toolbar_routing::{self, ToolbarNavAction}` (line 30) and
`use gui_state::FocusedContentStatus` (line 28). Both become unused after removing
`render_navigation_buttons`.

**Fix**: Remove both import lines.

### Drift Point 4 — `WorkbenchChromeProjection` spec `nav` field

**Spec says** (`chrome_scope_split_plan.md` §6, line 422): the struct still lists
`pub nav: PaneNavState`.

**Code does**: The real `WorkbenchChromeProjection` (`workbench_host.rs:262-278`) does NOT
have a `nav` field — it was never implemented. The spec-only field contradicts the updated
§5.1 disallowed-controls list.

**Fix**: Remove `pub nav: PaneNavState` from the spec struct. The
`focused_pane_status: Vec<PaneStatusBadge>` field (lines 423-425) already covers read-only
badges and is the correct replacement.

### Drift Point 5 — §12 acceptance criterion (line 750)

**Spec says**: "Back/Forward/Reload live in the workbench-scoped host, not in the
graph-scoped host" — this is the OLD model.

**Should say**: "Back/Forward/Reload live in tile chrome, not in any Navigator host."

**Fix**: Update line 750 to match the tile-chrome-only model.

### Drift Point 6 — `WorkbenchPaneEntry` missing `presentation_mode`

**Spec says** (`pane_chrome_and_promotion_spec.md` §2): pane tree should reflect
presentation mode (Floating/Docked/Tiled) with graduated row actions.

**Code does**: `WorkbenchPaneEntry` (`workbench_host.rs:229-239`) has no
`presentation_mode` field. All pane rows render identically.

**Fix**: Add `pub(crate) presentation_mode: PanePresentationMode` to `WorkbenchPaneEntry`.
Populate from `NodePaneState.presentation_mode` during `pane_entry_for_tile()`. Default to
`PanePresentationMode::Tiled` for graph and tool panes.

### Implementation Steps

1. Delete `render_navigation_buttons()` (`workbench_host.rs:3528-3629`) and call site
   (`lines 2281-2288`). The `render_frame_pin_controls` call at line 2289 remains.
2. Remove `focused_content_status` and `location_dirty` from `render_workbench_host()`
   signature; remove corresponding args from `gui_frame/toolbar_dialog.rs` call site.
3. Remove dead imports (`FocusedContentStatus`, `toolbar_routing`/`ToolbarNavAction`).
4. Add `pub(crate) presentation_mode: PanePresentationMode` to `WorkbenchPaneEntry`; populate
   in `pane_entry_for_tile()`.
5. Spec fixes in `chrome_scope_split_plan.md`: remove `pub nav: PaneNavState`; update §12
   line 750; add `presentation_mode` to `WorkbenchTreeRow`.

**Verification**: `cargo check` + `cargo test --lib -- --quiet` clean.
