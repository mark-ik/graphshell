<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Navigator Projection Spec

**Date**: 2026-04-21
**Status**: Canonical / Active
**Scope**: Canonical contract for how Navigator hosts derive projected views
from graph truth, workbench arrangement state, SUBSYSTEM_HISTORY-owned
aggregates, and Graph Cartography outputs. Defines the five-stage projection
pipeline, composition rules, annotation registry contract, portal gestures,
diff/animation identity, time-axis specialty behavior, layout inheritance, and
cost-class rules.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md) — Navigator domain authority and section model
- [navigator_interaction_contract.md](navigator_interaction_contract.md) — click grammar and reveal rules
- [../../../archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md](../../../archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md) — producing plan and findings
- [../subsystem_history/2026-04-21_graph_runtime_projection_layer_plan.md](../subsystem_history/2026-04-21_graph_runtime_projection_layer_plan.md) — Graph Cartography layer that fills scorer / parent-picker / annotation slots
- [../subsystem_history/SUBSYSTEM_HISTORY.md](../subsystem_history/SUBSYSTEM_HISTORY.md) — traversal-history authority and shared-projection policy
- [../subsystem_history/2026-03-18_mixed_timeline_contract.md](../subsystem_history/2026-03-18_mixed_timeline_contract.md) — mixed timeline source for time-axis and recency views
- [../../technical_architecture/domain_projection_matrix.md](../../technical_architecture/domain_projection_matrix.md) — named domain-pair projection catalog
- [../../technical_architecture/graph_tree_spec.md](../../technical_architecture/graph_tree_spec.md) — `ProjectionLens`, `LayoutMode`, `NavAction`, `TreeIntent`
- [../../technical_architecture/graphlet_model.md](../../technical_architecture/graphlet_model.md) — graphlet semantics and ownership split
- [../system/register/SYSTEM_REGISTER.md](../system/register/SYSTEM_REGISTER.md) — atomic-registry pattern used by `NodeAnnotation`
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical terms; `Lens` remains reserved for Layout+Theme+Physics+Filter

---

## 1. Authority and Boundary

Navigator is a projection-and-routing domain. This spec defines the
**projection contract** used by all Navigator hosts. It does not move policy
out of [NAVIGATOR.md](NAVIGATOR.md), redefine graph truth, or create a second
history store.

Authority split:

- **Graph truth** owns nodes, edges, relation families, graph identity, and
  canvas-space positions.
- **Workbench** owns arrangement state, frames, host geometry, and durable host
  preferences.
- **SUBSYSTEM_HISTORY** owns traversal truth, recent-history aggregation, and
  mixed timeline derivation. Navigator does not define a parallel recents log
  or index.
- **Graph Cartography** owns workspace-scoped aggregate views over graph truth +
  substrate + WAL. Navigator consumes its scorer / parent-picker / annotation
  outputs; Navigator does not reach into `NodeNavigationMemory` directly.
- **Navigator** owns projection selection, composition, host rendering, and
  routing projected interactions back to the correct authority.

Persistence rule:

- `ProjectionSpec` definitions and per-host selected projection configuration
  may persist in `WorkbenchProfile`.
- Projected outputs never persist. They are pure derivations over current
  authority inputs.

## 2. Core Terms

- **Projection Spec**: a configured five-stage pipeline.
- **Projection Composition**: composition of two or more projection specs.
- **Projection Run**: one ephemeral execution of a projection spec against the
  current authority inputs.
- **Projection Path**: the stable projected position of one item within one
  projection run. Used for duplicate-row identity and diffing.
- **Shape family**: the structural output of stage 2. Current families are
  `tree`, `list`, `graphlet`, `specialty`, and `summary`.

Terminology rule:

- `Lens` is not used here as a generic projection word. `Lens` remains the
  visual-system composition term from TERMINOLOGY.md.
- `ProjectionLens` remains the **tree-family Shape-stage mechanism** defined by
  GraphTree. It is not the full Navigator projection pipeline.

## 3. Projection Inputs

Every projection run is a pure function over:

```text
(
  graph_truth,
  workbench_arrangement_state,
  subsystem_history_projections,
  graph_cartography_views,
  projection_spec,
  host_state
) -> projected_view
```

Input contract:

- `graph_truth` provides node identity, graph relation families, tags,
  lifecycle state, and canvas positions.
- `workbench_arrangement_state` provides frame membership, open/hosted state,
  host geometry, and durable host preferences.
- `subsystem_history_projections` provides recency, mixed timeline, and other
  history-owned aggregates.
- `graph_cartography_views` provide co-activation, stable cluster membership,
  traversal centrality, repeated-path priors, frame-reformation statistics, and
  other workspace-scoped aggregates.
- `host_state` provides current host selection, active cursor, expanded rows,
  filters, density mode, and any host-local projection options.

## 4. ProjectionSpec Contract

Every `ProjectionSpec` declares:

- `id` — stable identifier
- `label` — human-readable name
- `scope` — root and inclusion rules
- `shape` — structural derivation rule
- `annotation_stack` — ordered annotation contributors and density policy
- `presentation` — intended host rendering contract
- `portal_profile` — locate / reveal / lift behavior
- `layout_inheritance` — `own | canvas | canvas-compressed`
- `cost_class` — `live | debounced | on-demand`
- `conflict_mode` — `hint | hide` when composed scopes disagree
- `diff_policy` — animation and identity rules

`ProjectionSpec` is declarative. Implementation carriers may differ by UI
framework, but the spec-level behavior must remain stable across hosts.

## 5. Five-Stage Projection Pipeline

Every Navigator output passes through the same five stages.

### 5.1 Scope

Scope selects the subgraph or subspace under consideration.

Canonical scope sources:

- active node
- anchor set
- frame
- cluster
- time window
- recent session
- user selection
- full graph

Scope rules:

- Scope selects **members**, not presentation.
- Scope may be sourced from graph truth, workbench state, SUBSYSTEM_HISTORY
  aggregates, or Graph Cartography aggregates.
- Scope does not materialize or persist rows by itself.
- When composed scopes disagree, outer scope wins and inner-only members are
  surfaced per `conflict_mode`.

### 5.2 Shape

Shape turns the scoped candidate set into a readable structure.

Canonical shape families:

- **Tree** — one primary parent per projected item, with suppressed edges
  surfaced by annotations.
- **List** — ranked or sectioned flat slice.
- **Graphlet** — local graph projection retaining graph topology.
- **Specialty** — purpose-driven layout such as constellation, corridor,
  atlas, or time-axis.
- **Summary** — compressed orienting surface such as overview swatch or
  minimap-like summary.

Tree-family rule:

- GraphTree `ProjectionLens` is the tree-family Shape-stage mechanism.
- `ProjectionLens` variants choose which relation family drives parent-child
  within a tree-shaped projection.
- `LayoutMode` is a tree-family presentation mechanism, not a scope or
  annotation mechanism.

Non-tree rule:

- Graphlets, time-axis, summary/minimap, and other non-tree outputs use their
  own Shape-stage mechanisms while still obeying the same pipeline contract.

### 5.3 Annotation

Annotation surfaces discarded or secondary structure as compact hints rather
than pretending it is part of the primary structure.

Rules:

- Annotations are registry-contributed `NodeAnnotation` implementations.
- Annotation density is determined by the active `annotation_stack`, not by a
  global toggle.
- Authored structure and inferred structure must remain visually distinct.
- Graph Cartography aggregates render as annotations unless and until promoted
  into durable graph relations by the appropriate authority.

Built-in annotation families:

- cross-link count
- cluster membership
- frame membership
- recency / activity heat
- trust / permission summary
- focused-content status
- hidden-neighbors count
- "also in" cross-references
- co-activation summary
- bridge / connector emphasis

### 5.4 Presentation

Presentation renders the shaped + annotated projection into a Navigator host.

Rules:

- Navigator remains one domain with many hosts; a projection host is not a
  second Navigator instance.
- Presentation may vary by host geometry and chrome capacity, but it may not
  change the semantic meaning of the projection.
- Host-local state may affect density, row expansion, diff policy, and
  animation, but not truth ownership.
- `layout_inheritance` controls whether the projection owns its own positions or
  follows canvas-derived positions.

### 5.5 Portal

Portal defines how the user moves from the projected view back into graph or
workbench context.

The portal gestures are:

- **Locate** — center or pan the authoritative graph/canvas to the selected
  node, preserve surrounding context, and highlight the target.
- **Reveal-in-place** — expand hidden structure inside the projection without
  moving the graph camera or changing authoritative selection.
- **Lift** — spill a projected subview into an ephemeral canvas/workbench
  overlay while preserving the projection's own shape.

Portal rules:

- Portal gestures route through graph or workbench intents, not Navigator-local
  direct mutation.
- `Lift` is ephemeral by default. Durable promotion is a separate explicit
  graphlet-fork or workbench action.
- `Locate` always resolves against graph-truth identity, never row position.

## 6. Projection Composition

Projection composition combines specs without overloading the word `Lens`.

Composition contract:

- The **inner spec** produces a candidate member set and a primary structure.
- The **outer spec** may constrain scope, regroup, reorder, or restyle the
  inner result.
- If outer and inner scopes disagree, outer scope wins.
- Members excluded by the outer scope render as annotation hints by default.
  `conflict_mode: hide` suppresses them entirely.
- Invalid compositions are rejected at spec-validation time with
  `whyInvalid`.

Valid v1 compositions:

- `cluster` x `recency-scorer`
- `frame-scope` x `graphlet`
- `time-window` x `constellation`
- `cluster` x `constellation`
- `frame-scope` x `time-axis`

Invalid v1 examples:

- a tree-only `ProjectionLens` applied directly to a graphlet-as-graph shape
- a host requiring `canvas-compressed` layout inheritance when the composed
  inner shape declares `own` and forbids compression

## 7. Projection Source Inventory

To keep domain-pair boundaries explicit, every pipeline stage reads from a
named projection source.

| Pipeline stage | Canonical source families | Notes |
|---|---|---|
| Scope | graph truth, arrangement projection, mixed timeline projection, Graph Cartography projection | Outer scope wins in composition |
| Shape | GraphTree `ProjectionLens` for trees; graphlet model for graphlets; specialty shape mechanisms for constellation / corridor / atlas / time-axis / summary | Tree and non-tree mechanisms stay distinct |
| Annotation | `NodeAnnotation` registry + Graph Cartography aggregate views + Navigator-owned trust/focused-content policies | Aggregates are annotations, not edges |
| Presentation | host geometry + GraphTree renderer adapters + host-local chrome rules | One Navigator, many hosts |
| Portal | `NavAction`, `TreeIntent`, graph/workbench intents | Routes back to authority, never local mutation |

## 8. NodeAnnotation Registry Contract

`NodeAnnotation` follows the atomic-registry pattern.

Each contributor declares:

- `id`
- `label`
- `source_family`
- `density_class` — `minimal | compact | expanded`
- `budget_class` — `cheap | moderate | expensive`
- `cost_class` — `live | debounced | on-demand`
- `render_capabilities` — icon, chip, halo, inline text, hover detail
- `compute(view_inputs, projection_item) -> annotation_payload`

Registry rules:

- `live` specs may include only `cheap` contributors unless the host opts into a
  higher budget.
- `moderate` and `expensive` contributors must degrade or defer when host
  geometry or projection `cost_class` does not support them.
- Registry contributors may not mutate graph truth, workbench state, or
  Navigator selection.

## 9. Identity and Projection Diff

Projection transitions must animate as a diff, not teleport.

Identity rules:

- Duplicate-row identity keys are `(node_id, projection_path)`, not `node_id`
  alone.
- `Locate` resolves by `node_id`.
- `Reveal-in-place` and `Lift` resolve by the clicked `projection_path`.

Diff rules:

- Shared items tween position, grouping, and annotation state.
- Entering and leaving items animate from nearest surviving neighbors when
  possible, else from host-edge fallbacks.
- Default timing envelope is `180 ms ease-out`.
- Hosts may override timing only if they preserve semantic ordering and do not
  block refresh publication.
- Refresh-triggered reruns may animate delta-only; explicit user projection
  switches animate full diffs.

Performance rule:

- Projection diff must not block signal publication or projection refresh
  routing. Hosts may degrade animation fidelity before they delay the refresh
  path.

## 10. Time-Axis Specialty Projection

Time-axis is a first-class specialty projection family, not a second Navigator
instance.

Rules:

- It consumes `mixed_timeline_entries` exclusively for temporal ordering.
- The cursor is host-local by default.
- Hosts may optionally bind to a shared time cursor later, but per-host cursor
  is the v1 contract.
- Time-axis may be composed as an outer spec that constrains downstream scope to
  a time window.
- Time-axis preserves the one-Navigator-many-hosts rule.

Time-axis output may appear as:

- a scrubbable lane in a sidebar host
- a compact strip in a toolbar host
- an overlay guide paired with a graphlet or constellation host

## 11. Layout Inheritance

Every spec declares one layout inheritance mode:

- `own` — projection computes its own positions. Applies to lists, trees,
  time-axis, and most sectioned views.
- `canvas` — projection inherits current canvas coordinates. Applies to
  graphlets and other graph-native local views.
- `canvas-compressed` — projection inherits canvas coordinates but renders a
  compressed minimap or summary form.

Rules:

- `canvas` and `canvas-compressed` specs may refresh off canvas-layout ticks.
- `own` specs may debounce independently of canvas motion.
- Layout inheritance is explicit so hosts can make cost and animation choices
  without guessing.

## 12. Cost Classification

Every spec declares a cost class and update strategy.

Cost classes:

- `live` — reruns on every relevant refresh trigger
- `debounced` — reruns on cadence or batched trigger windows
- `on-demand` — reruns only when explicitly opened, switched to, or refreshed

`live` sub-classification:

- `incremental` — applies deltas from refresh payloads
- `recompute` — reruns from scratch

Rule:

- A `live` spec with non-trivial complexity must be incremental or be demoted
  to `debounced`.

## 13. Refresh Triggers

Navigator projection refresh is signal-driven, not observer-sprawl.

Canonical refresh sources:

- graph node add/remove/update
- graph edge assertion/removal
- arrangement mutation
- host geometry changes that affect density or compression
- mixed timeline updates
- Graph Cartography aggregate invalidations
- host-local cursor / filter / projection changes

Trigger rules:

- Refresh triggers rerun the current spec against authority inputs.
- Projection outputs never become authority just because they are currently
  displayed.
- Hosts may debounce presentation work; they may not fork their own truth store.

## 14. Initial Built-In Projection Specs

The v1 contract assumes these built-in spec families exist:

- **History tree**
  - Scope: recent session, anchor set, or selected branch
  - Shape: tree via `ProjectionLens`
  - Sources: SUBSYSTEM_HISTORY + graph truth
- **Recency list**
  - Scope: recent window or recent session
  - Shape: ranked list
  - Sources: SUBSYSTEM_HISTORY, optionally Graph Cartography recency scoring
- **Frame-scoped graphlet**
  - Scope: one frame
  - Shape: graphlet
  - Sources: graph truth + arrangement projection
- **Cluster view**
  - Scope: stable cluster or cluster set
  - Shape: grouped tree, sectioned list, or constellation-family specialty view
  - Sources: Graph Cartography stable cluster aggregates
- **Time-axis view**
  - Scope: mixed timeline window
  - Shape: specialty temporal layout
  - Sources: SUBSYSTEM_HISTORY mixed timeline projection
- **Overview swatch**
  - Scope: whole graph or explicit filtered graph subset
  - Shape: summary
  - Sources: graph truth; may consume Graph Cartography heat / hotspot overlays

## 15. Non-Goals

This spec does not:

- redefine graph truth or edge taxonomy
- authorize Navigator to create a parallel recents or activity store
- collapse authored and inferred relations into one visual language
- make Graph Cartography aggregates into durable edges by default
- define workbench arrangement mutation policy
- overload `Lens`

## 16. Implementation Notes

- Tree-family implementations should continue to use `ProjectionLens`,
  `LayoutMode`, `NavAction`, and `TreeIntent` from GraphTree as the concrete
  Shape / Presentation / Portal mechanisms for tree outputs.
- Non-tree Shape-stage mechanisms remain separate implementations under the
  same pipeline contract.
- Any new domain-pair projection introduced for Navigator should be added to
  [domain_projection_matrix.md](../../technical_architecture/domain_projection_matrix.md)
  in the same session.
