# Facet Pane Routing Spec

**Date**: 2026-03-06
**Status**: Canonical interaction contract
**Priority**: Immediate implementation guidance

**Related**:

- `faceted_filter_surface_spec.md`
- `graph_node_edge_interaction_spec.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../subsystem_ux_semantics/ux_scenario_and_harness_spec.md`
- `../2026-03-01_ux_migration_design_spec.md`
- `../2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
- `../../TERMINOLOGY.md`

---

## 1. Scope

This spec defines the canonical contract for facet-rail interaction and
Enter-to-pane routing when a single node is selected.

It covers:

1. Facet rail activation and navigation semantics.
2. Pane target resolution and route ownership.
3. Focus transfer and deterministic focus return.
4. UxTree roles/actions/states for facet-rail surfaces.
5. Degraded behavior, diagnostics, and acceptance criteria.

This spec governs routing behavior for node-facet pane entry. It does not
redefine graph truth, command meaning, or workbench arrangement semantics.

---

## 2. Three-Tree Authority Contract

### 2.1 Graph Tree authority

- Node identity (`NodeKey`) and facet payload truth come from graph-owned data.
- Facet-pane routes are parameterized by `NodeKey` and facet kind.
- Routing does not mutate graph identity.

### 2.2 Workbench Tree authority

- Workbench resolves pane destination and arrangement policy (reuse active pane,
  open split, or open tab) based on workbench routing rules.
- Workbench owns pane hosting lifecycle after route resolution.

### 2.3 UxTree contract

- Facet rail appears as a semantic region with explicit selectable facet items.
- Current facet selection, enter action availability, and disabled reasons are
  exposed via Ux state/value fields.
- Probe coverage must validate focus return and route parity.

---

## 3. Facet Rail Interaction Contract

Facet rail is available only when exactly one node is selected.

| Action | Primary binding | Semantic result |
| --- | --- | --- |
| Enter facet-rail mode | `F` (single-node selection) | Active node enters rail focus context |
| Next facet | `Right` or `Down` | Selection advances `Personality -> Matter -> Energy -> Space -> Time` |
| Previous facet | `Left` or `Up` | Reverse cycle |
| Open selected facet pane | `Enter` | Route selected node/facet to pane destination |
| Exit facet-rail mode | `Escape` | Return to prior selection/navigation context |

Input-context rule:

- While facet-rail mode is active, arrow keys are consumed by facet navigation.
- Camera pan/other arrow-bound actions are suppressed in this context.

---

## 4. Pane Route Targets

Each facet maps to a canonical pane target profile.

| Facet | Route key | Canonical pane target |
| --- | --- | --- |
| Personality | `facet:personality` | Node identity/address pane mode |
| Matter | `facet:matter` | Node metadata/details pane mode |
| Energy | `facet:energy` | Edge/traversal summary pane mode |
| Space | `facet:space` | Membership/tag/region pane mode |
| Time | `facet:time` | Node timeline/history pane mode |

General-pane invariant:

- Targets are generic pane types parameterized by `NodeKey` and route key.
- Per-node bespoke pane classes are non-canonical.

---

## 5. Destination Resolution Contract

On `Enter`, route resolution executes in this order:

1. Validate exactly one selected node exists and is still resolvable.
2. Read active facet rail item (`facet:*` route key).
3. Resolve destination policy via command/workbench route authority.
4. Open or focus pane destination with `NodeKey` + facet route payload.
5. Transfer focus to pane root and preserve return anchor.

Destination policies must remain deterministic for identical route context.

Route payload minimum:

- `node_key`
- `facet_route_key`
- `source_surface` (for diagnostics and return-path metadata)
- `source_focus_anchor`

---

## 6. Focus and Return Path Contract

### 6.1 On successful route

- Focus moves to the opened/focused pane root element.
- Return anchor stores prior graph surface + node + facet-rail selection state.

### 6.2 On dismiss/back

- Focus returns to stored anchor if still valid.
- If anchor is invalid, deterministic fallback applies:
  - active graph pane root,
  - then active workbench frame root.

### 6.3 Failure behavior

- If route preconditions fail, facet rail remains active and focused.
- Failure reason is explicit in UI and diagnostics.

---

## 7. UxTree Contract

Facet rail semantic nodes:

| Ux node role | Required fields |
| --- | --- |
| `Region` (`facet-rail`) | `label`, `focused`, `active_facet` |
| `List` (facet items) | ordered PMEST list |
| `ListItem` (facet item) | `selected`, `enabled`, `hint` |
| `Button` (`open-facet-pane`) | enabled only when route preconditions pass |
| `StatusIndicator` (route status) | success/failure/blocked reason |

Ux action contract:

- `SelectNextFacet`
- `SelectPreviousFacet`
- `OpenFacetPane`
- `ExitFacetRail`

These actions must be invokable through UxBridge for scenario testing.

---

## 8. Degraded and Blocked States

| Condition | Required behavior |
| --- | --- |
| No selected node | Rail cannot activate; show explicit reason |
| Multi-select active | Rail actions disabled with explanation |
| Selected node tombstoned/invalid before Enter | Route rejected; keep rail focus and show reason |
| Destination pane type unavailable | Route rejected with fallback suggestion and diagnostics |
| Focus return anchor invalid | Deterministic fallback target (§6.2) |

Silent no-op behavior is forbidden for all blocked/degraded routes.

---

## 9. Diagnostics Contract

| Channel | Severity | Required fields |
| --- | --- | --- |
| `ux:facet_rail_entered` | `Info` | `node_key`, `active_facet` |
| `ux:facet_rail_navigate` | `Info` | `node_key`, `from_facet`, `to_facet` |
| `ux:facet_pane_route_attempt` | `Info` | `node_key`, `facet_route_key`, `destination_policy` |
| `ux:facet_pane_route_blocked` | `Warn` | `node_key`, `facet_route_key`, `reason` |
| `ux:facet_pane_route_failed` | `Error` | `node_key`, `facet_route_key`, `error`, `recovery_action` |
| `ux:facet_pane_focus_return` | `Info` | `from_route`, `return_target`, `fallback_used` |

Severity rule: blocked preconditions are `Warn`; runtime route failures are
`Error`; normal interaction/transition events are `Info`.

---

## 10. Acceptance Criteria

| Criterion | Verification |
| --- | --- |
| Facet rail activates only for single-node selection | Scenario test: single select succeeds; zero/multi-select blocked with reason |
| Arrow navigation cycles PMEST order deterministically | Unit/scenario test: forward/reverse wrap behavior is stable |
| Enter routes to canonical pane target per facet | Integration test: each facet opens/focuses expected pane mode |
| Route payload includes required fields | Unit test: dispatch payload contains `node_key`, `facet_route_key`, source metadata |
| Focus transfer occurs on successful route | Probe test: pane root gets focus after Enter |
| Escape exits rail and restores prior context | Scenario test: exit returns to prior graph selection context |
| Dismiss/back from pane restores anchor | Scenario test: close pane returns focus to graph anchor or deterministic fallback |
| Blocked routes never silently no-op | Diagnostics test: blocked route emits `ux:facet_pane_route_blocked` |
| UxTree exposes rail roles/actions/states | Probe test: facet rail nodes and actions are present and invokable |
| Channel severities match contract | Diagnostics test: blocked=`Warn`, failed=`Error`, normal interactions=`Info` |

Green-exit for UX migration §4.3 and §5.1A requires all criteria above and
UxHarness scenario evidence for success, blocked, and fallback-return flows.
