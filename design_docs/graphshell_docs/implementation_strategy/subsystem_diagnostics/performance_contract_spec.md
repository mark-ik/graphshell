# Performance Contract — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active

**Related**:

- `SUBSYSTEM_DIAGNOSTICS.md`
- `subsystem_diagnostics/2026-02-24_performance_tuning_plan.md`
- `../aspect_render/frame_assembly_and_compositor_spec.md`
- `../canvas/layout_behaviors_and_physics_spec.md`
- `../../TERMINOLOGY.md` — `DiagnosticsChannel`, `CanvasStylePolicy`, `ViewDimension`

---

## 1. Scope

This spec defines the canonical performance contracts for:

1. **Render targets** — node count and frame rate guarantees.
2. **Viewport culling policy** — spatial index and culling rules.
3. **Node and edge LOD** — level-of-detail thresholds by zoom.
4. **Badge animation budget** — animation frame cap.
5. **Physics frame budget** — auto-pause and per-frame time limit.
6. **Diagnostics sampling guard** — how performance metrics are collected without self-degradation.

---

## 2. Render Target Contract

| Scenario | Target | Measurement |
|----------|--------|-------------|
| 500 nodes, no physics | 60 fps | Frame time ≤ 16.7 ms at P95 |
| 1000 nodes, no physics | 30 fps | Frame time ≤ 33.3 ms at P95 |
| Any node count, physics active | ≥ 30 fps | Frame time ≤ 33.3 ms at P95 |

**Invariant**: These targets apply to a Canonical graph view on a mid-range development machine (Intel Core i5-class or Apple M1, 8 GB RAM, integrated GPU). They are the minimum bar; higher-end hardware is expected to do better.

**Invariant**: Render targets apply to the **canvas and workbench render path only**. Servo's web content rendering (composited textures) is excluded from these targets — Servo has its own frame budget.

---

## 3. Viewport Culling Policy Contract

### 3.1 Spatial Index

All nodes in the active graph view are indexed in a spatial acceleration structure. The spatial index is maintained incrementally:

- Rebuilt from scratch on graph load.
- Updated incrementally on node position change (physics step or user drag).
- Spatial index type: `rstar::RTree<NodeKey, AABB>` or equivalent.

**Invariant**: The spatial index is owned by the canvas render state, not by the physics simulation. The physics simulation does not query the spatial index; the render pass does.

### 3.2 Culling Rule

Before rendering, the canvas render pass queries the spatial index to determine the **visible set**: nodes whose bounding box (with a small margin) intersects the current viewport rect.

- Nodes outside the visible set are **not rendered** (neither node body nor edges to/from them).
- The culling margin is `2 × node_radius` to avoid pop-in at the viewport edge.

**Invariant**: Culled nodes are not in the egui widget tree for that frame. They do not consume layout or paint time.

### 3.3 Edge Culling

An edge is culled if **both** its source and destination nodes are outside the visible set. If either endpoint is visible, the edge is rendered (even if it extends outside the viewport, clipped by egui's clip rect).

**Invariant**: Edge culling is derived from the node visible set; no separate spatial index for edges.

### 3.4 Culling Bypass

Physics simulation runs on the full node set regardless of viewport. Culling applies only to the render pass.

---

## 4. Node and Edge LOD Contract

### 4.1 Zoom Thresholds

The canvas render pass uses zoom level to select the level of detail for node and edge rendering.

| Zoom level | Node LOD | Edge LOD |
|------------|----------|----------|
| `zoom < 0.25` | Point (2 px dot; no label, no badge) | Hidden |
| `0.25 ≤ zoom < 0.6` | Compact (icon + truncated label; no badge strip) | Thin solid line; no label |
| `0.6 ≤ zoom < 1.5` | Standard (full node body, label, badge strip) | Normal line with label |
| `zoom ≥ 1.5` | Detail (standard + expanded badge strip, extra metadata) | Normal line with full label |

**Invariant**: LOD thresholds are defined in `CanvasStylePolicy`, not hardcoded in the render path. Changing thresholds requires updating `CanvasStylePolicy` only.

### 4.2 Edge LOD and Traversal Weight

At `zoom < 0.25`, edges are hidden entirely. This is the correct behavior: at very low zoom, individual edges cannot be meaningfully distinguished. The overall graph topology is conveyed by node clustering (physics groups), not edge lines.

At `0.25 ≤ zoom < 0.6`, traversal-weighted edge width (from `edge_traversal_spec.md §4.1`) is suppressed: all edges render at uniform thin width. Width variation requires standard LOD or higher.

**Invariant**: Traversal weight visual encoding (stroke width proportional to `log(N)`) is applied only at `zoom ≥ 0.6`.

---

## 5. Badge Animation Budget Contract

### 5.1 Animation Cap

The badge system may display animated badges (e.g., `#loading`, `#processing`). The render pass enforces a **per-frame cap**:

- Maximum **20 animated badge instances** rendered per frame.
- If more than 20 animated badges are in the visible set, the lowest-priority badges (by badge priority order; see `node_badge_and_tagging_spec.md §3.2`) are rendered as static (non-animated) for that frame.

**Invariant**: The cap of 20 is a runtime limit, not a data model limit. A node may have any number of animated badges in the data model; the cap applies only to how many animate per frame.

### 5.2 Animation Budget Measurement

The badge animation budget is part of the canvas render pass time budget. If the canvas render pass (including badge animations) consistently exceeds 8 ms (half the 60 fps frame budget), the animation cap is automatically lowered by the diagnostics subsystem:

```
DiagnosticsChannel: "canvas:badge_animation_budget"
ChannelSeverity: Warn
```

This channel is emitted when the cap is lowered. It is not emitted every frame — only when the cap changes.

---

## 6. Physics Frame Budget Contract

### 6.1 Per-Frame Time Limit

The physics simulation runs on a background thread with a dedicated frame budget:

- **Physics time budget**: 5 ms per frame (at 60 fps target).
- If a physics step exceeds 5 ms, the remaining force computation is deferred to the next frame (partial step).

**Invariant**: The physics simulation never blocks the render thread. It runs on a dedicated worker thread and posts updated node positions to the render thread via a lock-free buffer.

### 6.2 Auto-Pause

The physics simulation auto-pauses when:

- All forces are below the convergence threshold (kinetic energy of the simulation < `physics_convergence_threshold` from `CanvasNavigationPolicy`).
- The graph has not changed (no nodes added/removed/connected) for the last `physics_idle_frames` frames (default: 120 frames = 2 seconds at 60 fps).

**Invariant**: Auto-pause is a rendering optimization. The physics state (node positions, velocities) is preserved while paused. The simulation resumes on any structural graph change or user drag.

### 6.3 Reheat on Structural Change

Structural graph changes (node add/remove, edge add/remove) trigger a physics **reheat**: kinetic energy is injected proportional to the magnitude of the change (number of nodes/edges affected). Reheat exits auto-pause and runs the simulation until convergence.

This is defined in `layout_behaviors_and_physics_spec.md §2.1` and referenced here as a performance contract: reheat must not cause a visible frame drop. The physics thread absorbs the reheat cost; the render thread continues at target frame rate.

---

## 7. Diagnostics Sampling Guard Contract

### 7.1 Sampling Rate Limit

Performance metrics collected by the Diagnostics subsystem are sampled at a maximum rate of **10 Hz** (one sample per 100 ms). This prevents the diagnostics system from itself becoming a performance bottleneck.

**Invariant**: No performance metric sampling occurs more than once per 100 ms. If the frame rate is 60 fps, performance metrics are sampled every ~6 frames, not every frame.

### 7.2 Sampled Metrics

The following metrics are sampled at the 10 Hz rate:

| Metric | Channel name | Notes |
|--------|-------------|-------|
| Canvas render time (P95) | `canvas:render_time_p95` | ms |
| Physics step time (P95) | `physics:step_time_p95` | ms |
| Visible node count | `canvas:visible_node_count` | culled set size |
| Active badge animation count | `canvas:badge_animation_count` | pre-cap |
| Physics convergence state | `physics:convergence_state` | `Running | Converging | Paused` |

### 7.3 Sampling Guard Implementation

The sampling guard is a token-bucket timer per channel: a sample is recorded only if at least 100 ms have elapsed since the last sample on that channel.

**Invariant**: Sampling guard timers are per-channel, not global. Two different channels may sample independently.

**Invariant**: Sampling guard state is not persisted. It resets on application startup.

### 7.4 Diagnostic Channel Severities

| Channel | Severity | Condition |
|---------|----------|-----------|
| `canvas:render_time_p95` | `Info` | Normal sampling |
| `canvas:render_time_p95` | `Warn` | P95 > 20 ms (exceeds 60 fps target) |
| `canvas:render_time_p95` | `Error` | P95 > 33 ms (exceeds 30 fps target) |
| `physics:step_time_p95` | `Warn` | P95 > 5 ms (exceeds budget) |
| `canvas:badge_animation_budget` | `Warn` | Cap lowered due to budget overrun |

**Invariant**: All `DiagnosticsChannel` entries must declare a `severity` field. See General Code Guidelines in `CLAUDE.md`.

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| 500 nodes renders at 60 fps | Benchmark: 500 nodes, static positions, no physics → P95 frame time ≤ 16.7 ms |
| 1000 nodes renders at 30 fps | Benchmark: 1000 nodes, static positions, no physics → P95 frame time ≤ 33.3 ms |
| Culled nodes absent from widget tree | Test: pan viewport away from all nodes → egui widget tree is empty |
| Edge hidden when both endpoints culled | Test: edge with both nodes outside viewport → no edge draw call |
| Nodes render as point at zoom < 0.25 | Test: zoom to 0.1 → nodes render as 2 px dots; labels absent |
| Animated badge cap enforced | Test: 30 animated badges in view → only 20 animate; 10 render static |
| Physics runs on background thread | Architecture invariant: no physics step calls from render thread |
| Physics auto-pauses at convergence | Test: settle graph → `physics:convergence_state` channel shows `Paused` |
| Reheat on node add does not drop frame | Test: add node during settled physics → frame time P95 unchanged within 3 frames |
| Diagnostics sampled at ≤ 10 Hz | Test: instrument sampling → no channel sampled more than once per 100 ms |
| LOD thresholds defined in CanvasStylePolicy | Architecture invariant: no hardcoded zoom threshold values in render path modules |
