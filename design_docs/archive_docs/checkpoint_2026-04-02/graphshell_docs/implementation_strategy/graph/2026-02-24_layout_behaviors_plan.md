# Layout Behaviors & Spatial Organization Plan (2026-02-24)

**Archived status**: Closed / Archived 2026-04-02 — retained as implementation history; canonical active authority now lives in `layout_behaviors_and_physics_spec.md` and related active contracts.
**Archived on**: 2026-04-02
**Reason**: The behavior slices tracked here landed; this document is preserved as the historical execution record.
**Status**: Execution complete for the behavior slices originally tracked here. Phase 1.2 parent-aware spawn placement, Phase 2.1 degree-dependent repulsion, and Phase 2.2 domain clustering landed on 2026-04-02. Remaining work, if any, is follow-on tuning or optimization rather than missing feature slices from this plan.
**Refactor note (2026-04-02)**: This file previously described Phase 3 as blocked on the layout injection hook. That status is stale. The hook and the frame-affinity slice landed under `lane:layout-semantics`, closed 2026-03-26.
**Supersedes**: `2026-02-19_layout_advanced_plan.md`
**Canonical spec**: `layout_behaviors_and_physics_spec.md` — authoritative contracts for all behaviors described here. This file is now a historical execution plan plus current status ledger.
**Latest checkpoint**: `../PLANNING_REGISTER.md` — see the 2026-03-26 `lane:layout-semantics` closure note.
**Relates to**: `2026-02-24_performance_tuning_plan.md` (culling/perf ownership), `graph_node_edge_interaction_spec.md` (graph-surface camera and viewport expectations), `semantic_tagging_and_knowledge_spec.md` (UDC parsing, semantic clustering semantics, canonicalization), `2026-02-24_physics_engine_extensibility_plan.md` (physics engine architecture, ExtraForce extension model, thematic preset designs), `multi_view_pane_spec.md` (graph-pane isolation and per-view layout ownership), `../workbench/graph_first_frame_semantics_spec.md` (Frame identity and membership authority).

## Context

This plan covers behavioral layout features, not frame-time optimization.

### External pattern note (2026-04-01): RustGrapher / WasmGrapher

Reviews of RustGrapher and WasmGrapher reinforce two boundaries already implicit in this plan.

- Behavioral layout policy and force composition should remain Graphshell-owned rather than widget-owned. External graph libraries are useful as evidence about engine structure, not as authority over Graphshell semantics.
- Acceleration structures such as Barnes-Hut or quadtree indexing are legitimate follow-on work, but only after simulation ownership and per-view runtime boundaries are clean. They are not a reason to bypass the injection-hook and policy-first sequencing.

Practical consequence: keep prioritizing explicit behavior hooks, per-view isolation, and deterministic policy gates before deeper performance specialization.

---

## Scope and Constraints

1. Keep force-behavior toggles policy-driven (`CanvasRegistry` / resolved surface profile), not hardcoded in render callsites.
2. Preserve existing intent and lifecycle boundaries (`apply_intents` -> reconcile -> render).
3. Treat this plan as additive to the current physics system; avoid introducing a parallel layout engine.
4. Scope applies to graph panes only (`CanvasRegistry` surface behavior). Node viewer panes and tool panes are out of scope.

**Multi-view note**: Graph-view layout state is isolated per `GraphViewId` as defined in `multi_view_pane_spec.md`. Unless explicitly stated otherwise, behaviors in this plan must not implicitly overwrite a sibling view's local simulation state.

---

## Current State Snapshot

### Landed

- Structural graph mutations reheat physics without zeroing velocity state.
- Profile-driven gravity strength remains wired through the resolved physics profile into the runtime physics state.
- The post-physics extension path is present and used for semantic/frame-affinity behavior.
- Frame-affinity regions are derived at runtime and rendered as soft organizational backdrops behind nodes.
- Frame-affinity force is gated by `CanvasRegistry.zones_enabled`.
- Graph-backed frame semantics are aligned on `ArrangementRelation` / `frame-member` authority rather than a separate zone store.

### Follow-on only

- Force-strength tuning and scenario-based regression coverage can continue as separate polish work.
- Deeper acceleration work such as Barnes-Hut or quadtree indexing remains out of scope for this plan.

---

## Phase 1: Physics Micro-Behaviors

### 1.1 Reheat on Structural Change

**Status**: Landed.

**Outcome**:

- Adding nodes/edges through graph mutation paths reheats physics by setting `physics.is_running = true`.
- The behavior resumes simulation rather than resetting the velocity state.
- Snapshot/load replay is not treated as a user-visible reheat path.

### 1.2 New Node Placement Near Semantic Parent

**Status**: Landed.

**Original goal**:

- If node creation has a semantic source/parent, place at `parent_position + jitter`.
- Ensure the create-new path carries parent identity so placement does not need a later graph search.

**Outcome**:

- Shared anchored-placement logic now backs both host child-webview creation and generic anchored new-node placement.
- When a known semantic source/parent is available, new nodes are placed near that anchor rather than center-spawned.
- Center/centroid fallback remains only for cases without an anchor.

### 1.3 Gravity Parameter Consistency

**Status**: Landed.

**Outcome**:

- The resolved physics profile remains the authority for gravity strength.
- Gravity strength is mapped into runtime physics tuning and applied to the center-gravity extra in the active physics state.

---

## Phase 2: Post-Physics Extension Path and Advanced Force Slices

**Status**: Foundation landed; planned force slices remain partially open.

### 2.0 Landed foundation

The former prerequisite is now resolved in practice through the existing post-layout/post-physics extension path:

- Render flow pulls the latest layout state, updates runtime physics state, then applies extension forces.
- Resolved per-view physics profile data builds a `GraphPhysicsExtensionConfig`.
- `CanvasRegistry.zones_enabled` gates frame-affinity behavior at the call site.

This means the original Phase 3 blocker is gone.

### 2.1 Degree-Dependent Repulsion

**Status**: Landed.

**Original goal**: De-clutter high-degree hub topologies.

**Outcome**:

- `PhysicsProfile.degree_repulsion` now gates a real post-physics force pass.
- Nearby nodes receive additional separation bias based on local degree, improving high-degree hub decluttering without introducing a parallel layout engine.

### 2.2 Domain Clustering Force

**Status**: Landed.

**Original goal**: Pull same-domain nodes into soft spatial neighborhoods.

**Outcome**:

- `PhysicsProfile.domain_clustering` now gates a real post-physics force pass.
- Nodes are grouped by registrable-domain heuristic and softly attracted toward the domain centroid.
- The force remains independent from UDC semantic clustering.

**Note**: This remains independent from UDC semantic clustering; both may be enabled together.

---

## Phase 3: Frame-Affinity Organizational Behavior

**Status**: Core behavior landed. `lane:layout-semantics` execution slices closed 2026-03-26.

> **Terminology**: The historical name "Magnetic Zones" is a legacy alias only. Canonical framing is frame-affinity organizational behavior under graph-first frame semantics. Do not use `Zone`, `MagneticZone`, or `node.zone_id` in new code or docs. The authoritative behavioral contract remains `layout_behaviors_and_physics_spec.md §4`.

### 3.0 Former blockers are now resolved

1. The post-physics extension hook is present in the render/physics path and is used to apply frame-affinity force.
2. Graph-first frame semantics are settled enough for implementation use, with newer alignment notes treating durable `ArrangementRelation` / `frame-member` edges as the long-term authority.

### 3.1 Current data model framing

The original Phase 3 data model has been refined by later frame-semantics work. The current framing is:

```text
FrameAffinityRegion {
  frame_anchor: NodeKey,
  members: Vec<NodeKey>,
  centroid: Vec2,
  strength: f32,
}

ArrangementRelation(FrameMember) edges      // durable frame-membership authority
GraphFrame.member_nodes: Vec<NodeKey>       // transitional/derived projection
Per-node membership views                   // derived/read-model projections where exposed
```

A node may belong to zero, one, or many frames. There is no `node.zone_id` field and no separate persisted zone store.

### 3.2 Persistence scope

Frame-affinity regions are derived runtime state, not an independent persisted workspace entity.

- Durable frame identity and membership live in graph-backed frame semantics.
- Any region geometry cache is derivable runtime state and must not create a second durable identity.
- Snapshot roundtrip persists graph-backed frame membership truth; affinity projection recomputes from that source.

### 3.3 Applied behavior

- Frame-affinity force is applied after the primary physics step through the extension path.
- The force is a soft bias toward frame centroids, not a hard positional constraint.
- Behavior is gated by `CanvasRegistry.zones_enabled`.
- When a node has multiple frame memberships, the resulting force contributions compose deterministically.
- A semi-transparent backdrop with label is rendered below member nodes for active frame-affinity regions.

### 3.4 What landed from the original execution sequence

The core execution sequence is now complete for the force/render slice:

1. `FrameAffinityRegion` exists as a derived runtime type.
2. Frame-affinity force is computed from graph-backed frame membership and applied through the extension path.
3. Frame-affinity backdrop rendering is present and visually gated by `zones_enabled`.

The broader frame interaction surface now lives primarily under the workbench/frame semantics lane rather than this file's original checklist. `graph_first_frame_semantics_spec.md`, `workbench_frame_tile_interaction_spec.md`, and `2026-03-26_frame_layout_hint_spec.md` should be treated as the current sources for frame lifecycle, tile-group materialization, and related UI interactions.

---

## Validation and Exit State

See `layout_behaviors_and_physics_spec.md §8` for the full acceptance criteria table. Current summary:

- [x] Structural mutation reheats physics.
- [x] Parent-aware spawn near semantic parent is landed.
- [x] Gravity strength remains profile-driven.
- [x] Degree repulsion is configured and applied as a force slice.
- [x] Domain clustering is configured and applied as a force slice.
- [x] Frame-affinity regions are derived runtime state; no separate zone store exists.
- [x] Multi-membership frame-affinity forces compose deterministically.
- [x] Frame-affinity applies as a soft post-physics bias, not a hard override.
- [x] Frame membership authority is graph-backed; affinity projection recomputes from persisted graph state.

## Remaining Follow-On Work

1. Tune force constants and expand scenario-level regression coverage if behavior polish is needed.
2. Keep any deeper optimization work, such as Barnes-Hut or quadtree acceleration, separate from the semantic/layout ownership already established here.
