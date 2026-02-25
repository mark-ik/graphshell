# Layout Behaviors & Spatial Organization Plan (2026-02-24)

**Status**: Implementation-Ready
**Supersedes**: `2026-02-19_layout_advanced_plan.md`
**Relates to**: `2026-02-24_performance_tuning_plan.md` (culling/perf ownership), `2026-02-23_graph_interaction_consistency_plan.md` (viewport gravity), `2026-02-23_udc_semantic_tagging_plan.md` (semantic clustering), `2026-02-24_physics_engine_extensibility_plan.md` (physics engine architecture, ExtraForce extension model, thematic preset designs).

## Context

This plan covers *behavioral layout features* (how the graph arranges itself), not frame-time optimization.

---

## Scope and Constraints

1. Keep force-behavior toggles policy-driven (`CanvasRegistry`), not hardcoded in render callsites.
2. Preserve existing intent and lifecycle boundaries (`apply_intents` -> reconcile -> render).
3. Treat this plan as additive to current physics system; avoid introducing a parallel layout engine.

---

## Phase 1: Physics Micro-Behaviors

### 1.1 Reheat on Structural Change

**Problem**: Adding nodes/edges while paused leaves newly changed topology visually inert.

**Plan**:

- In `apply_intents()`, when `AddNode` or `AddEdge` occurs (excluding snapshot load/replay), set `physics.is_running = true`.
- Do not zero or reset velocity state; only resume simulation.

### 1.2 New Node Placement Near Semantic Parent

**Problem**: Center-spawned nodes break local context during navigation growth.

**Plan**:

- If node creation has a semantic source/parent, place at `parent_position + jitter`.
- Ensure create-new event path carries parent identity (`GraphSemanticEvent::CreateNewWebView` -> parent node reference).

### 1.3 Gravity Parameter Consistency

**Problem**: Gravity behavior can drift from configured profile parameters.

**Plan**:

- Verify viewport-gravity implementation consumes current profile gravity strength.
- Keep authoritative behavior aligned with `2026-02-23_graph_interaction_consistency_plan.md`.

---

## Phase 2: Advanced Force Injection Hook

**Prerequisite**: Provide post-physics, pre-render injection hook (e.g., `apply_post_frame_layout_injection(...)`).

### 2.1 Degree-Dependent Repulsion

**Goal**: De-clutter high-degree hub topologies.

**Plan**:

- Compute per-node degree bonus (e.g., `log(degree) * k`).
- Apply additional separation force to nearby neighbors.
- Gate by `CanvasRegistry.degree_repulsion_enabled`.

### 2.2 Domain Clustering Force

**Goal**: Pull same-domain nodes into soft spatial neighborhoods.

**Plan**:

- Group nodes by eTLD+1.
- Compute group centroids.
- Apply weak attraction toward domain centroid.
- Gate by `CanvasRegistry.domain_clustering_enabled`.

**Note**: This is independent from UDC semantic clustering; both may be enabled together.

---

## Phase 3: Magnetic Zones

### 3.1 Data Model

- `Zone { id, name, centroid, strength }`
- `GraphWorkspace.zones` persisted in snapshots
- `node.zone_id: Option<Uuid>` membership pointer

### 3.2 Zone Force Application

- For zone-bound nodes, apply attraction to zone centroid during layout injection.
- Render subtle zone backdrop (member bounds + padding) for spatial affordance.

### 3.3 User Interaction

- Create Zone: from selected nodes, derive initial centroid.
- Drag Zone: move centroid; member nodes follow by soft force.

---

## Validation

- [ ] Paused physics resumes automatically when structural intents add nodes/edges.
- [ ] Link-triggered/new-context node spawns near source node.
- [ ] Degree repulsion toggle changes hub spread behavior measurably.
- [ ] Domain clustering toggle creates visible same-domain grouping.
- [ ] Zone create/drag updates node spatial behavior and survives snapshot roundtrip.
