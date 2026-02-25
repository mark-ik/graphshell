# Layout Behaviors & Spatial Organization Plan (2026-02-24)

**Status**: Implementation-Ready
**Supersedes**: `2026-02-19_layout_advanced_plan.md`
**Relates to**: `2026-02-24_performance_tuning_plan.md` (culling/perf ownership), `2026-02-23_graph_interaction_consistency_plan.md` (viewport gravity), `2026-02-23_udc_semantic_tagging_plan.md` (semantic clustering), `2026-02-24_physics_engine_extensibility_plan.md` (physics engine architecture, ExtraForce extension model, thematic preset designs), `2026-02-22_multi_graph_pane_plan.md` (graph-pane Canonical/Divergent semantics and per-pane budgets).

## Context

This plan covers *behavioral layout features* (how the graph arranges itself), not frame-time optimization.

---

## Scope and Constraints

1. Keep force-behavior toggles policy-driven (`CanvasRegistry`), not hardcoded in render callsites.
2. Preserve existing intent and lifecycle boundaries (`apply_intents` -> reconcile -> render).
3. Treat this plan as additive to current physics system; avoid introducing a parallel layout engine.
4. Scope applies to **graph panes** only (`CanvasRegistry` surface behavior). Node viewer panes and tool panes are out of scope.

**Multi-view note**: Canonical vs Divergent graph-pane semantics are defined in `2026-02-22_multi_graph_pane_plan.md`. Unless explicitly stated otherwise, behaviors in this plan target the canonical shared graph layout path and should not implicitly overwrite divergent local simulations.

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

**Status**: Design prerequisites defined. Implementation blocked pending layout injection hook (Phase 2 prerequisite) and Multi-view Canonical/Divergent scope settlement.

**Tracking note**: This section is the authoritative tracked plan for Magnetic Zones / Group-in-a-Box. It was promoted from research/layout notes into the active implementation sequence per the concept adoption roadmap.

### 3.0 Prerequisites for Implementation

Before any Phase 3 code lands, the following design gaps must be resolved:

1. **Layout injection hook** (`apply_post_frame_layout_injection`) from Phase 2 must be in place. Zone forces are applied through this hook; they are not a separate layout engine.
2. **Multi-view Canonical/Divergent semantics** must be settled enough to assign a zone scope. See `2026-02-22_multi_graph_pane_plan.md` for current state and blocking questions.

### 3.1 Data Model

- `Zone { id, name, centroid, strength }`
- `GraphWorkspace.zones` persisted in snapshots
- `node.zone_id: Option<Uuid>` membership pointer

### 3.2 Zone Persistence Scope

Zones carry spatial meaning and must be scoped to a persistence boundary. Three candidate scopes are defined here; one must be selected before implementation begins.

| Scope | Definition | Trade-offs |
| --- | --- | --- |
| **Workspace** | Zone lives in `GraphWorkspace`; all views that open this workspace share the same zone set. | Simple model; natural snapshot/roundtrip unit. Risk: zones created for one view's layout may pollute other views of the same workspace. |
| **View (per-pane)** | Zone belongs to a specific `GraphViewId`; deleted when the pane is closed unless explicitly promoted. | Clean isolation for Divergent views. Risk: zones are lost if a view is closed unexpectedly; requires view-lifecycle durability work. |
| **Lens** | Zone is attached to a Lens definition and is activated/deactivated with the Lens. | Natural fit for thematic groupings. Risk: adds coupling between layout subsystem and Lens resolution; Lens lifecycle rules must be finalized first. |

**Recommended scope (pre-implementation)**: Start with **Workspace** scope. It aligns with the existing snapshot shape (`GraphWorkspace.zones`), requires no view-lifecycle or Lens coupling, and can be narrowed later if per-view isolation is needed.

**Overlap rules**:
- A node may belong to at most one zone at a time (`node.zone_id: Option<Uuid>`).
- If a zone membership reassignment occurs (e.g., drag into overlapping zone), the most recent explicit assignment wins (last-write precedence).
- Overlapping zone backdrop regions are rendered with distinct visual depth (lower z-order zone renders behind, no force conflict occurs since membership is exclusive).

### 3.3 Zone Force Application

- For zone-bound nodes, apply attraction to zone centroid during layout injection hook.
- Force magnitude is proportional to `Zone.strength` and distance from centroid.
- Zone force is applied **after** global physics forces in the injection hook so it acts as a soft bias, not a hard constraint.
- Render subtle zone backdrop (member bounds + padding) for spatial affordance.

### 3.4 Interaction Model

| Interaction | Behavior |
| --- | --- |
| **Create Zone** | Select ≥1 nodes → "Create Zone" action → derives initial centroid from selection bounding box center; assigns all selected nodes to new zone. |
| **Rename Zone** | Double-click zone label or zone context menu → inline rename. |
| **Add node to Zone** | Drag node onto zone backdrop; node's `zone_id` updated; physics bias shifts toward new centroid. |
| **Remove node from Zone** | Context menu "Remove from Zone" or drag node entirely outside zone backdrop with confirmation; `zone_id` cleared. |
| **Drag Zone** | Drag zone centroid handle → centroid moves; zone members follow by soft force (not teleport). |
| **Delete Zone** | Context menu "Delete Zone" → zone removed; member nodes' `zone_id` fields cleared; nodes retain their last positions. |
| **Merge Zones** | Drag one zone backdrop onto another → combine membership under target zone; source zone deleted. |

**Overlap interaction rules**:
- Zone backdrops may visually overlap; membership is still exclusive (a node cannot be in two zones).
- A drag gesture that ends inside an overlapping backdrop region assigns membership to the topmost (most recently created) zone unless the user explicitly targets the lower one via context menu.

### 3.5 Implementation Sequence

Once prerequisites (§3.0) are resolved:

1. **Step 1**: Add `Zone` type and `GraphWorkspace.zones: Vec<Zone>` to data model; wire snapshot serialization/deserialization.
2. **Step 2**: Add `node.zone_id: Option<Uuid>` field; ensure existing snapshots deserialize with `zone_id = None`.
3. **Step 3**: Implement zone force computation in layout injection hook; gate by `CanvasRegistry.zones_enabled`.
4. **Step 4**: Render zone backdrop (bounding box of member nodes + padding, semi-transparent fill, label).
5. **Step 5**: Wire "Create Zone from Selection" action through `ActionRegistry`.
6. **Step 6**: Implement drag-to-assign and drag-zone interactions.
7. **Step 7**: Implement merge, rename, and delete zone interactions.

---

## Validation

- [ ] Paused physics resumes automatically when structural intents add nodes/edges.
- [ ] Link-triggered/new-context node spawns near source node.
- [ ] Degree repulsion toggle changes hub spread behavior measurably.
- [ ] Domain clustering toggle creates visible same-domain grouping.
- [ ] Zone create/drag updates node spatial behavior and survives snapshot roundtrip.
- [ ] Zone persistence scope is workspace-scoped; zones appear consistently across all views of the same workspace.
- [ ] Node belongs to at most one zone; membership reassignment follows last-write precedence.
- [ ] Deleting a zone clears `zone_id` on all member nodes; nodes retain their positions.
- [ ] Zone force applies as a soft bias after physics forces, not a hard override.
