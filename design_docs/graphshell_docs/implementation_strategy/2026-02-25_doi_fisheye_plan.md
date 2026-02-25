# DOI + Semantic Fisheye Focus/Context Plan (2026-02-25)

**Status**: Deferred (blocked) — awaiting basic LOD/culling baseline from `2026-02-24_performance_tuning_plan.md` Phases 1–2  
**Blocking Prerequisites**: Viewport culling (Phase 1) and Node + Edge LOD (Phase 2) from `2026-02-24_performance_tuning_plan.md` must be in place.  
**Context**: Post-LOD readability improvement for dense graphs; preserves mental map while surfacing relevance.  
**Relates to**: `2026-02-24_performance_tuning_plan.md` (LOD/culling primitives this plan builds on), `2026-02-18_graph_ux_research_report.md` §§13.2, 14.8, 14.9 (research basis), `2026-02-22_multi_graph_pane_plan.md` (per-pane isolation rule)

---

## Goal

Introduce DOI-driven rendering and semantic fisheye as a concrete post-LOD roadmap item for graph UX. Focus+context without geometric distortion: keep the graph topology undistorted while scaling visual emphasis by relevance.

**Not a replacement for LOD/culling.** DOI and semantic fisheye are rendering-emphasis layers that build on top of stable viewport culling and zoom-adaptive LOD primitives. They do not duplicate or replace those systems.

---

## DOI Data Contract

### DOI Score Definition

```
DOI(n) = α·Recency(n) + β·Frequency(n) + γ·ExplicitInterest(n) - δ·DistanceFromFocus(n)
```

| Component | Formula | Range |
| --- | --- | --- |
| `Recency(n)` | `1.0 / (1.0 + time_since_last_visit_secs)` | `[0.0, 1.0]` |
| `Frequency(n)` | `log(1 + visit_count) / log(1 + max_visit_count)` | `[0.0, 1.0]` |
| `ExplicitInterest(n)` | Pinned = 1.0, Bookmarked = 0.8, Tagged = 0.5, None = 0.0 | `{0.0, 0.5, 0.8, 1.0}` |
| `DistanceFromFocus(n)` | BFS hop distance from selected/hovered node, normalized to `[0.0, 1.0]` over `max_hops` | `[0.0, 1.0]` |

Default weights: `α = 0.30`, `β = 0.20`, `γ = 0.30`, `δ = 0.20`. All weights must sum to 1.0. Weights are configurable via `CanvasRegistry` policy; the defaults are the starting point, not a fixed contract.

### DOI Score Tiers

| Tier | Range | Rendering Behavior |
| --- | --- | --- |
| `High` | `>= 0.65` | Full node size, full label, full opacity, full color |
| `Medium` | `0.30 – 0.64` | Normal node size, domain-only label, full opacity |
| `Low` | `0.10 – 0.29` | Dot-only, muted color, no label |
| `Ghost` | `< 0.10` | Very faint dot; structural context preserved, visual noise minimized |

`Ghost` tier nodes are **never hidden entirely** — they provide the structural context that makes semantic fisheye meaningful. Hiding `Ghost` nodes is the responsibility of the search/filter system, not DOI.

### DOI Score Cache Shape

DOI scores are cached in `MetadataFrame` per graph pane alongside the existing LOD zoom oracle:

```rust
pub struct DoiScore {
    pub value: f32,           // [0.0, 1.0]
    pub tier: DoiTier,        // High / Medium / Low / Ghost
    pub computed_at: Instant, // for staleness check
}

pub enum DoiTier {
    High,
    Medium,
    Low,
    Ghost,
}
```

The cache (`HashMap<NodeIndex, DoiScore>`) is write-owned by the DOI calculator worker and read-only from the render path. Reads require no lock (snapshot swap pattern) to avoid stalling the render thread.

---

## Update Cadence

### Background Calculator

DOI scores are recomputed on a background worker, **not per-frame**. The cadence targets:

| Trigger | Cadence |
| --- | --- |
| No focus change, no structural change | **100 ms** (time-based tick) |
| Focus change (node selected or hovered) | **Immediate** — enqueue next tick ≤ 16 ms; `DistanceFromFocus` component changes |
| Structural change (node or edge added/removed) | **Immediate** — enqueue next tick; `Recency` and `Frequency` may change |
| Graph idle (no interaction for > 5 s) | **Suspend** — stop ticking; resume on next interaction event |

The worker holds a read-only snapshot of node metadata. It does not hold the graph write lock during calculation.

---

## Cost Guardrails

| Guardrail | Target |
| --- | --- |
| Per-recalculation wall time | **≤ 2 ms** for graphs up to 1 000 nodes |
| Cache read cost per frame | **O(1)** lookup by `NodeIndex` key |
| BFS max depth (`max_hops`) | **5 hops** — nodes beyond 5 hops get `DistanceFromFocus = 1.0` without traversal |
| Background thread CPU budget | **≤ 5 % single-core** at 100 ms cadence on reference hardware |
| Staleness threshold | **500 ms** — if cache entry is older than 500 ms and no recompute has run, the render path falls back to zoom-adaptive LOD-only mode (no DOI tier applied) |
| Fisheye per-frame cost | **O(visible_nodes)** — one multiply per culled-visible node; no BFS or cache required |

---

## Rendering Behavior Separation

**Key design rule**: DOI drives *rendering emphasis only*. It does not hide, remove, or filter nodes from the graph. Node visibility for filtering is governed by the separate search/filter system. These two concerns must not be entangled.

| Concern | Owner | DOI's Role |
| --- | --- | --- |
| Node visible / hidden (filter) | Search/filter system | None — DOI does not control visibility |
| Node size | DOI render layer | Scales radius by tier (see table below) |
| Node opacity | DOI render layer | Fades toward `Ghost` tier threshold |
| Label LOD (full / domain / none) | DOI render layer | DOI `High` may promote a node to full label even at zoom levels where zoom-adaptive LOD alone would suppress it; DOI never *demotes* below the zoom-LOD floor |
| Node color / brightness | DOI render layer | Mutes color toward `Low` / `Ghost` tier |
| Z-order in render pass | DOI render layer | High-DOI nodes render on top of low-DOI nodes within the same frame |

### Size Scaling

```
rendered_radius = base_radius * size_scale(tier)
```

| Tier | `size_scale` |
| --- | --- |
| `High` | `1.5` |
| `Medium` | `1.0` |
| `Low` | `0.6` |
| `Ghost` | `0.3` |

### Opacity Mapping

| Tier | Alpha |
| --- | --- |
| `High` | `1.0` |
| `Medium` | `0.85` |
| `Low` | `0.50` |
| `Ghost` | `0.15` |

---

## Semantic Fisheye (Focus + Context)

Semantic fisheye applies cursor-distance scaling on top of DOI tier rendering. It is a separate, independently toggleable pass.

**Invariant**: node `(x, y)` positions are **never modified**. Only draw-size changes. Graph topology remains undistorted.

### Algorithm

For each visible node in the current frame:

```
dist  = length(mouse_pos - node_pos)
scale = max(1.0, 3.0 * (1.0 - dist / fisheye_radius))
```

where `fisheye_radius` is a configurable canvas-space radius (default: `300.0` canvas units, stored in `CanvasRegistry`).

Apply the fisheye scale on top of the DOI-tier size:

```
final_radius = doi_radius * scale
```

Render pass z-order: sort visible nodes by `final_radius` ascending (paint smallest first; largest — most focused — on top).

### Fisheye Cost Guardrails

| Guardrail | Target |
| --- | --- |
| Per-frame cost | **O(visible_nodes)** — one distance + multiply per visible node |
| Input source | Reuse existing egui hover/mouse state; no additional polling |
| Disable path | `semantic_fisheye_enabled = false` in `CanvasRegistry`; render path skips scale pass entirely |

---

## Architecture Integration

- **`CanvasRegistry`**: policy toggles and parameters:
  - `doi_rendering_enabled: bool`
  - `semantic_fisheye_enabled: bool`
  - `doi_update_cadence_ms: u32` (default: 100)
  - `doi_max_hops: u8` (default: 5)
  - `fisheye_radius: f32` (default: 300.0)
  - `doi_weights: DoiWeights` (α, β, γ, δ; must sum to 1.0)
- **`MetadataFrame`**: DOI score cache (`HashMap<NodeIndex, DoiScore>`) stored alongside the LOD zoom oracle; snapshot-swap updated by worker, read-only to render.
- **DOI calculator worker**: background task owned by graph-pane context; reads graph snapshot + node visit metadata; writes snapshot to `MetadataFrame` cache.
- **Render path** (`render/graph_node_shape.rs` or equivalent): reads cached `DoiScore` per node; applies size/opacity/LOD tier overrides; applies fisheye scale when `semantic_fisheye_enabled`.
- **Multi-pane rule**: DOI scores and fisheye calculations are computed **per graph pane**, consistent with existing culling/LOD per-pane isolation (`2026-02-22_multi_graph_pane_plan.md`).

---

## Prerequisites (Blocking)

The following must be in place before implementation begins:

1. **Viewport culling** (`2026-02-24_performance_tuning_plan.md` Phase 1) — DOI rendering should only iterate over the culled visible node set to stay within the 2 ms per-recalculation budget.
2. **Node Label LOD** (`2026-02-24_performance_tuning_plan.md` Phase 2.1) — DOI LOD tier overrides zoom-adaptive LOD *upward* only; the zoom-adaptive LOD path must be stable before DOI can layer on top of it.
3. **`MetadataFrame` available as a stable per-pane cache oracle** — DOI cache fields are added here; the frame struct must be settled before new fields are introduced.

---

## Validation Targets

- [ ] DOI score recomputation completes in ≤ 2 ms for a 1 000-node graph on reference hardware.
- [ ] DOI cache hit rate ≥ 99 % during interactive hover/selection at 100 ms cadence.
- [ ] `doi_rendering_enabled = false` produces rendering identical to pre-DOI baseline (no visual regression).
- [ ] `semantic_fisheye_enabled = false` does not affect DOI tier-based size/opacity/LOD behavior.
- [ ] Node `(x, y)` positions are unchanged by the fisheye pass — verified by snapshot comparison of node position map before and after.
- [ ] High-DOI nodes render on top of low-DOI nodes within the same render pass.
- [ ] `Ghost` tier nodes remain visible as faint dots; they are not hidden by the DOI layer.
- [ ] All DOI + fisheye policy toggles are governed by `CanvasRegistry` and not hardcoded in render callsites.
- [ ] DOI rendering does not mutate graph state; it is a read-only render pass over cached scores.
