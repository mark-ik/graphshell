<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Semantic Clustering Follow-On Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the semantic-clustering lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan that bridges semantic enrichment, out-of-band clustering computation, and graph layout consumption.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-03-11_graph_enrichment_plan.md`
- `force_layout_and_barnes_hut_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `2026-04-03_layout_backend_state_ownership_plan.md`
- `2026-04-03_layout_variant_follow_on_plan.md`
- `2026-04-03_wasm_layout_runtime_plan.md`

---

## Context

Semantic clustering already exists in Graphshell, but only as a partial cross-lane capability:

- the enrichment lane already owns semantic provenance, classification, and user-facing
  explanation requirements
- the force-layout lane already treats semantic clustering as a Graphshell-owned extension force
- the physics extensibility umbrella note sketches future algorithmic follow-ons such as k-means,
  DBSCAN, and embedding-driven grouping

What is still missing is a dedicated execution lane for the part in the middle:

- what semantic inputs are allowed to drive clustering
- how cluster assignments are computed, invalidated, and diagnosed
- how those assignments affect layout without becoming a hidden source of graph truth

This plan exists so semantic clustering is no longer split between "enrichment someday" and
"physics helper already exists" with no authority for the actual bridge.

---

## Non-Goals

- treating semantic clustering as a graph-canonical mutation
- deepening hidden runtime-only semantic state without explanation or provenance
- making ML embeddings a prerequisite for all semantic grouping behavior
- replacing domain clustering, frame-affinity behavior, or other existing layout helpers
- turning this lane into a general-purpose model-serving or vector-search plan

---

## Feature Target 1: Define the Semantic Input Contract

### Target 1 Context

The first missing decision is what data semantic clustering is allowed to consume. The umbrella
note points at burn embeddings and UDC similarity; the enrichment lane already requires provenance,
confidence, and user-facing explanation.

### Target 1 Tasks

1. Define the allowed semantic inputs for clustering in priority order:
   embeddings when available, classification/tag similarity when not, and explicit fallback rules.
2. Require every clustering input source to remain attributable to enrichment metadata rather than
   hidden renderer or physics state.
3. Define whether clustering operates on per-node vectors, pairwise similarity tables, or both.
4. Specify how cluster inputs are invalidated when node content or semantic metadata changes.

### Target 1 Validation Tests

- Clustering can explain which semantic source produced a grouping decision.
- Missing embeddings degrade to a documented fallback path rather than disabling the whole lane.
- Input invalidation triggers only when the relevant semantic data changes.

---

## Feature Target 2: Land the Out-Of-Band Clustering Pipeline

### Target 2 Context

The umbrella note explicitly places clustering computation out-of-band rather than inside the
per-frame physics step. That boundary is important for both performance and inspectability.

### Target 2 Tasks

1. Define a background or on-demand clustering pipeline that computes cluster assignments outside
   the interactive layout step.
2. Start with a bounded first-slice algorithm choice and leave richer alternatives such as DBSCAN
   as later admissions rather than day-one complexity.
3. Produce a stable cluster-assignment artifact keyed by `GraphViewId` and node identity.
4. Keep clustering recomputation policy explicit: manual refresh, data-change invalidation, or
   bounded automatic recompute.

### Target 2 Validation Tests

- Cluster assignments are stable for identical inputs.
- Recompute cadence is explicit and diagnosable.
- Background clustering cannot directly mutate graph truth or bypass reducer-owned enrichment.

---

## Feature Target 3: Define Layout Consumption Rules

### Target 3 Context

The force-layout spec already says semantic clustering is a post-step extension force. This plan
needs to define how richer cluster assignments feed that force without becoming a second hidden
layout engine.

### Target 3 Tasks

1. Define how cluster assignments feed layout behavior: centroid targets, affinity groups, or
   other explicit extension-force inputs.
2. Keep semantic clustering independent from domain clustering and frame-affinity behavior, while
   allowing them to compose predictably.
3. Define profile and diagnostics surfaces for enabling, weighting, or disabling semantic
   clustering effects.
4. Ensure semantic clustering remains a toggleable behavioral consumer rather than an always-on
   replacement for baseline layout semantics.

### Target 3 Validation Tests

- Enabling semantic clustering measurably changes related-node positions.
- Disabling the feature removes its spatial effect without altering graph truth.
- Semantic clustering composes with domain clustering rather than silently overriding it.

---

## Feature Target 4: Make The Results Explainable And User-Visible

### Target 4 Context

The enrichment umbrella already sets the prototype rule: explain before automate. Semantic
clustering should not become a hidden grouping engine that users cannot inspect, reject, or reason
about.

### Target 4 Tasks

1. Expose requested vs resolved semantic clustering state through diagnostics.
2. Provide user-facing explanation hooks for why nodes are being grouped semantically.
3. Keep clustering provenance aligned with the enrichment inspector/filter surfaces instead of a
   physics-only debug panel.
4. Define how suggested or low-confidence semantic inputs affect clustering policy.

### Target 4 Validation Tests

- A user can inspect why a node is participating in a semantic cluster.
- Diagnostics distinguish semantic clustering from other organizer helpers.
- Low-confidence or missing semantic inputs degrade according to documented policy.

---

## Exit Condition

This plan is complete when Graphshell has a documented and testable semantic clustering pipeline
that starts from attributable semantic inputs, computes cluster assignments out-of-band, feeds them
into explicit layout behavior, and exposes the result through both diagnostics and enrichment-facing
explanation surfaces.
