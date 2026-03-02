# UX Migration Lifecycle Audit Register

**Date**: 2026-03-02  
**Status**: Canonical planning artifact  
**Purpose**: Unified audit of UX-related capabilities across active specs, implementation strategies, research notes, and archive checkpoints, with explicit delivery timing gates.

**Related**:
- `2026-03-01_ux_migration_design_spec.md`
- `2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
- `../research/2026-03-02_ux_integration_research.md`
- `subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `subsystem_ux_semantics/ux_scenario_and_harness_spec.md`
- `aspect_render/2026-03-01_webrender_readiness_gate_feature_guardrails.md`
- `aspect_render/2026-03-01_webrender_wgpu_renderer_implementation_plan.md`
- `PLANNING_REGISTER.md`

---

## 1. Audit Taxonomy

### 1.1 Lifecycle state

- **Current**: Implemented and/or actively enforced by canonical contracts.
- **Planned**: Explicitly scoped in canonical specs or implementation plans but not fully closed.
- **Speculative**: Research-backed or archive-derived candidate, not on immediate canonical delivery path.

### 1.2 Delivery timing gate

- **Pre-renderer/WGPU required**: Must land before any Glow → WebRender/wgpu switch authorization.
- **Post-renderer/WGPU**: Safe to defer until renderer switch gates (`G1–G5`) are closed.
- **Pre-networking required**: Must land before broader Verse/network-driven UX phases become default.
- **Post-networking**: Can follow Tier 1/Tier 2 networking maturity.

### 1.3 UxTree automation readiness

- **Green**: UxTree role/state contract + UxProbe invariants + UxHarness scenario path are all specified.
- **Yellow**: Partial UxTree and/or probe contract exists, but scenario/probe coverage is incomplete.
- **Red**: No explicit UxTree/Probe/Harness contract path yet.

---

## 2. Lifecycle Register (Current Canonical Scope)

| Capability family | Lifecycle | Timing gate | UxTree readiness | Primary authority | Notes |
|---|---|---|---|---|---|
| Three-phase event dispatch | Planned | Pre-renderer/WGPU required | Yellow | `2026-03-01_ux_migration_design_spec.md` + `aspect_input/input_interaction_spec.md` | Needs dedicated dispatch contract in UX semantics or expanded canonical UxTree routing table |
| UxTree authority trajectory (UX source of truth) | Planned | Pre-renderer/WGPU required | Yellow | `2026-03-01_ux_migration_design_spec.md` §3.3 + `subsystem_ux_semantics/ux_tree_and_probe_spec.md` | Requires staged convergence roadmap with explicit non-goals |
| Faceted filter schema + operations | Planned | Pre-networking required | Red | `2026-03-01_ux_migration_design_spec.md` | Missing dedicated canonical Faceted Filter Surface spec |
| Facet rail + Enter-to-pane routing | Planned | Pre-networking required | Red | `2026-03-01_ux_migration_design_spec.md` | Missing Facet Pane Routing spec (input/focus/pane-target semantics) |
| Core graph selection/lasso semantics | Planned | Pre-renderer/WGPU required | Yellow | `canvas/graph_node_edge_interaction_spec.md` + `aspect_input/input_interaction_spec.md` | Canonical modifier/boundary invariants are explicit (`#271`); remaining grouped closure slices tracked in `#173`, `#185`, `#102`, `#104`, `#101`, `#103` |
| Target-locked zoom + pointer-relative camera behavior | Planned | Pre-renderer/WGPU required | Yellow | `canvas/graph_node_edge_interaction_spec.md` | Requires pointer-anchor invariants and passive-input conformance text |
| Edge traversal event-stream projection | Current | Pre-networking required | Yellow | `subsystem_history/edge_traversal_spec.md` + `2026-03-01_ux_migration_design_spec.md` | Core model aligned; edge-focus inspection vs traversal append parity is now explicit; timeline/preview scenario coverage remains planned |
| Physics presets and mode switching | Current | Pre-renderer/WGPU required | Yellow | `canvas/layout_behaviors_and_physics_spec.md` | Runtime behavior is canonical; readability-driven adaptation still planned |
| Layout readability diagnostics and adaptation | Planned | Post-renderer/WGPU | Yellow | `2026-03-01_ux_migration_design_spec.md` + research (`1808.00703`) | Advisory vs automatic adaptation policy still open |
| Command palette contextual mode surface (context + radial) | Planned | Pre-renderer/WGPU required | Red | `aspect_command/command_surface_interaction_spec.md` + migration spec | Needs full two-tier mode parity + dedicated radial geometry/overflow canonical spec |
| Frame/tile baseline interactions | Current | Pre-renderer/WGPU required | Green | `workbench/workbench_frame_tile_interaction_spec.md` | Canonical interaction contract is active |
| Graph-first frame lifecycle semantics | Current | Pre-renderer/WGPU required | Green | `workbench/graph_first_frame_semantics_spec.md` | Cross-tree authority model is now explicit and canonical |
| Multi-view (canonical/divergent) graph panes | Current | Pre-renderer/WGPU required | Yellow | `canvas/multi_view_pane_spec.md` | UxHarness flows for divergence sync/merge need explicit CI gating |
| Multi-workbench management | Planned | Pre-networking required | Yellow | `workbench/workbench_frame_tile_interaction_spec.md` | Inter-workbench open/switch semantics need canonical closure |
| WorkbenchProfile composition and sharing | Planned | Pre-networking required | Yellow | `aspect_input/input_interaction_spec.md` + `aspect_control/settings_and_control_surfaces_spec.md` | Requires dedicated WorkbenchProfile/Workflow composition spec |
| Viewer fallback/degraded-state clarity | Current | Pre-renderer/WGPU required | Yellow | `viewer/viewer_presentation_and_fallback_spec.md` | Placeholder-state UX and diagnostics messaging still needs scenario hardening |
| UxTree/UxProbe runtime contracts | Current | Pre-renderer/WGPU required | Green | `subsystem_ux_semantics/ux_tree_and_probe_spec.md` | Core C1–C5 + probe contracts are canonical |
| UxScenario/UxHarness deterministic UX testing | Current | Pre-renderer/WGPU required | Green | `subsystem_ux_semantics/ux_scenario_and_harness_spec.md` | CI-required core scenarios already specified |
| UX integration deliverables (D1-D5) | Current | Pre-renderer/WGPU required | Green | UX integration research + canonical spec family | D1 (`#292`) through D5 (`#296`) canonical artifacts are merged and linked into the control-plane/parity triad |
| IA/predictability/discoverability closure bundle | Current | Pre-renderer/WGPU required | Green | command/focus/workbench specs + UX research | `#297` discoverability, `#299` IA scope/label disambiguation, and `#300` predictability closure are implemented with canonical focus/workbench contracts and targeted deterministic-focus tests |
| Accessibility closure bundle (beyond baseline) | Planned | Pre-renderer/WGPU required | Yellow | accessibility/focus/viewer specs + UX research | `#301` closure evidence is merged (reduced-motion guardrails, contrast/target-size audit artifact, keyboard-trap validation); remaining implementation closure tracked by `#298` |
| Canonical docs parity audit | Planned | Pre-renderer/WGPU required | Green | control-plane + coverage matrix + lifecycle register | Tracked by `#302`; keeps research/issues/specs synchronized |

---

## 3. Renderer/WGPU Readiness Dependency View

### 3.0 Practical cutline (canonical)

- If a feature changes interaction semantics or contract invariants, it is **Pre-renderer/WGPU required**.
- If a feature changes visual sophistication, optional modes, or speculative capability, it is **Post-renderer/WGPU**.

### 3.0A Stale-but-relevant reinterpretations (canonical)

- **"Magnetic zones"** are interpreted as **frame-affinity organizational behavior** under graph-first frame semantics.
- **Legacy context menu as primary surface** remains deprecated in favor of Command Palette contextual mode.
- **Edge semantics remain event-stream-first**: traversal events are primary and projected into durable edge state.

### 3.1 Must close before switch authorization (UX side)

The following UX slices are mandatory closure candidates before renderer switch authorization is considered complete for product-level UX confidence:

1. Three-phase event dispatch contract closure.
2. Canonical radial overflow/readability contract closure.
3. Canvas interaction invariants (selection/lasso/zoom) fully normalized in canonical specs.
4. UxHarness coverage for critical graph-workbench command flows.

These are aligned to guardrail policy in
`aspect_render/2026-03-01_webrender_readiness_gate_feature_guardrails.md`:
feature work continues, but migration-adjacent UX behavior must remain fallback-safe,
observable, and contract-driven.

### 3.2 Pre-WGPU closure checklist (canonical gate)

- [ ] Event dispatch contract closure: `#261`, `#269`.
- [x] Radial geometry/overflow closure: `#263`, `#270`.
- [ ] Canvas interaction invariants closure (selection/lasso/zoom/edge focus): `#271`, `#173`, `#185`, `#102`, `#104`, `#101`, `#103`.
`#271` now contributes explicit canonical invariants for lasso/zoom/edge-focus and targeted diagnostics coverage; this grouped checklist item remains open pending companion issues.
- [x] Viewer fallback/degraded-state clarity closure: `#188`, `#162`.
`#162` overlay affordance policy per `TileRenderMode` is implemented at compositor boundary, and `#188` now adds explicit user-visible degraded/fallback reasons + recovery affordances with diagnostics receipts.
- [ ] UxHarness critical-path evidence closure: `#251`, `#257`, `#273`.
- [ ] UxTree authority trajectory gate closure: `#272`.
- [x] UX integration deliverables closure: `#292`, `#293`, `#294`, `#295`, `#296`.
`#292` command semantics matrix, `#293` focus/selection interaction contract, `#294` surface behavior policy, `#295` accessibility baseline checklist, and `#296` telemetry plan are present in canonical design/spec docs.
- [x] IA/predictability/discoverability closure bundle: `#297`, `#299`, `#300`.
`#297` discoverability closure (empty-state inventory + disabled-action explanations with regression checks), `#299` IA object-action scope/label disambiguation closure, and `#300` predictability closure (selection/focus mapping + return-path validation) are implemented and linked in canonical surface/command/workbench behavior docs.
- [ ] Accessibility closure bundle: `#298`.
- [ ] Canonical docs parity audit closure: `#302`.
- [ ] Terminology reinterpretation pass complete in affected canonical docs:
	- "Magnetic zones" language reframed to frame-affinity behavior.
	- Context-menu-primary language reframed to Command Palette contextual mode.
	- Edge semantics framed as traversal-event projection.

### 3.2A Pre-WGPU Spec → Issue Linkage (audit table)

| Canonical spec | Pre-WGPU closure issue IDs | Scope note |
|---|---|---|
| `subsystem_ux_semantics/ux_event_dispatch_spec.md` | `#261`, `#269` | Dispatch contract closure |
| `aspect_command/radial_menu_geometry_and_overflow_spec.md` | `#263`, `#270` | Radial geometry, overflow, readability contract |
| `aspect_command/command_surface_interaction_spec.md` | `#263`, `#270` | Command-surface mode parity and contextual invocation |
| `canvas/graph_node_edge_interaction_spec.md` | `#271`, `#173`, `#185`, `#102`, `#104`, `#101`, `#103` | Selection/lasso/zoom/edge-focus invariants |
| `aspect_input/input_interaction_spec.md` | `#271`, `#103` | Input routing and canvas interaction boundary semantics |
| `viewer/viewer_presentation_and_fallback_spec.md` | `#188`, `#162` | Fallback/degraded-state clarity |
| `subsystem_ux_semantics/ux_scenario_and_harness_spec.md` | `#251`, `#257`, `#273` | Critical-path UxHarness gate evidence |
| `subsystem_ux_semantics/ux_tree_and_probe_spec.md` | `#272`, `#251`, `#257`, `#273` | UxTree authority trajectory and probe/harness closure |
| `subsystem_focus/focus_and_region_navigation_spec.md` | `#140`, `#174`, `#187`, `#189`, `#103` | Focus-domain primary/support mapping from UX control-plane; represented in milestone domains, not as a separate checklist bullet |
| `workbench/workbench_frame_tile_interaction_spec.md` | `#293`, `#300` | Cross-tree focus/selection integration anchor for routing/activation semantics and return-path linkage |
| `graphshell_docs/design/command_semantics_matrix.md` | `#292`, `#299` | D1 canonical action semantics matrix; IA scope clarity and future surface expansion remain linked to IA closure work |
| `graphshell_docs/design/surface_behavior_spec.md` | `#294`, `#297` | D3 canonical surface behavior policy plus discoverability addendum (empty-state inventory + disabled-action explanation policy + implementation/test linkage) |
| `graphshell_docs/design/accessibility_baseline_checklist.md` | `#295`, `#298`, `#301` | D4 canonical WCAG A/AA baseline checklist with initial screen-reader matrix; implementation hardening remains tracked by accessibility closure issues |
| `graphshell_docs/design/ux_telemetry_plan.md` | `#296` | D5 canonical telemetry metric register with channel/probe mapping, baseline targets, and wired diagnostics evidence |
| UX integration research deliverables (`D1`-`D5`) | `#292`, `#293`, `#294`, `#295`, `#296` | Command semantics, interaction contract, surface behavior, accessibility baseline, telemetry plan |
| Canonical docs parity | `#302` | Keep control-plane/matrix/lifecycle synchronized with research and issue state |

### 3.3 Safe to defer until after switch

- Readability-driven automatic adaptation tuning.
- Advanced layout portfolio expansion (beyond baseline policy set).
- Higher-order gesture optimizations (marking-menu style expert shortcuts).

---

## 4. Networking-Phase Dependency View

### 4.1 Pre-networking UX contracts

Before networking-first UX slices are expanded, ensure:

1. Faceted filtering and pane routing are canonically specified.
2. WorkbenchProfile composition semantics are explicit and serializable.
3. Multi-workbench/multi-view state semantics have deterministic harness coverage.

### 4.2 Post-networking candidates

- Network-informed collaborative UX overlays (presence/remote focus patterns).
- Tier-2-scale graph cognition aids and exploratory semantic overlays.

---

## 5. Archive-Derived Feature Candidates (Non-Authoritative Inputs)

Archive checkpoints are not canonical authorities, but they remain useful for
feature recall so migration planning does not miss prior intent.

| Archive source | Candidate capability | Lifecycle interpretation | Action |
|---|---|---|---|
| `archive_docs/checkpoint_2026-02-24/2026-02-19_graph_ux_polish_plan.md` | Secondary input-surface polish | Planned (already absorbed) | Keep mapped to active interaction-consistency + command-surface canon |
| `archive_docs/checkpoint_2026-02-24/2026-02-19_layout_advanced_plan.md` | Advanced layout/physics portfolio | Speculative/Planned hybrid | Keep as backlog input to layout portfolio spec, not direct implementation authority |
| `archive_docs/checkpoint_2026-02-25/2026-02-24_spatial_accessibility_plan.md` | Graph reader linearization ideas | Speculative (partially adopted) | Reconcile only through active accessibility + UX semantics subsystem contracts |
| `archive_docs/checkpoint_2026-02-24/2026-02-24_input_surface_polish_plan.md` | Radial/context/palette polish | Planned (absorbed redirect) | Treat as pointer to active canonical command/input specs |

---

## 6. Operational Policy

For every UX-related issue or PR touching migration scope:

1. Update this register row (`Lifecycle`, `Timing gate`, `UxTree readiness`) if semantics changed.
2. Update the feature row in `2026-03-01_ux_migration_feature_spec_coverage_matrix.md`.
3. Keep terminology synchronized with `TERMINOLOGY.md`.
4. If archive context is used, copy intent into active canon before implementation claims.

Completion rule: a capability is not considered migration-complete until both
the coverage matrix gate and this lifecycle gate are green for that row.

---

## 7. GitHub Issue Categorization (Pre/Post WGPU)

### 7.1 Pre-WGPU required (must close before switch authorization)

- **Event dispatch contract**: `#261`, `#269`
- **Radial geometry/overflow contract**: `#263`, `#270`
- **Canvas interaction invariants** (selection/lasso/zoom/edge focus): `#271`, `#173`, `#185`, `#102`, `#104`, `#101`, `#103`
- **Viewer fallback/degraded-state clarity**: `#188`, `#162`
- **UX integration deliverables**: `#292`, `#293`, `#294`, `#295`, `#296`
- **IA/predictability/discoverability bundle**: `#299`, `#300`
- **Accessibility closure bundle**: `#298`, `#301`
- **Canonical docs parity audit**: `#302`
- **UxHarness critical-path evidence**: `#251`, `#257`, `#273`
- **UxTree authority trajectory gates**: `#272`

### 7.2 Post-WGPU (defer until switch stabilized)

- **Layout readability automation depth**: `#265` follow-on slices
- **Advanced gesture and radial expert modes**: post-`#270` enhancement backlog
- **Speculative canvas interaction extensions** (SketchLay, DOI/fisheye) from research/docs backlog

### 7.3 Migration-boundary lanes (not pre-WGPU UX closure by themselves)

- Backend/render migration lanes: `#180`, `#181`, `#182`, `#183`, `#184`, `#245`

These issues remain critical overall, but do not substitute for pre-WGPU UX
contract closure.
