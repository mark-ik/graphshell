# UX Execution Control Plane

**Date**: 2026-03-02  
**Status**: Canonical operational control-plane  
**Purpose**: Consolidated execution surface for UX baseline done-gates, current milestone checklist, and issue-domain mapping.

**Consolidates**:
- `../../../../archive_docs/checkpoint_2026-03-01/2026-02-27_ux_baseline_done_definition.md`
- `../../../../archive_docs/checkpoint_2026-03-01/2026-02-28_current_milestone_ux_contract_checklist.md`
- `../../../../archive_docs/checkpoint_2026-03-01/2026-02-28_ux_issue_domain_map.md`

**Related**:
- `2026-02-28_ux_contract_register.md`
- `../../research/2026-03-02_ux_integration_research.md`
- `../../design/surface_behavior_spec.md`
- `../2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
- `../2026-03-01_ux_migration_lifecycle_audit_register.md`
- `ux_tree_and_probe_spec.md`
- `ux_scenario_and_harness_spec.md`

---

## 1. What this control plane owns

This document is the operational UX execution surface for three questions:

1. What defines baseline UX closure for “usable application” status?
2. What is the current milestone checklist and execution order?
3. Which open issues are primary UX work vs enabling/support lanes?

Canonical interaction semantics remain owned by the six-spec family in
`2026-02-28_ux_contract_register.md`.

---

## 2. Baseline Done-Gate (Consolidated)

UX baseline is considered closed only when all domains below pass together:

1. **Interaction correctness**
   - graph navigation, pane open/close/switch, and focus handoff are deterministic.
2. **Viewer baseline correctness**
   - baseline viewer set behaves predictably across render modes.
3. **Lifecycle and routing correctness**
   - graph/workbench routing preserves identity and authority semantics.
4. **Performance and degradation clarity**
   - fallback/degraded states are explicit and diagnostics-visible.
5. **Spec/code parity**
   - active docs match runtime behavior; no stale claim drift.

Validation gate:

- quick smoke profile passes,
- diagnostics evidence exists for compositor pass ordering/fallback visibility,
- matrix/register rows reflect current truth.

---

## 3. Current Milestone Checklist (Consolidated)

### 3.1 Priority buckets

1. **Navigation and camera**
   - `#173`, `#104`, `#101`, `#103`, `#271`
2. **Pane/workbench lifecycle**
   - `#174`, `#186`, `#187`, `#118`, `#119`
3. **Content opening and routing**
   - `#175`
4. **Command-surface unification**
   - `#176`, `#108`, `#106`, `#107`, `#178`, `#270`
5. **Settings/control surfaces**
   - `#109`, `#110`, `#177`, `#189`
6. **Selection and viewer clarity**
   - `#185`, `#102`, `#162`, `#188`
7. **UX integration deliverables and parity**
   - `#292`, `#293`, `#294`, `#295`, `#296`, `#297`, `#298`, `#299`, `#300`, `#301`, `#302`

### 3.2 Exit questions

Before milestone closure, answer “yes” to all:

1. Can users navigate graph space without control ambiguity?
2. Are pane open/close/focus flows deterministic?
3. Do content-opening actions always route through Graphshell semantics?
4. Are command surfaces semantically unified?
5. Do settings/history behave as first-class surfaces with return-path integrity?
6. Are selection + degraded/fallback states explicit and testable?

---

## 4. Issue Domain Map (Consolidated)

Issue status classes:

- **Primary**: direct UX contract behavior slice.
- **Enabling**: architecture/refactor prerequisite for a UX slice.
- **Support**: docs/diagnostics/policy reinforcement.

### 4.1 Graph / Node / Edge

- Primary: `#173`, `#104`, `#101`, `#185`, `#102`
- Enabling: `#103`, `#105`

### 4.2 Workbench / Frame / Tile

- Primary: `#174`, `#175`, `#186`, `#187`
- Enabling: `#118`, `#119`
- Support: `#100`

### 4.3 Command Surfaces

- Primary: `#176`, `#106`, `#107`, `#108`, `#178`, `#270`
- Support: `#89`

### 4.4 Focus and Region Navigation

- Primary: `#140`, `#174`, `#187`, `#189`
- Enabling: `#103`
- Support: `#138`, `#139`, `#141`, `#95`

### 4.4A UX Semantics and Automation

- Primary: `#251`, `#257`, `#269`, `#272`, `#273`
- Support: `#246`, `#247`, `#248`, `#249`, `#250`

### 4.5 Viewer Presentation and Fallback

- Primary: `#162`, `#188`, `#109`, `#111`, `#112`
- Support: `#155`, `#159`, `#92`

### 4.6 Settings and Control Surfaces

- Primary: `#109`, `#110`, `#177`, `#189`
- Support: `#89`, `#134`, `#135`, `#136`, `#137`, `#142`, `#94`

### 4.7 UX Integration Deliverables and Docs Parity

- Primary: `#292`, `#293`, `#294`, `#295`, `#296`, `#297`, `#298`, `#299`, `#300`, `#301`
- Support: `#302`

---

## 5. Deferred Migration Boundary

The following remain migration-deferred and should not be counted as immediate
UX-contract closure slices:

- `#179`, `#180`, `#181`, `#182`, `#183`, `#184`

These can influence UX readiness but do not replace baseline UX closure work.

---

## 6. Operational Update Rule

For any UX issue/PR touching current milestone semantics:

1. Update this control-plane doc when priority/status/domain changes.
2. Update `../2026-03-01_ux_migration_feature_spec_coverage_matrix.md` for spec ownership + three-tree status.
3. Update `../2026-03-01_ux_migration_lifecycle_audit_register.md` for lifecycle stage + timing gate.
4. Keep `#302` parity mapping current when UX research deliverables or closure statuses change.

A UX slice is not considered done until all three artifacts agree.
