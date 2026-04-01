# Layout Behaviors & Spatial Organization Plan (2026-02-24)

**Status**: Implementation-Ready (Phases 1–2); Phase 3 blocked on layout injection hook
**Supersedes**: `2026-02-19_layout_advanced_plan.md`
**Canonical spec**: `layout_behaviors_and_physics_spec.md` — authoritative contracts for all behaviors described here; this plan documents the execution sequence and open prerequisites only.
**Relates to**: `2026-02-24_performance_tuning_plan.md` (culling/perf ownership), `graph_node_edge_interaction_spec.md` (graph-surface camera and viewport expectations), `semantic_tagging_and_knowledge_spec.md` (UDC parsing, semantic clustering semantics, canonicalization), `2026-02-24_physics_engine_extensibility_plan.md` (physics engine architecture, ExtraForce extension model, thematic preset designs), `multi_view_pane_spec.md` (graph-pane isolation and per-view layout ownership), `../workbench/graph_first_frame_semantics_spec.md` (Frame identity and membership authority).

## Context

This plan covers *behavioral layout features* (how the graph arranges itself), not frame-time optimization.

---

## Scope and Constraints

1. Keep force-behavior toggles policy-driven (`CanvasRegistry`), not hardcoded in render callsites.
2. Preserve existing intent and lifecycle boundaries (`apply_intents` -> reconcile -> render).
3. Treat this plan as additive to current physics system; avoid introducing a parallel layout engine.
4. Scope applies to **graph panes** only (`CanvasRegistry` surface behavior). Node viewer panes and tool panes are out of scope.

**Multi-view note**: Graph-view layout state is isolated per `GraphViewId` as defined in `multi_view_pane_spec.md`. Unless explicitly stated otherwise, behaviors in this plan must not implicitly overwrite a sibling view's local simulation state.

---

## Phase 1: Physics Micro-Behaviors

### 1.1 Reheat on Structural Change

**Problem**: Adding nodes/edges while paused leaves newly changed topology visually inert.

**Plan**:

- In `apply_reducer_intents()`, when `AddNode` or `AddEdge` occurs (excluding snapshot load/replay), set `physics.is_running = true`.
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
- Keep authoritative behavior aligned with `layout_behaviors_and_physics_spec.md §2.3`
  and graph-surface expectations in `graph_node_edge_interaction_spec.md`.

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

## Phase 3: Frame-Affinity Organizational Behavior

**Status**: Implementation blocked on layout injection hook (Phase 2 prerequisite).

> **Terminology**: The historical name "Magnetic Zones" is a legacy alias only. Canonical framing is **frame-affinity organizational behavior** using `GraphFrame` identity and `Frame membership` as defined in `../workbench/graph_first_frame_semantics_spec.md`. Do not use `Zone`, `MagneticZone`, or `node.zone_id` in new code or docs — these names are superseded. The authoritative contract for this behavior is `layout_behaviors_and_physics_spec.md §4`.

### 3.0 Prerequisites for Implementation

Before any Phase 3 code lands, the following must be in place:

1. **Layout injection hook** (`apply_post_frame_layout_injection`) from Phase 2 must be in place. Frame-affinity forces are applied through this hook; they are not a separate layout engine.
2. **Graph-first frame semantics** (`../workbench/graph_first_frame_semantics_spec.md`) must be settled enough to use `GraphFrame` identity and `NodeFrameMembership` as the persistence authority.

### 3.1 Data Model

The canonical model is derived from `layout_behaviors_and_physics_spec.md §4.1`:

```text
FrameAffinityRegion {
  frame_id: FrameId,     // references canonical GraphFrame identity
  centroid: Vec2,
  strength: f32,
}

GraphFrame.member_nodes: Vec<NodeKey>         // canonical membership authority
NodeFrameMembership.frames: Vec<FrameId>      // canonical per-node projection
```

A node may belong to zero, one, or many frames. There is no `node.zone_id` field and no `GraphWorkspace.zones` store.

### 3.2 Persistence Scope

Frame-affinity regions are **derived from graph-frame state**, not persisted as an independent workspace entity.

- `GraphFrame` identity and `NodeFrameMembership` are persisted in graph scope per `graph_first_frame_semantics_spec.md`.
- Any region geometry cache (e.g., computed hull/bounds) is derivable runtime state; it must not create a second durable identity.
- Snapshot roundtrip persists frame identity + memberships only; affinity projection recomputes from that source.

### 3.3 Force Application

- Frame-affinity force is applied through the post-physics injection hook (§3 above), after global physics forces.
- Force: attraction toward each applicable frame-affinity centroid, magnitude proportional to `strength × distance`.
- Frame-affinity force is a **soft bias**, not a hard constraint.
- Gated by `CanvasRegistry.zones_enabled`.
- When a node has multiple frame memberships, forces are composed from all active frame-affinity regions deterministically.

### 3.4 Interaction Model

Canonical interaction table from `layout_behaviors_and_physics_spec.md §4.5`:

| Interaction | Required behavior |
| ----------- | ----------------- |
| Create frame-affinity region (≥1 nodes selected) | Create/resolve a `Frame` via graph-first semantics; derive centroid from frame member distribution |
| Rename Frame | Rename canonical `GraphFrame` label; affinity rendering reflects updated label |
| Add node to Frame | Drag node onto frame-affinity backdrop or command action → `AddNodeToFrame(frame_id, node_key)` |
| Remove node from Frame | Contextual action or drag-out gesture → `RemoveNodeFromFrame(frame_id, node_key)` |
| Drag region centroid | Visual centroid/anchor moves; members follow via soft force (not teleport) |
| Delete Frame | `DeleteFrame(frame_id)` removes frame identity and memberships atomically |
| Merge Frames | `MergeFrames(source, target)` combines memberships under target frame |

**Invariant**: Canvas interactions that add/remove affinity membership must mutate canonical frame membership via the above intents. No direct mutation of a `zone_id` field.

### 3.5 Implementation Sequence

Once prerequisites (§3.0) are resolved:

1. **Step 1**: Add `FrameAffinityRegion` as a derived runtime type (not persisted); wire it from `GraphFrame` + `NodeFrameMembership`.
2. **Step 2**: Implement frame-affinity force computation in layout injection hook; gate by `CanvasRegistry.zones_enabled`.
3. **Step 3**: Render frame-affinity backdrop (derived hull of member nodes + padding, semi-transparent fill, frame label) below nodes.
4. **Step 4**: Wire "Create Frame from Selection" action through `ActionRegistry`.
5. **Step 5**: Implement drag-to-assign (→ `AddNodeToFrame`) and drag-region-centroid interactions.
6. **Step 6**: Implement merge, rename, and delete frame interactions.

---

## Validation

See `layout_behaviors_and_physics_spec.md §8` for the full acceptance criteria table. Checklist summary:

- [ ] Paused physics resumes automatically when structural intents add nodes/edges.
- [ ] Snapshot load does not trigger reheat.
- [ ] Link-triggered/new-context node spawns near source node.
- [ ] Degree repulsion toggle changes hub spread behavior measurably.
- [ ] Domain clustering toggle creates visible same-domain grouping.
- [ ] Frame-affinity region is derived from `GraphFrame` + `NodeFrameMembership`; no separate zone store exists.
- [ ] A node may have zero, one, or many frame memberships; multi-membership forces compose deterministically.
- [ ] `DeleteFrame(frame_id)` removes all membership links atomically; no node retains membership to a deleted frame.
- [ ] Frame-affinity force applies as a soft bias after physics forces, not a hard override.
- [ ] Frame identity + memberships survive snapshot roundtrip; affinity projection recomputes without a separate persisted zone store.
