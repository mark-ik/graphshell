# Faceted Filter Surface Spec

**Date**: 2026-03-06
**Status**: Canonical interaction contract
**Priority**: Immediate implementation guidance

**Related**:

- `GRAPH.md`
- `graph_node_edge_interaction_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `../2026-03-01_ux_migration_design_spec.md`
- `../2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../subsystem_ux_semantics/ux_scenario_and_harness_spec.md`
- `../../TERMINOLOGY.md`

---

## 1. Scope

This spec defines the canonical contract for faceted filtering on graph nodes.

It covers:

1. PMEST-aligned node facet schema and namespaced extension rules.
2. Filter operator semantics and composition model.
3. Index/evaluation strategy and Graph authority boundaries.
4. Result model for visibility, grouping, and Lens integration.
5. Omnibar and command-surface integration contracts.
6. UxTree exposure, diagnostics channels, and acceptance criteria.

This spec defines filter semantics. It does not redefine graph truth ownership,
tile arrangement ownership, or command meaning ownership.

---

## 2. Three-Tree Authority Contract

### 2.1 Graph Tree authority

- Facet values and filter truth are derived from graph-owned node/edge metadata.
- Filter evaluation runs through reducer-owned intent paths.
- Filter output is a projection over graph truth, never a graph identity mutation.

### 2.2 Workbench Tree authority

- Workbench owns only presentation of facet surfaces (pane hosting, split/tab placement).
- Workbench does not own facet-value truth or filter evaluation semantics.

### 2.3 UxTree contract

- Facet controls, active filter chips, and result-count status are emitted as `UxNode`s.
- Ux actions for add/remove/toggle/clear filters are exposed through UxTree actions.
- Ux probe scenarios must verify equivalent behavior across keyboard, command surfaces,
  and omnibar-triggered filter routes.

---

## 3. Canonical Facet Projection Schema

Every graph node has a logical PMEST facet projection composed from durable node
data plus graph/workbench/runtime state.

This spec does not define the canonical node datastructure. It defines the
queryable projection used for filtering. The expected layering is:

- node data fields = durable source of truth
- facet projection = reducer-evaluated PMEST query surface
- presentation facet/view mode = runtime rendering/presentation choice

Several canonical facet keys are intentionally derived projections rather than
stored node fields, including `domain`, `viewer_binding`, `edge_kinds`,
`in_degree`, `out_degree`, `frame_memberships`, `udc_classes`, and
`spatial_cluster`.

| PMEST facet | Canonical facet keys | Source authority |
| --- | --- | --- |
| Personality | `address_kind`, `domain`, `title`, `address` | Node identity metadata |
| Matter | `mime_hint`, `viewer_binding`, `content_length` | Node content/viewer metadata |
| Energy | `edge_kinds`, `traversal_count`, `in_degree`, `out_degree` | Edge/traversal projections |
| Space | `frame_memberships`, `frame_affinity_region`, `udc_classes`, `spatial_cluster` | Workbench/knowledge/layout projections |
| Time | `created_at`, `last_traversal`, `lifecycle` | Temporal/lifecycle metadata |

Extension rule:

- Additional facets must use namespaced keys: `namespace:name`.
- Non-namespaced extension keys are invalid and must emit validation diagnostics.
- New durable node fields do not automatically become canonical facet keys; add
  them here only when they support filter/group/route semantics.

---

## 4. Filter Query Model

### 4.1 Canonical query shape

Faceted filtering evaluates a composable predicate expression over node facets:

`FacetExpr = FacetPredicate | And(Vec<FacetExpr>) | Or(Vec<FacetExpr>) | Not(Box<FacetExpr>)`

### 4.2 Predicate operators

| Operator | Semantics |
| --- | --- |
| `Eq` | facet value equals operand |
| `NotEq` | facet value not equal to operand |
| `In` | facet value is one of operand values |
| `ContainsAny` | collection facet overlaps operand set |
| `ContainsAll` | collection facet contains full operand set |
| `Range` | scalar facet between inclusive bounds |
| `Exists` | facet key present |
| `NotExists` | facet key absent |

Operator invariants:

- Operator/type mismatch must not panic; it resolves to no match and emits `Warn` diagnostics.
- `Range` on non-ordered types is invalid.
- `ContainsAny` and `ContainsAll` require collection-valued facets.

---

## 5. Index and Evaluation Strategy

### 5.1 Evaluation authority

- Filter evaluation executes through reducer-owned intents.
- UI surfaces submit filter intents; they do not compute authoritative result sets.

### 5.2 Index model

- Facet index scope is `GraphId` with optional `GraphViewId` query projection.
- Index updates are incremental on node/edge/traversal/lifecycle changes.
- Index rebuild on startup must be deterministic for identical persisted graph state.

### 5.3 Lens integration contract

- Active faceted filters are part of Lens composition:
  `Lens = Layout × Theme × Physics Profile × Faceted Filter Set`.
- Applying a Lens may set/replace the current `Faceted Filter Set` by policy.
- Clearing filters must not mutate node/edge identity, positions, or frame membership.

---

## 6. Result Model

Filter evaluation produces a projection result for visible graph state.

| Result field | Meaning |
| --- | --- |
| `matched_nodes` | Node keys satisfying `FacetExpr` |
| `filtered_out_nodes` | Node keys excluded by current filters |
| `facet_counts` | Per-facet bucket counts for visible scope |
| `result_scope` | `GraphId` and optional `GraphViewId` |
| `degraded_reason` | Optional reason when partial data prevents full evaluation |

Result invariants:

- Filtering hides/de-emphasizes nodes in presentation; it does not delete/tombstone nodes.
- Selection must remain stable when selected nodes become filtered out; they move to hidden-selected state until filters change.
- Traversal history and edge records remain unchanged by filtering.

---

## 7. Surface Integration

### 7.1 Omnibar integration

- Omnibar supports facet query entry with explicit facet tokens.
- Enter on a valid facet query applies/updates active filter set.
- Invalid tokens are rejected with explicit feedback and no silent partial parse.

### 7.2 Command surface integration

- Search Palette Mode, Context Palette Mode, and Radial Palette Mode may invoke
  facet actions, but all route to one filter authority.
- Command-surface visibility rules may vary by context; semantics may not.

### 7.3 Facet pane entry integration

- If exactly one node is selected, command surfaces may route to facet-pane flows
  defined in `facet_pane_routing_spec.md`.
- Multi-node selection disables node-specific facet-pane routing actions and shows reason text.

---

## 8. UxTree and Diagnostics

### 8.1 UxTree requirements

- Facet filter pane exposes:
  - facet group nodes,
  - active filter-chip nodes,
  - apply/clear nodes,
  - result summary node.
- Every interactive facet control exposes name/role/value and enabled state.

### 8.2 Diagnostics channels

| Channel | Severity | Required fields |
| --- | --- | --- |
| `ux:facet_filter_applied` | `Info` | `graph_id`, `graph_view_id`, `expr_hash`, `result_count` |
| `ux:facet_filter_cleared` | `Info` | `graph_id`, `graph_view_id`, `cleared_count` |
| `ux:facet_filter_invalid_query` | `Warn` | `graph_id`, `query`, `reason` |
| `ux:facet_filter_type_mismatch` | `Warn` | `facet_key`, `operator`, `value_type` |
| `ux:facet_filter_eval_failure` | `Error` | `graph_id`, `expr_hash`, `error`, `recovery_action` |

Severity rule: evaluation/runtime failures are `Error`; invalid user queries and
type mismatches are `Warn`; successful apply/clear operations are `Info`.

---

## 9. Acceptance Criteria

| Criterion | Verification |
| --- | --- |
| PMEST canonical facets are queryable | Unit test: each PMEST facet key resolves and filters deterministically |
| Namespaced extension keys enforced | Unit test: extension key without `namespace:name` is rejected with `Warn` diagnostic |
| Operator semantics are type-safe | Unit test: invalid operator/type combinations return empty match + mismatch diagnostic |
| Reducer owns filter truth | Integration test: UI submits intent; reducer result drives visible projection |
| Filtering does not mutate graph truth | Regression test: node/edge identity and lifecycle unchanged across apply/clear |
| Lens composition includes facet set | Integration test: Lens apply restores expected facet filters |
| Omnibar facet query parity with command surfaces | Scenario test: equivalent query via omnibar and palette yields identical result set |
| Active filter controls emitted in UxTree | Probe test: facet controls/chips/status appear with expected roles/actions/states |
| Diagnostics channel severity contract holds | Diagnostics test: invalid query is `Warn`, evaluation failure is `Error` |

Green-exit for UX migration §4.1 and §4.2 requires all criteria above plus UxHarness
coverage for omnibar and command-surface entry paths.
