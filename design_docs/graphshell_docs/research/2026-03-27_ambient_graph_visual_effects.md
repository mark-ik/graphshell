# Ambient Graph Visual Effects

**Date**: 2026-03-27
**Status**: Research / Design Exploration
**Purpose**: Document a set of ambient visual effects for the graph canvas that
enrich legibility and spatial feel without requiring user interaction. These are
distinct from physics profiles and lens semantics — they operate at the
node/edge presentation layer, not the force model.

**Related**:

- `2026-02-24_interaction_and_semantic_design_schemes.md` — physics-as-semantics, lens model
- `../implementation_strategy/graph/2026-03-14_graph_relation_families.md` — relation families, family physics policy
- `../implementation_strategy/graph/layout_behaviors_and_physics_spec.md` — LensConfig, progressive switching, FamilyPhysicsPolicy
- `../../TERMINOLOGY.md` — Node, GraphViewId, EdgeKind, LensConfig

---

## 1. Motivation

The graph canvas currently encodes meaning through node position (force-directed
layout), edge presence, and label text. These are semantic channels but largely
static — the graph is a snapshot. A set of ambient, low-cost visual effects can
make the graph feel alive and legible without adding UI chrome or requiring
explicit user action.

The effects described here are **non-canonical**: they do not encode graph truth,
do not mutate graph state, and do not affect physics. They are purely
presentation-layer signals that ride on top of existing graph data.

---

## 2. Effects Catalogue

### 2.1 Temporal Decay

**What it does**: Node visual saturation and contrast drain slowly as time since
last visit increases. Recently visited or active nodes are vivid; old, untouched
nodes are muted. The graph becomes a passive recency heatmap.

**Data source**: `TraversalDerived` edge timestamps / last-visit metadata already
tracked by the History subsystem.

**Visual encoding**: HSL saturation scalar, range ~40%–100%. Contrast follows
the same scalar. No shape change. Decay curve is logarithmic so recently-visited
nodes stay vivid for a while before fading.

**Default**: On.

**Toggle scope**: Global settings + per-lens suppression (lenses with their own
color encoding, e.g. a containment lens, may want to suppress decay to avoid
fighting the lens color signal).

**Computational cost**: Low. One scalar per node, updated at visit events and
on a slow background timer. No per-frame geometry.

---

### 2.2 Graphlet Halos

**What it does**: When a graphlet is active (a lens is applied, a filter is
running, or a named graphlet is selected), the nodes belonging to that graphlet
share a faint convex-hull ambient glow. The boundary is felt rather than
outlined — no hard border, just a soft luminance field that dissolves as the
lens is removed.

**Data source**: Active graphlet membership, already derivable from the lens
projection.

**Visual encoding**: Per-graphlet color (derived from lens identity or
user-assigned graphlet color). Soft radial falloff beyond the convex hull
boundary. Alpha ~15–25% at full strength.

**Default**: On.

**Toggle scope**: Global settings. Individual lenses can declare
`suppress_halo: true` in their config if the halo would clash with the lens
visual language.

**Computational cost**: Low-medium. Convex hull computed on graphlet membership
change (not per-frame). Rendered as a background quad with a shader-driven
radial falloff.

---

### 2.3 Rhythm / Pulse

**What it does**: Nodes with ongoing background activity (agent crawl in
progress, Verse sync pending, content fetch in flight) emit a slow, subtle
radius oscillation — not a spinner, not a progress indicator, just a rhythm
that says "something is happening here." The oscillation calms when activity
resolves.

**Data source**: Per-node activity state from the diagnostics/async task layer
(already tracked for diagnostics channels).

**Visual encoding**: Sinusoidal radius multiplier, period ~2–3 s, amplitude
~5–8% of base node radius. No color change. Multiple simultaneous activities
do not stack amplitude — the pulse is binary (active / calm), not proportional.

**Default**: On.

**Toggle scope**: Global settings.

**Computational cost**: Very low. One phase offset per active node, computed
on the CPU, passed as a uniform to the node shader.

---

### 2.4 Warm-Node Particle Emission

**What it does**: Nodes that are warm (recently visited, have an open Viewer
tile, or have received fresh agent-derived edges) emit a small orbiting
particle cloud. Particle count = number of edges in the node's payload.
Particle color = edge relation family.

**Warmth definition**: A node is warm if any of the following are true:
- It has an open `TileKind::Node` pane in the current session.
- It was visited (traversal event) within a configurable recency window
  (default: current session).
- It has received a new `AgentDerived` edge within the last N minutes.

Warmth decays: particles fade out gradually as the node cools. Snap-off on
pane close is avoided — particles fade over ~10–30 s.

**Visual encoding**: Particles orbit the node at a radius slightly beyond the
node boundary. Orbit is the canonical directionality choice — it encodes
"present and active" without implying force direction. Orbit density (number
of particles) encodes edge count; orbit color mix encodes family composition.

**Relation family color mapping** (to be finalized at implementation; families
from `2026-03-14_graph_relation_families.md §2`):

| Family | Suggested hue |
| --- | --- |
| Semantic | Warm blue |
| Traversal | Amber |
| Containment | Green |
| Arrangement | Violet |
| Imported | Grey |

Traversal, Containment, Arrangement, and Imported families are
canvas-hidden by default (§2.2–2.5 of the relation families doc). Whether
their edges contribute to particle emission when the node is warm — even
though those edge types are not drawn — is an open design question. Initial
recommendation: **Semantic-family only** for the first implementation pass,
expanding to other families only if there is a clear legibility benefit.

**Default**: On (Semantic-family only).

**Toggle scope**: Global settings. Per-family participation toggleable
separately once multi-family emission is implemented.

**Computational cost**: Medium. Particle positions computed on CPU per warm
node per frame (or GPU-side if particle count warrants it). Budget: cap
particles per node at `clamp(log2(edge_count + 1) * k, min_particles,
max_particles)` to prevent Wikipedia-class nodes from dominating the scene.

---

### 2.5 Tidal Influence

**What it does**: Nodes near the camera center are foregrounded — slightly
larger, higher contrast. Nodes near the viewport edge are backgrounded —
slightly smaller, desaturated. Not a zoom operation; no clipping. An ambient
depth-of-field that makes the current focal area legible as the user pans.

**Data source**: Per-node distance from viewport center, computed from camera
transform.

**Visual encoding**: Size scalar range ~85%–115% of base radius. Saturation
scalar range ~70%–100%. Both scalars are smooth sigmoid functions of
normalized distance from viewport center. Center nodes are not dramatically
enlarged; edge nodes are not invisible — the effect is subtle.

**Default**: Off (configurable; off because it changes node sizes which
affects layout legibility and may surprise users).

**Toggle scope**: Global settings. Not per-lens — this is a viewport-level
effect, not a semantic one.

**Computational cost**: Very low. One scalar per node per frame, derived from
camera transform already computed for render.

---

### 2.6 Edge Tension Arcs

**What it does**: When a node is being pulled toward a cluster by the physics
engine, a faint tension arc briefly appears along the dominant force vector and
fades. The user sees *why* the graph moved, not just that it did. At rest, no
arcs are visible.

**Data source**: Force accumulator values from the physics simulation, sampled
per frame.

**Visual encoding**: Bezier arc from node center along force vector, alpha
proportional to force magnitude, max alpha ~20%. Fades over ~0.5 s after force
drops below threshold. Arc color matches the dominant edge family contributing
the force (using the same family color table as §2.4).

**Default**: Off (can be visually busy during layout settling; useful as a
diagnostic/educational mode).

**Toggle scope**: Global settings.

**Computational cost**: Medium. Arc geometry generated per active-force node
per frame. Cost scales with graph size and settlement activity; negligible at
rest.

---

## 3. Configurability Model

All effects are individually toggleable. Two toggle scopes exist:

| Scope | Mechanism |
| --- | --- |
| **Global on/off** | Settings surface entry under a "Graph Visual Effects" section. Persisted in user settings. |
| **Per-lens suppression** | `LensConfig` gains an optional `suppressed_effects: Vec<EffectId>` field. A lens can suppress any named effect when active. |

`EffectId` is a string enum: `temporal-decay`, `graphlet-halos`, `rhythm-pulse`,
`node-particles`, `tidal-influence`, `edge-tension-arcs`.

Default states:

| Effect | Default |
| --- | --- |
| Temporal decay | On |
| Graphlet halos | On |
| Rhythm / pulse | On |
| Warm-node particles | On |
| Tidal influence | Off |
| Edge tension arcs | Off |

The two off-by-default effects (tidal influence, edge tension arcs) are off
because they have higher perceptual impact or cost and should be opt-in. The
four on-by-default effects are low-cost and additive — they enrich without
dominating. All six should remain toggleable for users who prefer a minimal
presentation.

**Compound cost note**: The four default-on effects are all CPU-side scalar
updates feeding into existing render passes. Their compound cost at typical
graph sizes (tens to low hundreds of nodes) is expected to be negligible.
Particles and tension arcs (the off-by-default set) are the effects most
likely to require budgeting at scale.

---

## 4. Open Questions

1. **Warm-node particles: which families participate?** Initial recommendation
   is Semantic-family only. Revisit after first implementation pass.

2. **Temporal decay and lens color conflict**: When a lens uses color encoding
   (e.g., a containment lens colors nodes by depth), decay saturation may fight
   the lens signal. The per-lens suppression mechanism (§3) handles this, but
   the interaction should be explicitly tested.

3. **Graphlet halo color assignment**: Should halo color be derived from lens
   identity (auto-assigned), user-assignable per graphlet, or always a fixed
   neutral tint? User-assignable is the most powerful but adds a settings
   surface. Neutral tint is simplest. Recommendation: neutral tint first,
   user-assignable later.

4. **Pulse for multiple simultaneous activities**: Currently specified as
   binary (active/calm). If multiple distinct activity types are present
   simultaneously (fetch + agent crawl), a single pulse rate is ambiguous.
   Deferred — binary is correct for the first pass.
