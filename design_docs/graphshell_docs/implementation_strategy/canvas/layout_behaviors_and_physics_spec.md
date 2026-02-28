# Layout Behaviors and Physics — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Implementation-ready

**Related**:

- `CANVAS.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-24_layout_behaviors_plan.md`
- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-02-25_progressive_lens_and_physics_binding_plan.md`
- `2026-02-23_udc_semantic_tagging_plan.md`
- `../system/register/canvas_registry_spec.md`
- `../system/register/physics_profile_registry_spec.md`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Physics micro-behaviors** — reheat, new-node placement, gravity consistency.
2. **Advanced force injection** — the post-physics force hook; degree repulsion; domain clustering.
3. **Magnetic zones** — data model, force application, interaction model, persistence scope.
4. **Lens/physics binding** — how a `LensConfig` references a `PhysicsProfileId` and the `Always/Ask/Never` preference gate.
5. **Progressive lens switching** — threshold-based triggers, hysteresis, preference chain.
6. **Physics engine extension points** — `ExtraForce` hook contract and `LayoutRegistry` algorithm registration.

All layout behavior is canvas-scoped (graph view panes only). Node viewer panes and tool panes are out of scope.

---

## 2. Physics Micro-Behavior Contracts

### 2.1 Reheat on Structural Change

When `AddNode` or `AddEdge` intent is applied (excluding snapshot load and replay paths), the physics simulation **must resume** if paused (`physics.is_running = true`). Velocity state is preserved; simulation is not reset. This prevents newly-changed topology from appearing visually inert.

**Invariant**: A snapshot load must not trigger reheat. Only live user-initiated or link-derived structural changes trigger reheat.

### 2.2 New Node Placement Near Semantic Parent

When a node is created with a known semantic source/parent (e.g., link-follow from an existing node), the new node is placed at `parent_position + jitter`. Center-spawning is the fallback for nodes with no semantic parent.

**Contract**: `GraphSemanticEvent::CreateNewWebView` (or equivalent intent) must carry parent node identity when available. The placement policy must not require the physics engine to search for the parent after the fact.

### 2.3 Gravity Parameter Consistency

The viewport-gravity force must always read its strength from the active `PhysicsProfile` parameter, not a hardcoded constant. Authoritative behavior aligns with `graph_node_edge_interaction_spec.md`.

---

## 3. Post-Physics Force Injection Hook

A `apply_post_frame_layout_injection` hook runs after the primary physics step and before rendering. Custom forces are registered through this hook rather than embedded in the core physics loop. All optional forces in this spec are applied through this hook.

**Contract**: Hook execution order is deterministic. Forces registered via `CanvasRegistry` run in registration order. The hook must not be called if physics is paused (unless a force explicitly requests a single-frame application).

### 3.1 Degree-Dependent Repulsion

Adds extra separation force to nodes with high edge degree.

- Force: `log(degree) * k * separation_direction` applied to nearby neighbors.
- Gated by `CanvasRegistry.degree_repulsion_enabled`.
- `k` is a tunable parameter from the active `PhysicsProfile`.

### 3.2 Domain Clustering Force

Applies weak attraction toward the centroid of nodes sharing the same eTLD+1.

- Groups nodes by eTLD+1; computes group centroids each frame (or on domain-group change).
- Force magnitude: configurable weak attraction coefficient from `PhysicsProfile`.
- Gated by `CanvasRegistry.domain_clustering_enabled`.
- Independent from UDC semantic clustering; both may be active simultaneously.

---

## 4. Magnetic Zones Contract

### 4.1 Data Model

```
Zone {
    id: Uuid,
    name: String,
    centroid: Vec2,
    strength: f32,
}

Node.zone_id: Option<Uuid>   -- membership; None = no zone
GraphWorkspace.zones: Vec<Zone>
```

A node may belong to **at most one zone** at a time. Membership is exclusive.

### 4.2 Persistence Scope

Zones are **workspace-scoped**: they live in `GraphWorkspace.zones` and are part of the snapshot. All views of the same workspace share the same zone set.

**Rationale**: Workspace scope aligns with the existing snapshot shape and requires no view-lifecycle or Lens coupling. Per-view zone isolation is deferred.

### 4.3 Force Application

- Zone force is applied through the post-physics injection hook (§3), after global physics forces.
- Force: attraction toward `Zone.centroid`, magnitude proportional to `Zone.strength` × distance from centroid.
- Zone force is a **soft bias**, not a hard constraint. Physics forces may still displace zone members.
- Gated by `CanvasRegistry.zones_enabled`.

### 4.4 Membership Rules

- A node may belong to at most one zone at a time.
- If membership reassignment occurs (e.g., drag into overlapping zone backdrop), the most recent explicit assignment wins (last-write precedence).
- Overlapping zone backdrops are rendered at distinct visual depth (lower z-order renders behind). No force conflict occurs since membership is exclusive.

### 4.5 Zone Interaction Contract

| Interaction | Required behavior |
|-------------|------------------|
| Create Zone (≥1 nodes selected) | Derive centroid from selection bounding box center; assign all selected nodes to new zone |
| Rename Zone | Double-click zone label or zone context menu → inline rename |
| Add node to Zone | Drag node onto zone backdrop → node's `zone_id` updated; force bias shifts toward new centroid |
| Remove node from Zone | Context menu "Remove from Zone" or drag entirely outside zone backdrop; `zone_id` cleared |
| Drag Zone centroid | Centroid moves; zone members follow via soft force (not teleport) |
| Delete Zone | Zone removed; all member nodes' `zone_id` fields cleared; node positions are preserved |
| Merge Zones | Drag one zone backdrop onto another → combine membership under target; source zone deleted |

**Invariant**: Deleting a zone must clear `zone_id` on all member nodes in the same atomic operation. No node may retain a `zone_id` pointing to a deleted zone.

### 4.6 Zone Rendering

Zone backdrops render as: bounding box of member nodes + padding, semi-transparent fill, zone label. Backdrop is rendered below nodes on the z-axis.

---

## 5. Lens/Physics Binding Contract

### 5.1 Data Model

`LensConfig` carries one optional field:

```
LensConfig {
    physics_profile_id: Option<PhysicsProfileId>,   -- None = no binding
    // …existing fields…
}
```

`None` means the Lens has no physics opinion; the current active profile is preserved on Lens apply.

### 5.2 Binding Preference

Stored in `AppPreferences`:

```
lens_physics_binding: LensPhysicsBindingPreference  -- default: Ask
```

```
LensPhysicsBindingPreference =
  | Always   -- auto-switch without confirmation
  | Ask      -- non-blocking toast; user confirms or dismisses
  | Never    -- ignore physics_profile_id; never auto-switch
```

### 5.3 Runtime Behavior at Lens Apply

When `LensCompositor::apply_lens(lens_id, view_id)` is called:

1. Resolve `LensConfig` via fallback chain (Workspace → User → Default).
2. If `physics_profile_id` is `None` → skip all binding logic.
3. Check `lens_physics_binding`:
   - `Always` → call `PhysicsProfileRegistry::activate(physics_profile_id, view_id)` immediately.
   - `Ask` → emit a `LensPhysicsBindingSuggestion` event; render non-blocking inline prompt. No auto-switch until user confirms. Dismissal is stored as a per-`(LensId, PhysicsProfileId)` skip hint in session state (not persisted; resets on restart).
   - `Never` → no-op; active profile is unchanged.

**Invariant**: `LensTransitionHook` entries registered via mods are subject to the same `Always/Ask/Never` gate. Hooks must not bypass the preference gate.

---

## 6. Progressive Lens Switching Contract

### 6.1 Mechanism

Progressive Lens switching is **threshold-based** (discrete transitions at defined zoom levels), not continuous interpolation. Continuous interpolation is deferred until per-field interpolation contracts exist at the registry layer.

### 6.2 Data Model

```
LensConfig {
    progressive_breakpoints: Option<Vec<ProgressiveLensBreakpoint>>,
    // …
}

ProgressiveLensBreakpoint {
    zoom_scale_threshold: f32,   -- scale at which this Lens activates (zoom-out direction = decreasing)
    lens_id: LensId,
}
```

Breakpoints are sorted descending by `zoom_scale_threshold`. The first breakpoint whose threshold is ≥ current zoom scale is the active progressive target.

### 6.3 Trigger Evaluation

`LensCompositor` evaluates progressive breakpoints on every `CameraScaleChanged` event.

**Hysteresis**: A ±10% band on each threshold prevents oscillation at boundaries. A switch triggers only when the scale crosses outside the hysteresis band from the prior side:

```
hysteresis_band = zoom_scale_threshold * 0.10
switch_triggers_when: abs(current_scale - zoom_scale_threshold) > hysteresis_band
                      AND side_changed
```

### 6.4 Preference Chain

Stored in `AppPreferences`:

```
progressive_lens_auto_switch: ProgressiveLensAutoSwitch  -- default: Ask
```

```
ProgressiveLensAutoSwitch =
  | Always
  | Ask
  | Never
```

Preference chain at threshold crossing:

1. Check `progressive_lens_auto_switch` first; if `Never`, stop.
2. If target Lens carries a `physics_profile_id` and the switch is allowed, evaluate `lens_physics_binding` before activating the physics profile.

---

## 7. Physics Engine Extension Points

### 7.1 LayoutRegistry Algorithm Registration

`LayoutRegistry` is an atomic algorithm store: maps `LayoutId → Algorithm`. `CanvasRegistry` uses this to resolve the active layout algorithm. Custom layout algorithms are registered as entries in `LayoutRegistry`; they do not modify the core physics loop.

**Contract**: Registered algorithms must implement a stable interface callable from `CanvasRegistry`'s execution path. Algorithm registration is a mod concern; `CanvasRegistry` is the execution authority.

### 7.2 ExtraForce Hook

Physics profiles may include `ExtraForce` entries. An `ExtraForce` is a named, parameterized force function appended to the physics step. ExtraForce entries are registered through the engine extension interface; they are not hardcoded in the core FR loop.

**Contract**: ExtraForce invocations must not assume a specific execution order relative to each other unless declared as dependent. Force ordering within the post-physics injection hook is deterministic by registration order.

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Physics resumes on `AddNode`/`AddEdge` | Test: add node while paused → `is_running` becomes true |
| Snapshot load does not trigger reheat | Test: load snapshot while paused → `is_running` remains false |
| Link-follow node spawns near source | Test: new node with parent → position within `parent_position + max_jitter` |
| Degree repulsion toggle changes hub spread | Test: enable/disable → measurable difference in high-degree node neighbor distances |
| Domain clustering toggle creates grouping | Test: enable → same-domain nodes converge; disable → disperse |
| Zone membership is exclusive | Test: node in zone A dragged into zone B → `zone_id` updated to B, not both |
| Zone delete clears all member `zone_id` fields | Test: delete zone → all former members have `zone_id == None` |
| Zone force is soft bias, not hard override | Test: zone member can be displaced by physics; force magnitude proportional to strength |
| Zone survives snapshot roundtrip | Test: save/load → zones and membership intact |
| `lens_physics_binding: Never` blocks auto-switch | Test: apply Lens with `physics_profile_id` → active profile unchanged |
| `lens_physics_binding: Always` auto-switches | Test: apply Lens → `PhysicsProfileRegistry::activate` called |
| Progressive switching respects hysteresis | Test: zoom oscillation at threshold → switch fires once, not repeatedly |
| `progressive_lens_auto_switch: Never` disables all progressive switches | Test: zoom past threshold → no Lens change |
