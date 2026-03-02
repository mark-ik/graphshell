# UX Migration Feature ↔ Spec Coverage Matrix

**Date**: 2026-03-02  
**Status**: Canonical planning artifact  
**Purpose**: Ensure every UX migration feature is anchored to an existing canonical spec or a clearly defined new spec, with three-tree pattern compliance checks.

**Related**:
- `2026-03-01_ux_migration_design_spec.md`
- `../research/2026-03-02_ux_integration_research.md`
- `subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `subsystem_ux_semantics/ux_scenario_and_harness_spec.md`
- `aspect_input/input_interaction_spec.md`
- `aspect_command/command_surface_interaction_spec.md`
- `aspect_control/settings_and_control_surfaces_spec.md`
- `canvas/graph_node_edge_interaction_spec.md`
- `canvas/layout_behaviors_and_physics_spec.md`
- `canvas/multi_view_pane_spec.md`
- `subsystem_history/edge_traversal_spec.md`
- `workbench/workbench_frame_tile_interaction_spec.md`
- `workbench/graph_first_frame_semantics_spec.md`
- `2026-03-01_ux_migration_lifecycle_audit_register.md`
- `subsystem_ux_semantics/ux_event_dispatch_spec.md`
- `aspect_command/radial_menu_geometry_and_overflow_spec.md`

---

## 1. Coverage Rubric

**Coverage status**
- **Green**: Feature has canonical spec coverage with acceptance criteria.
- **Yellow**: Partially covered; semantics exist but feature-specific contract is split/implicit.
- **Red**: No dedicated canonical contract for feature behavior.

**Three-tree pattern gate (required for Green)**
1. **Graph Tree authority** is explicit where data truth changes.
2. **Workbench Tree authority** is explicit where arrangement/handle truth changes.
3. **UxTree contract** exposes the feature via roles/actions/states/probes.

---

## 2. UX Migration Feature Inventory and Mapping

| UX migration feature | Primary source section | Current spec anchors | Coverage | Three-tree gate | Action needed |
|---|---|---|---|---|---|
| Three-phase event dispatch | UX migration §3.2, §7 | `aspect_input/input_interaction_spec.md`, `subsystem_ux_semantics/ux_tree_and_probe_spec.md`, `subsystem_ux_semantics/ux_event_dispatch_spec.md` | Yellow | Partial | Implement + validate dispatch spec via `#261` + `#269` |
| UxTree as UX source of truth trajectory | UX migration §3.3 | `subsystem_ux_semantics/ux_tree_and_probe_spec.md` | Yellow | Partial (trajectory defined, migration gates not fully staged) | Add **UxTree Convergence Roadmap** spec with gated milestones and non-goals |
| Faceted node schema (PMEST) | UX migration §4.1 | UX migration only | Red | Missing | Create **Faceted Filter Surface Spec** (schema, operators, index strategy, result model) |
| Faceted filter operations | UX migration §4.2 | UX migration only | Red | Missing | Same as above; include acceptance tests and omnibar/palette integration |
| Facet rail + Enter-to-pane routing | UX migration §4.3, §5.1A | UX migration only | Red | Missing | Create **Facet Pane Routing Spec** (input context, focus return, pane target resolution, UxTree exposure) |
| Selection and lasso semantics | UX migration §5.1 | `canvas/graph_node_edge_interaction_spec.md`, `aspect_input/input_interaction_spec.md` | Yellow | Partial | Add lasso modifier and boundary semantics to canvas canonical spec + tests |
| Target-locked/pointer-relative zoom | UX migration §5.2 | `canvas/graph_node_edge_interaction_spec.md` | Yellow | Partial | Add explicit pointer-anchor invariants and passive-input rule to canonical canvas spec |
| Node manipulation (create/delete/pin/group-move) | UX migration §5.3 | `canvas/graph_node_edge_interaction_spec.md` | Yellow | Partial | Add complete key/action mapping and group-move mode contract in canonical spec |
| Edge management interactions | UX migration §5.4 | `canvas/graph_node_edge_interaction_spec.md`, `subsystem_history/edge_traversal_spec.md` | Yellow | Partial | Align edge-focus traversal policy between canvas and history specs |
| Traversal event stream interaction | UX migration §5.4A | `subsystem_history/edge_traversal_spec.md` (§2.3A) | Green | Pass | Keep glossary/spec parity checks in future doc passes |
| Physics controls (toggle/reheat/preset) | UX migration §5.5 | `canvas/layout_behaviors_and_physics_spec.md` | Green | Pass | None |
| Command palette mode unification (global/context/radial) | UX migration §5.6 | `aspect_command/command_surface_interaction_spec.md`, `aspect_input/input_interaction_spec.md` | Yellow | Partial | Implement shared two-tier model + right-click contextual shell + omnibar contract via `#263` + `#270` |
| Radial palette geometry/readability redesign | UX migration §5.6 | `aspect_command/command_surface_interaction_spec.md`, `aspect_command/radial_menu_geometry_and_overflow_spec.md` | Green | Pass | Keep radial diagnostics channels (`ux:radial_layout`, `ux:radial_overflow`, `ux:radial_label_collision`) and unit coverage in CI gate |
| Frame management basics | UX migration §5.7 | `workbench/workbench_frame_tile_interaction_spec.md` | Green | Pass | None |
| Graph-first frame semantics (cross-tree) | UX migration §5.7A | `workbench/graph_first_frame_semantics_spec.md` | Green | Pass | Propagate terminology to remaining canvas docs |
| Multiple graph views (canonical/divergent) | UX migration §5.8 | `canvas/multi_view_pane_spec.md` | Green | Pass | None |
| Multiple workbenches | UX migration §5.9 | `workbench/workbench_frame_tile_interaction_spec.md` | Yellow | Partial | Add explicit inter-workbench switch/open semantics to canonical workbench spec |
| User-configurable WorkbenchProfile | UX migration §5.10 | `aspect_input/input_interaction_spec.md` (InputProfile), `aspect_control/settings_and_control_surfaces_spec.md` | Yellow | Partial | Create **WorkbenchProfile & Workflow Composition Spec** |
| Layout mode portfolio | UX migration §6.1 | `canvas/layout_behaviors_and_physics_spec.md` | Yellow | Partial | Create **Layout Algorithm Portfolio Spec** |
| Readability-driven adaptation | UX migration §6.2 | UX migration + research refs only | Red | Missing | Extend layout canonical spec with readability metric contract/channels |
| LOD semantic zoom policy | UX migration §6.3 | `canvas/graph_node_edge_interaction_spec.md` (partial), UxTree C5 in `ux_tree_and_probe_spec.md` | Yellow | Partial | Add explicit LOD threshold contract to canonical canvas spec and UxTree emission contract cross-link |
| Modal isolation and focus return | UX migration §7.3 | `aspect_input/input_interaction_spec.md`, `subsystem_focus/focus_and_region_navigation_spec.md`, UxTree spec | Yellow | Partial | Add shared modal isolation contract table across Input + Focus + UxTree specs |
| Command Semantics Matrix deliverable | UX integration research §10 D1 | `../design/command_semantics_matrix.md`, `aspect_command/command_surface_interaction_spec.md`, UX register family | Green | Pass | `#292` canonical matrix landed; keep matrix synchronized with `ActionRegistry`/dispatch changes |
| Focus/Selection Interaction Contract deliverable | UX integration research §10 D2 | `subsystem_focus/focus_and_region_navigation_spec.md`, `workbench/workbench_frame_tile_interaction_spec.md` | Green | Pass | `#293` deliverable contract merged (selection scope, ownership map, handoff/arbitration rules, checklist + test references); `#300` predictability closure is implemented with deterministic focus/selection mapping and return-path test evidence |
| Surface Behavior Spec deliverable | UX integration research §10 D3 | `../design/surface_behavior_spec.md`, command/workbench/viewer specs | Green | Pass | `#294` canonical surface behavior spec is merged with policy + implementation checklist; discoverability addendum and implementation linkage are now closed via `#297` |
| Accessibility Baseline Checklist deliverable | UX integration research §10 D4 | `../design/accessibility_baseline_checklist.md`, `subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md`, focus/viewer specs | Green | Pass | `#295` canonical WCAG A/AA checklist is merged with initial screen-reader matrix; `#298` graph keyboard-focus + naming baseline evidence and `#301` closure evidence (reduced-motion guardrails, contrast/target-size audit, keyboard-trap validation) are recorded |
| UX Telemetry Plan deliverable | UX integration research §10 D5 | `../design/ux_telemetry_plan.md`, diagnostics registry + UxProbe/UxHarness specs | Green | Pass | `#296` canonical telemetry plan is merged with metric-channel-target mapping and wired diagnostics evidence (`ux:navigation_violation` / `ux:navigation_transition`) |
| Docs parity audit deliverable mapping | UX integration research §13 + control-plane ops | control-plane + matrix + lifecycle register | Green | Pass | `#302` parity audit baseline is closed; keep control-plane/matrix/lifecycle synchronized as maintenance |

---

## 3. New Specs Required (Priority Order)

1. **Faceted Filter Surface Spec** (Red)
2. **Facet Pane Routing Spec** (Red)
3. **Radial Palette Geometry & Overflow Spec** (Red)
4. **Layout Algorithm Portfolio Spec** (Yellow→Green)
5. **WorkbenchProfile & Workflow Composition Spec** (Yellow→Green)
6. **UxTree Event Dispatch Spec** or equivalent UxTree spec expansion (Yellow→Green)

---

## 4. Three-Tree Pattern Checklist (Apply per Feature)

For each feature before implementation completion:

- **Graph Tree**
  - Is data ownership explicit?
  - Are mutations expressed via reducer-owned intents?
- **Workbench Tree**
  - Is arrangement/handle behavior explicit?
  - Are close/open semantics non-destructive where intended?
- **UxTree**
  - Are roles/actions/states surfaced?
  - Is there probe/harness coverage for regressions?

Feature is **not complete** until all three pass.

---

## 5. Definition of UX Migration Completeness

UX migration is complete when:

1. Every feature row in §2 is Green.
2. Every Green row passes the three-tree gate.
3. Every feature has at least one canonical acceptance-criteria source doc.
4. UxHarness scenarios cover the critical path flows for each feature family.
5. Terminology is synchronized across canonical docs (`TERMINOLOGY.md` + strategy specs).

---

## 6. Immediate Next Doc Passes

1. Patch `canvas/layout_behaviors_and_physics_spec.md` to replace organizational `MagneticZone` wording with frame-affinity terminology where applicable.
2. Add explicit cross-links from canonical specs to `workbench/graph_first_frame_semantics_spec.md`.
3. Create the three Red specs listed in §3.
4. Add a recurring checklist item in PR review templates: “feature row updated in UX migration coverage matrix.”

## 6A. Issue Mapping Delta (2026-03-01)

- `#269` — Phase A supplement: UxTree event dispatch canonical-spec closure.
- `#270` — Phase C supplement: radial geometry and overflow contract closure.
- `#271` — Pre-WGPU canvas interaction invariants closure.
- `#272` — UxTree convergence roadmap staged authority gates.
- `#273` — Pre-WGPU UxHarness critical-path gate.
- `#292` — Command Semantics Matrix deliverable closure.
- `#293` — Focus/Selection Interaction Contract deliverable closure.
- `#294` — Surface Behavior Spec deliverable closure.
- `#295` — Accessibility Baseline Checklist deliverable closure.
- `#296` — UX Telemetry Plan deliverable closure.
- `#297` — Discoverability closure (empty states + disabled-action explanations).
- `#298` — Graph canvas keyboard focus + AccessKit naming implementation.
- `#299` — IA object-action scope audit + label disambiguation.
- `#300` — Predictability closure: selection truth + focus/active-pane mapping.
- `#301` — Accessibility closure bundle (reduced motion, contrast/target-size, keyboard trap).
- `#302` — Canonical docs parity audit against UX integration research (closed baseline pass).

---

## 7. Lifecycle Audit Alignment (Required)

This matrix tracks **spec coverage quality**.

`2026-03-01_ux_migration_lifecycle_audit_register.md` tracks **delivery stage and
timing gates** (`Current`/`Planned`/`Speculative`, pre/post renderer/WGPU,
pre/post networking).

A feature is migration-ready only when:

1. This matrix row is Green with three-tree pass, and
2. The lifecycle register row has an explicit timing gate and UxTree readiness
  state that matches implementation evidence.
