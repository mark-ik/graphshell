# Physics Worker Spike — Success Metrics

**Date**: 2026-03-27
**Status**: Metrics defined; baselines pending runtime measurement
**Purpose**: Stage 3 output of the physics worker spike. Defines the success criteria
and measurement protocol that must be satisfied before any worker implementation proceeds.

---

## Context

A physics worker is only worth building if it measurably reduces frame-budget pressure
without introducing position divergence or velocity-loss regressions. This document
defines the three metric families and how to collect baseline numbers.

See also:
- Spike Stage 1 receipt: `graph/layouts/graphshell_force_directed.rs` (doc comment block)
- Spike Stage 2 receipt:
  `2026-03-27_egui_retained_state_efficiency_and_physics_worker_evaluation_plan.md`
  (ownership table and ordered phases)
- egui_graphs efficiency improvements: `structural_dirty`/`visual_dirty` split should
  happen **before** Stage 3 baseline measurement, since it changes the velocity-loss rate.

---

## Metric 1 — Frame Time Budget

**What it measures:** How much of the egui frame budget the synchronous physics step
consumes. If the step is fast enough, a worker adds complexity with no benefit.

**How to measure:**
Wrap the physics step region in `render/mod.rs` (lines 561–670: `set_layout_state` →
`GraphView::add()` → `get_layout_state` → `apply_graph_physics_extensions`) with a
`std::time::Instant` pair and emit via the existing `emit_span_duration` helper:

```rust
let t0 = std::time::Instant::now();
// ... physics step region ...
emit_span_duration("render::physics_step", t0.elapsed().as_micros() as u64);
```

The `emit_span_duration` function is in `shell/desktop/runtime/diagnostics.rs:147`.

**Graphs to benchmark:**

| N | Description |
| - | ----------- |
| 100 nodes | Typical active session |
| 500 nodes | Large knowledge graph |
| 1000 nodes | Stress case |

**Pass criteria:**

- If p99 frame time for the physics step is < 1 ms at N=500, a worker is **not justified**.
- If p99 is > 2 ms at N=500 or > 1 ms at N=100, a worker is **worth prototyping**.

**Baseline numbers (pending measurement):**

| N | avg (µs) | p99 (µs) | measured at |
| - | -------- | -------- | ----------- |
| 100 | — | — | — |
| 500 | — | — | — |
| 1000 | — | — | — |

---

## Metric 2 — Position Divergence

**What it measures:** Whether a proposed worker path produces the same node positions as
the synchronous path after N steps. If the FR step is non-deterministic across async
boundaries (e.g. due to floating-point ordering differences or state races), the worker
model is incorrect by construction.

**How to measure:**
1. Run a deterministic graph (fixed seed positions, no drag, no lens modification) for
   N=1000 frames synchronously. Capture the final node positions as a
   `Vec<(NodeKey, Pos2)>` snapshot.
2. Run the same graph via the proposed worker path (copy-out → off-thread step →
   copy-in). Capture the same snapshot.
3. Assert that all positions agree within epsilon (suggested: 1e-3 in each axis).

**Pass criteria:**

- Position divergence after N=1000 frames < 1e-3 in both x and y for all nodes.
- If divergence is larger, the copy-out / copy-in boundary introduces ordering
  differences that make the step non-reproducible — the worker design is invalid.

**Baseline numbers:** N/A (comparison metric, not an absolute baseline).

---

## Metric 3 — Velocity-Loss Rate

**What it measures:** How often per session the FR velocity is reset to zero due to a
full `egui_state_dirty` rebuild of `EguiGraphState`. Every full rebuild calls
`EguiGraphState::from_graph()`, which seeds positions from `Node::projected_position()`
and discards all accumulated FR velocity — causing a visible physics stutter.

**How to measure:**
Add a diagnostics emit inside `EguiGraphState::from_graph()` in
`model/graph/egui_adapter.rs`:

```rust
emit_event(DiagnosticEvent::MessageSent {
    channel_id: CHANNEL_GRAPH_EGUI_STATE_REBUILT, // new channel — see below
    byte_len: 0,
});
```

A new channel `graph:egui_state_rebuilt` (severity: `Info`) should be registered.
Count this channel's events per session in the Diagnostics Inspector pane.

**Baseline and target:**

| Condition | Expected rate | Notes |
| --------- | ------------- | ----- |
| Before `structural_dirty`/`visual_dirty` split | High — triggered by selection, badge, crash-flag changes (40+ `egui_state_dirty` sites) | Baseline |
| After split | Low — triggered only by node/edge add/remove | Target |
| Physics worker active | Should be ≤ "after split" rate | Worker must not introduce additional rebuilds |

**Baseline numbers (pending measurement):**

| Condition | rebuilds/min (typical session) | measured at |
| --------- | ------------------------------ | ----------- |
| Before split | — | — |
| After split | — | — |

---

## Measurement Protocol

1. Build with `--release` (physics step time is not representative in debug builds).
2. Load the same fixture graph for each N (100/500/1000 nodes) — use a saved `.gsw`
   workspace or a deterministic in-memory fixture.
3. Let physics run for 30 seconds with no user interaction.
4. Read `emit_span_duration("render::physics_step", ...)` events from the Diagnostics
   Inspector ring buffer.
5. Record average and p99 in the table above.

---

## Decision Gate

After baselines are collected:

- **If p99 < 1 ms at N=500 and velocity-loss rate is acceptable after split:**
  Do not build a physics worker. Close the spike as "not worth it."

- **If p99 > 2 ms at N=500 or velocity-loss is unacceptable after split:**
  Proceed to worker implementation per the conditional architecture in the spike plan
  (`2026-03-27_egui_retained_state_efficiency_and_physics_worker_evaluation_plan.md`
  — Phase E).

---

## Prerequisites

Before collecting Stage 3 baselines, complete:

1. `structural_dirty` / `visual_dirty` split in `model/graph/egui_adapter.rs` —
   this changes the rebuild rate that Metric 3 measures.
2. Add the `graph:egui_state_rebuilt` diagnostic channel to the channel registry.
3. Add the `render::physics_step` span emit to `render/mod.rs`.
