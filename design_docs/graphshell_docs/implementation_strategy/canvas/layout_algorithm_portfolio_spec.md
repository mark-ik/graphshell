# Layout Algorithm Portfolio Spec

**Date**: 2026-03-06
**Status**: Canonical portfolio contract
**Priority**: Implementation-ready

**Related**:

- `CANVAS.md`
- `layout_behaviors_and_physics_spec.md`
- `graph_node_edge_interaction_spec.md`
- `../2026-03-01_ux_migration_design_spec.md`
- `../2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
- `../system/register/canvas_registry_spec.md`
- `../system/register/physics_profile_registry_spec.md`

---

## 1. Scope

This spec canonicalizes the **layout algorithm portfolio** for graph canvases, including:

1. Formal algorithm catalog and registry ID requirements.
2. Selection policy and deterministic fallback rules.
3. Quality metrics and threshold-based acceptance gates.
4. Per-mode constraints for interaction, stability, and readability.
5. Diagnostics channels required for observability and CI enforcement.

This spec governs portfolio-level behavior. Low-level force and physics contracts remain in `layout_behaviors_and_physics_spec.md`.

---

## 2. Canonical Portfolio Catalog

The canvas layout portfolio must expose the following canonical algorithm entries.

| Canonical name | Registry ID | Category | Primary usage | Determinism |
| --- | --- | --- | --- | --- |
| Force Directed | `graph_layout:force_directed` | Dynamic physics | Interactive browse, live topology updates | Deterministic given seed + stable insertion order |
| Grid | `graph_layout:grid` | Structured placement | Dense overviews, scan-heavy tasks, debugging | Deterministic |
| Tree | `graph_layout:tree` | Topology-driven | Hierarchical/traversal-first views | Deterministic |

Any additional algorithm registered by mods must use `namespace:name` ID format and declare a compatibility profile (see §5).

---

## 3. Selection Policy

### 3.1 Selection Inputs

Portfolio selection evaluates the following ordered inputs:

1. Explicit per-view `layout_id` (highest priority).
2. Active Lens `layout_id` (if present and allowed by Lens policy).
3. Workspace default layout preference.
4. Runtime default `layout:default` resolution in `LayoutRegistry`.

If multiple inputs are present, the first valid registered ID wins.

### 3.2 Mode-Aware Recommendation Rules

Recommendation rules are advisory unless the user explicitly applies them.

| Canvas mode / workload signal | Recommended algorithm | Reason |
| --- | --- | --- |
| Live browsing with frequent node/edge creation | `graph_layout:force_directed` | Preserves spatial continuity under incremental updates |
| High-density scan / auditing layout quality | `graph_layout:grid` | Maximizes visual regularity and comparison speed |
| Directed traversal and hierarchy inspection | `graph_layout:tree` | Makes parent/child and depth relationships explicit |

### 3.3 Deterministic Fallback Rules

When the selected algorithm cannot run, fallback must be deterministic and diagnostics-backed.

1. If requested `layout_id` is unknown, fall back to resolved `layout:default`.
2. If resolved `layout:default` is unavailable, fall back to `graph_layout:force_directed`.
3. If portfolio execution fails at runtime for the selected algorithm, apply fallback without clearing graph state.
4. Fallback must not mutate user preference; it is a runtime recovery path only.

---

## 4. Quality Metrics and Thresholds

Portfolio quality checks are evaluated as post-layout metrics. Threshold values are canonical defaults and may be tightened by profile.

| Metric | Symbol | Default threshold | Intent |
| --- | --- | --- | --- |
| Edge crossing density | `crossing_density` | `<= 0.18` | Preserve readability in medium/high complexity graphs |
| Label overlap ratio | `label_overlap_ratio` | `<= 0.05` | Prevent text occlusion regressions |
| Mean edge length coefficient of variation | `edge_len_cv` | `<= 0.65` | Limit unstable spacing extremes |
| Frame displacement ratio after incremental change | `incremental_displacement_ratio` | `<= 0.30` | Preserve mental map during live updates |
| Iteration budget overrun ratio | `iteration_overrun_ratio` | `<= 0.02` | Keep layout frame-time predictable |

Metric thresholds are evaluated against graph size bands:

| Size band | Node count |
| --- | --- |
| Small | `<= 100` |
| Medium | `101..=600` |
| Large | `> 600` |

Profiles may tune thresholds by size band but must declare the override in portfolio diagnostics metadata.

### 4.1 Readability-Driven Adaptation Contract (UX migration §6.2)

Readability adaptation uses portfolio quality metrics to choose deterministic adaptation actions.

Adaptation trigger policy:

1. Evaluate `ux:layout_quality` metrics after layout execution.
2. If no threshold is violated, no adaptation is applied.
3. If thresholds are violated, apply the first eligible action in the adaptation ladder.

Adaptation ladder (first-success policy):

1. Increase spacing pressure in active algorithm/profile (non-structural adjustment).
2. Re-run current algorithm with readability-biased parameters.
3. Fall back to readability-favored algorithm recommendation for current workload (`graph_layout:grid` for dense scan; `graph_layout:tree` for hierarchy-heavy views).
4. Preserve current layout and emit explicit degraded readability warning if no action improves metrics.

Readability adaptation invariants:

- Adaptation must not mutate graph truth (`GraphId` content, node identity, edge identity).
- Adaptation must remain per-`GraphViewId`; cross-view camera/layout mutation is forbidden.
- Adaptation action selection must be deterministic for identical metric payload and profile state.
- A single evaluation cycle may apply at most one ladder step before re-measuring.

Readability diagnostics requirements:

- Every adaptation evaluation emits `ux:layout_quality`.
- Any threshold breach emits `ux:layout_quality_violation` with violated metrics and thresholds.
- Any algorithm switch caused by readability adaptation emits `ux:layout_portfolio_fallback` with `reason=readability_adaptation`.

---

## 5. Per-Mode Constraints

Each algorithm must declare and satisfy constraints for the operational modes below.

| Mode | Required constraints |
| --- | --- |
| Interactive mode | Stable incremental updates, bounded per-frame iteration cost, no hard reset on `AddNode`/`AddEdge` |
| Snapshot restore mode | Deterministic restore from persisted state; no spontaneous relayout unless explicitly requested |
| Readability audit mode | Full quality metric emission (§4) and strict threshold checking |
| Low-motion preference mode | Reduced displacement bias and conservative reheat behavior aligned with user accessibility preference |

If an algorithm does not support a required mode, it must be declared as incompatible and excluded from automatic recommendations for that mode.

---

## 6. Diagnostics Contract

The portfolio must emit these channels as part of runtime observability.

| Channel | Severity | When emitted | Required fields |
| --- | --- | --- | --- |
| `ux:layout_portfolio_selection` | `Info` | Selector resolves algorithm for a canvas | `canvas_id`, `requested_layout_id`, `resolved_layout_id`, `selection_source` |
| `ux:layout_portfolio_fallback` | `Warn` | Selector/runtime falls back from requested algorithm | `canvas_id`, `requested_layout_id`, `fallback_layout_id`, `reason` |
| `ux:layout_quality` | `Info` | Post-layout quality evaluation completes | `canvas_id`, `resolved_layout_id`, `size_band`, `metrics` |
| `ux:layout_quality_violation` | `Warn` | One or more metrics exceed thresholds | `canvas_id`, `resolved_layout_id`, `violations`, `thresholds` |
| `ux:layout_execution_failure` | `Error` | Algorithm execution fails before producing a usable layout | `canvas_id`, `requested_layout_id`, `error`, `recovery_action` |

Severity rule: execution failures are `Error`; fallback and threshold overruns are `Warn`; successful selections and quality emissions are `Info`.

---

## 7. Selection and Fallback State Machine

1. Resolve requested algorithm from selection inputs (§3.1).
2. Validate algorithm exists and is mode-compatible (§5).
3. Emit `ux:layout_portfolio_selection`.
4. Execute algorithm.
5. If execution fails or compatibility check fails:
   1. Resolve fallback chain (§3.3).
   2. Emit `ux:layout_portfolio_fallback`.
   3. Retry execution with fallback.
6. Evaluate quality metrics (§4) and emit `ux:layout_quality`.
7. If any threshold is violated, emit `ux:layout_quality_violation`.
8. If no usable layout can be produced after fallback, emit `ux:layout_execution_failure` and retain prior stable layout.

---

## 8. Acceptance Criteria

| Criterion | Verification |
| --- | --- |
| Canonical algorithms are registered and discoverable | Unit test: portfolio registry returns `graph_layout:force_directed`, `graph_layout:grid`, `graph_layout:tree` |
| Selection precedence is deterministic | Unit test: explicit view ID overrides lens/workspace/default chain |
| Unknown `layout_id` falls back via canonical chain | Unit test: unknown ID resolves to `layout:default`, then `graph_layout:force_directed` if needed |
| Fallback preserves graph state | Integration test: execution failure path keeps prior node identities/positions until fallback applies |
| Quality metrics emitted for every successful run | Scenario/assertion: `ux:layout_quality` present with full metrics payload |
| Metric threshold violation is observable | Scenario/assertion: induced overlap/crossing case emits `ux:layout_quality_violation` |
| Mode incompatibility blocks auto-recommendation | Unit test: incompatible algorithm excluded from recommendation list in target mode |
| Snapshot restore does not relayout without explicit request | Regression test: load snapshot then compare positions; no drift |
| Execution failure severity is `Error` | Diagnostics test: `ux:layout_execution_failure` channel severity is `Error` |
| Readability violation triggers deterministic adaptation ladder | Scenario/assertion: induced readability breach applies first eligible ladder step and records reason |
| Readability-driven algorithm switch is observable | Scenario/assertion: adaptation-caused switch emits `ux:layout_portfolio_fallback` with `reason=readability_adaptation` |
| Readability adaptation does not mutate graph truth | Regression test: adaptation run keeps node/edge identities unchanged |

Green-exit for UX migration §6.1 requires all criteria above to pass and diagnostics channels to be wired into CI gate suites.
