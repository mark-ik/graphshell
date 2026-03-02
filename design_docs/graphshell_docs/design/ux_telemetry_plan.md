# UX Telemetry Plan

**Date**: 2026-03-02  
**Status**: Canonical deliverable (D5)  
**Purpose**: Define measurable UX quality metrics using DiagnosticsRegistry-backed instrumentation.

**Related**:
- `../research/2026-03-02_ux_integration_research.md`
- `../implementation_strategy/subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../implementation_strategy/subsystem_ux_semantics/ux_scenario_and_harness_spec.md`
- `../implementation_strategy/subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`

---

## 1. Principles

- Local-first telemetry only by default; no external analytics dependency.
- Reuse DiagnosticsRegistry channels and UxProbe/UxHarness where possible.
- Each metric must define:
  - collection method,
  - channel/probe source,
  - target threshold or baseline-establishment rule,
  - implementation status (`Wired` or `Planned`).

---

## 2. Metric register

| Metric | What it measures | Collection method | Channel / Probe mapping | Target / baseline | Status |
|---|---|---|---|---|---|
| Task success rate | % of intended flows that complete successfully | CI scenario aggregation | `ux_scenario_and_harness_spec` pass ratio | Initial baseline `>= 95%` on critical-path scenario suite | Planned |
| Command abandonment rate | Palette opens dismissed without invoke | Channel counter ratio | Proposed `command.palette.opened` vs `command.palette.invoked` | Establish baseline first; target downward trend after instrumentation | Planned |
| Undo-after-action rate | Actions followed by undo within 5s | Event correlation window | `registry.action.execute_started` + undo action path events | Establish baseline; target stability band by feature family | Planned |
| Focus confusion events | Unexpected focus return/transition failures | Violation counter | `ux:navigation_violation` | Initial target `0` in critical-path CI scenarios | Wired |
| Backpressure visibility | User actions during invisible cooldown/degradation | Channel counter | Proposed `viewer.backpressure.user_action_during_cooldown`; related compositor degradation channels available (`compositor.degradation.gpu_pressure`) | Establish baseline; target monotonic decline as affordances improve | Planned |
| Accessibility invariant pass rate | % of accessibility invariants passing | Probe result ratio | UxProbe S-series + checklist-linked audits | Initial baseline measurement; target upward trend to all required invariants passing | Planned |
| First-action latency | Time from app-ready to first user action | Timestamp delta | Proposed `app.ready` and first action channel (`registry.action.execute_started`) correlation | Establish baseline p50/p95; target p95 reduction over milestone | Planned |
| Focus cycle completeness | Whether focus cycle visits required regions | Probe/scenario pass rate | `ux:navigation_transition` + UxProbe N3/N4 assertions | Initial target `100%` pass in deterministic focus-cycle scenarios | Wired (partial) |

---

## 3. End-to-end wired metric evidence

At least one metric is already wired end-to-end via diagnostics emission and test assertion:

1. **Focus confusion events**
   - Channel: `ux:navigation_violation`
   - Emission path: focus-cycle orchestration failure path.
   - Test evidence: `shell/desktop/ui/gui_orchestration_tests.rs`
     - `cycle_focus_region_emits_ux_navigation_violation_on_missing_target`

2. **Focus cycle transition success (supporting metric signal)**
   - Channel: `ux:navigation_transition`
   - Emission path: focus hint/region transition path.
   - Test evidence:
     - `cycle_focus_region_success_does_not_emit_ux_navigation_violation_channel`
     - `open_tool_pane_emits_ux_navigation_transition_channel`

This satisfies the initial D5 done-gate requirement for a wired diagnostics metric while additional channels are staged.

---

## 4. Implementation stages

### Stage A (now)

- Use existing `ux:navigation_violation` / `ux:navigation_transition` as baseline operational metrics.
- Report scenario-level counts in CI diagnostics snapshots.

### Stage B

- Add explicit command palette open/invoke channels.
- Add app lifecycle ready marker for first-action latency.
- Add backpressure user-action visibility channel.

### Stage C

- Publish aggregate trend rollups in diagnostics tooling views.
- Define pass/fail telemetry gates for milestone closure dashboards.

---

## 5. Initial implementation checklist

- [x] D5 artifact exists at `design_docs/graphshell_docs/design/ux_telemetry_plan.md`.
- [x] Every core metric has collection method + channel/probe + target/baseline note.
- [x] At least one metric is explicitly wired end-to-end with test evidence.
- [x] Plan is linked from UX execution control-plane and parity trackers.

Maintenance rule: telemetry channel or probe contract changes must update this plan and parity trackers in the same PR.