# Performance Tuning Plan (2026-02-24)

**Status**: Implementation-Ready  
**Supersedes**: `2026-02-11_performance_optimization_plan.md`  
**Context**: Post-Registry migration cleanup and scaling.

## Goal

Target performance envelopes on reference hardware:
- **500 nodes @ 60 FPS**
- **1000 nodes @ 30+ FPS**

---

## Architecture Integration

- `CanvasRegistry`: runtime toggles/policies (`viewport_culling_enabled`, `label_culling_enabled`, edge LOD mode).
- `DiagnosticRegistry`: frame-time/entity metrics with bounded sampling.
- `MetadataFrame`: previous-frame zoom/pan oracle used for LOD and viewport decisions.
- `render/spatial_index.rs`: viewport candidate lookup using spatial index (avoid full O(N) scans).

---

## Phase 1: Viewport Culling (Primary Win)

### Plan
1. Compute visible world rect from previous-frame camera metadata.
2. Query spatial index for candidate visible nodes.
3. Submit only visible nodes to `egui_graphs`.

### Edge Handling Constraint
- Validate behavior when edge endpoints are culled.
- If renderer requires both endpoints present, add explicit edge filtering/ghost endpoint policy.

### Policy
- Add/confirm `viewport_culling_enabled` in `CanvasRegistry`.

---

## Phase 2: Node + Edge LOD

### 2.1 Node Label LOD
1. Zoom `< 0.5`: hide labels.
2. Zoom `0.5..=1.5`: domain-only label.
3. Zoom `> 1.5`: full title.

### 2.2 Label Occlusion Culling
- Rank visible nodes by importance (selection + graph importance).
- Greedy rect packing: skip intersecting lower-priority labels.

### 2.3 Edge LOD
1. Zoom `< 0.3`: hide non-critical edges or all edges by policy.
2. Zoom `< 0.8`: reduced alpha/width, no arrowheads.
3. Zoom `>= 0.8`: full edge styling.

### Policy
- Add/confirm `label_culling_enabled` and edge LOD policy in `CanvasRegistry`.

---

## Phase 3: Badge Animation Budget

### Plan
1. Cap animated badges per frame (initial cap: `20`).
2. Disable animation for nodes beyond view-center distance threshold.
3. Fallback to static badge rendering when budget is exhausted.

---

## Phase 4: Physics Budget and Stability

### 4.1 Auto-Pause
- If average displacement remains below epsilon for N frames, pause simulation.
- Resume on structural/interaction intent.

### 4.2 Per-Frame Physics Time Budget
- Bound physics update work per frame (initial budget target: ~5ms).
- Carry remaining simulation work to subsequent frames.
- Favor frame responsiveness over instant convergence.

### 4.3 Deferred Upgrade
- Keep Barnes-Hut as deferred path for sustained >1000 node regimes.

---

## Phase 5: Diagnostics Sampling Guard

To avoid observer-effect distortion:
- Keep diagnostic aggregation/render sampling bounded (10 Hz target for diagnostic updates).
- Keep main UI/render loop unconstrained by diagnostic refresh cadence.

---

## Validation

- [ ] 500-node benchmark at zoomed-out view meets 60 FPS target band.
- [ ] 1000-node benchmark meets 30+ FPS target band.
- [ ] Viewport culling produces expected visible-set reduction and frame-time drop.
- [ ] Label and edge LOD transitions occur at configured zoom thresholds.
- [ ] Physics budget cap prevents long-frame spikes during active simulation.
- [ ] Diagnostics enabled vs disabled does not materially skew benchmark readings.