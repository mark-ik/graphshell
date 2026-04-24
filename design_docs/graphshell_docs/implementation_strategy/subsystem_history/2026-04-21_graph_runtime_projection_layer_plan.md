<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Runtime Projection Layer Plan

**Date**: 2026-04-21
**Status**: Planning / produces new crate + canonical spec
**Scope**: Produce the §8.1 graph-facing runtime projection layer named in
`2026-04-17_graph_memory_architecture_note.md`. This layer sits between the
`graph-memory` substrate + live graph + WAL aggregates and the consumers
(Navigator projection pipeline, History Manager, canvas summaries,
contribution assembly). It is the home for workspace-scoped aggregates,
co-activation statistics, cluster stability, frame-reformation priors,
relation-decay curves, and durable cached views that Navigator scorer /
parent-picker / annotation slots consume.

**Authority discipline**:

- `graph-memory` substrate shape remains owned by
  `2026-04-17_graph_memory_architecture_note.md`. This plan consumes, does
  not redesign — except where §8.3 near-term moves (workspace scope, richer
  `K`, real `X`) are explicit prerequisites.
- Traversal/history truth remains owned by SUBSYSTEM_HISTORY. This layer
  reads history-owned aggregates; it does not define a parallel recents
  store (SUBSYSTEM_HISTORY §0A.7).
- Navigator projection authority remains in NAVIGATOR.md. This layer fills
  pluggable slots declared by `navigator_projection_spec.md` (produced by
  the companion projection-pipeline plan); it does not own projection
  policy.
- Agent-style prediction remains owned by `AgentRegistry`. This layer
  exposes aggregates and derived structure to agents; it does not predict.

**Related**:

- [2026-04-17_graph_memory_architecture_note.md](2026-04-17_graph_memory_architecture_note.md) — substrate and §8.1 runtime-projection role
- [SUBSYSTEM_HISTORY.md](SUBSYSTEM_HISTORY.md) — traversal authority and shared-projection policy
- [2026-03-18_mixed_timeline_contract.md](2026-03-18_mixed_timeline_contract.md) — WAL timeline aggregate this layer consumes
- [2026-03-08_unified_history_architecture_plan.md](2026-03-08_unified_history_architecture_plan.md) — history taxonomy
- [../../../archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md](../../../archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md) — archived companion producing plan; C7 sequencing anchor
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — primary consumer
- [../system/register/SYSTEM_REGISTER.md](../system/register/SYSTEM_REGISTER.md) — `AgentRegistry` handoff surface
- [../graph/GRAPH.md](../graph/GRAPH.md) — truth authority this layer reads from
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — `EdgeKind` taxonomy, agent/action distinction

---

## Graph Runtime Projection Layer Plan

### Framing

The substrate (`graph-memory`) owns branch-preserving per-owner navigation
history — a tree shape that is correct for its job. The live graph holds
multi-parent, cross-link, directed/undirected, cyclic, multi-edge relations
as truth. Graph-not-tree structure is already captured by these two together;
`AggregatedEntryEdgeView` already exposes substrate data as a multi-graph at
the entry level.

What's missing is a **workspace-scoped aggregate layer** on top that:

- observes the substrate + live graph + WAL over time,
- derives durable aggregates (co-activation counts, entry-edge rollups,
  cluster stability, frame-reformation statistics, workspace priors),
- caches them with clear invalidation rules,
- exposes pluggable scorer / parent-picker / annotation slots to Navigator,
- hands off non-deterministic inference (task-group detection, bridge
  detection, likely-next-node) to `AgentRegistry` rather than inlining it.

This plan produces that layer.

### Phase 0 — Naming decision

The note calls it "graph-facing runtime projection." Candidate names, with
trade-offs:

| Candidate | Trade-off |
|---|---|
| `graph-cartography` | Strong metaphor match: cartographic projection (multidimensional truth → readable derived views), thematic overlays (hotspots, clusters, heat), scale decisions. Semantically adjacent to NAVIGATOR.md §11.7 Atlas vocabulary and the ambient-effects research. Composes well ("cartographic aggregate", "cartographic cache"). Slight risk of implying visual output (mitigated in docs); minor discoverability hit vs. literal names. |
| `graph-runtime-projection` | Matches the note's phrasing. Accurate but verbose. "Runtime" distinguishes from Navigator projection pipeline and VGCP contribution projection. |
| `graph-aggregates` | Literal, discoverable. Narrow to the deterministic aggregate responsibility; under-scopes P2.2 learned-affinity caches. |
| `workspace-memory` | Captures the workspace scope shift. Confusion risk vs. `graph-memory`. |
| `graph-priors` | Captures the "learned/observed priors" intent. Misses the deterministic-aggregate half. |
| `structural-memory` | Memory over structure vs. navigation. Reads as behavioral in ways that collide with the no-personalization-without-privacy-scope constraint. |
| `graph-projection` (user suggestion) | Rejected: "projection" is already overloaded six ways in this codebase (Navigator projection, projection pipeline, ProjectionLens in GraphTree, contribution projection). Instant ambiguity. |
| `graph-psychometry` (user suggestion) | Rejected: lexically wrong in technical writing (psychometry = measurement of psychological characteristics; the latent-attribute sense is metaphorical/fringe). |
| `nav-projection` (user suggestion) | Rejected: ties to one consumer; layer is multi-consumer. |
| `graph-isometry` (user suggestion) | Rejected: isometry preserves distance; this layer derives, does not preserve. |

Recommended shortlist: **`graph-cartography`**, `graph-runtime-projection`,
`graph-aggregates`. Final pick is a maintainer decision and may reveal a
further split (see Phase 2) that motivates naming two crates rather than one.

Until picked, this plan uses **Graph Cartography (GC)** as the placeholder
name.

### Phase 1 — Substrate prerequisites

The note §8.3 flags three substrate moves that should land before GC has
consumers (adding consumers before these hardens the wrong assumptions —
note §6, §9):

**P1.1 — Workspace-global instantiation of `graph-memory`.**
Move from per-node `NodeNavigationMemory` (one
`GraphMemorySnapshot<String, String, NodeHistoryOwner, ()>` per graph node)
to one workspace-scoped `GraphMemorySnapshot` with graph nodes or
node-presentations modeled as `OwnerRecord`s within that shared snapshot.
Consequences:

- spawn provenance becomes workspace-wide (new tab from here, new pane
  from here — substrate knows the relationship)
- cross-node entry identity becomes shared (one entry, many owner
  positions)
- per-node history views become owner-scoped projections over the shared
  tree, not isolated trees
- GC becomes workspace-coherent

**P1.2 — Less-primitive `K`.** `String` URL-only discards content change
at the same URL and diverges from VGCP identity projection. Options:

- canonical `(URL, content-hash)` composite key (matches VGCP direction
  from §3.1 of the note)
- node UUID + URL tuple (ties substrate entry identity to graph node
  identity when the node is promoted)
- substrate-owned opaque key with a separate `(URL, content-hash) →
  EntryKey` resolution table

The second option (node UUID + URL) is likely the right direction for
Graphshell because it makes substrate entries directly aligned with graph
nodes when they exist, and falls back to URL-only identity when they
don't. Decision lives in a follow-on; this plan's job is to surface the
constraint, not resolve it.

**P1.3 — Real `X` (visit context).** `X = ()` is a placeholder. The note
§3.2 sketches a minimal shape:

```rust
struct VisitContext {
    transition: Transition,
    referrer_entry: Option<EntryKey>,
    dwell_ms: Option<u64>,
}
```

GC's co-activation and dwell aggregates require at minimum `dwell_ms`
and `referrer_entry`. Ship these with P1.1 so persisted snapshots don't
have to migrate twice.

**P1.4 — Privacy boundary clarification.** Note §7.1 recommends the
`EntryPrivacy` enum move outward into a policy layer. GC is a natural
host for user-intent policy keyed by entry (or by aggregate row). This
plan should either absorb privacy-policy ownership explicitly or defer
it and flag the ownership gap.

### Phase 2 — Layer responsibility split

Two kinds of "memory" work are bundled in the initial framing and should
split cleanly inside GC, possibly into two modules/crates:

**P2.1 — Deterministic aggregates.**
Pure functions over (substrate, live graph, WAL timeline, session
boundary markers). Reproducible, test-friendly, cache-safe.

Examples:

- co-visit frequency (two entries appear in the same owner's branch
  within window W)
- co-activation frequency (two nodes active in the same session)
- entry-edge rollup (`AggregatedEntryEdgeView` elaborated with
  per-transition counts and session-bucketed variants)
- frame-reformation count (how many sessions a given frame membership
  pattern recurs across)
- traversal centrality (entries that frequently bridge branches)
- repeated-path detection (recurring parent→child→grandchild chains)
- last-activation freshness blended with revisit count

**P2.2 — Learned-affinity caches.**
Non-deterministic derivations (clustering, task-group detection, bridge
detection) produced by agents — this layer caches the output with
invalidation rules, it does not run the inference.

Examples:

- persistent cluster assignment state (centroid, member-set, label,
  confidence, last-recomputed timestamp)
- task-region membership (agent-produced)
- bridge-node annotation (agent-produced)
- stable-relation promotion candidates (agent says "these two nodes
  should probably be `AgentDerived` edged")

Split rationale:

- determinism and test properties differ
- invalidation rules differ (P2.1 invalidates on event; P2.2 invalidates
  on agent re-run)
- persistence shape differs (P2.1 is rollup tables; P2.2 is versioned
  agent-output snapshots)
- cost class differs
- crate split may become sensible if the split hardens

### Phase 3 — Consumer contracts

GC exposes read-only views to consumers. Consumers never mutate GC
state; GC mutates only through event-driven refresh or agent-run output.

**P3.1 — Navigator projection pipeline.**
GC provides the implementations for scorer / parent-picker / annotation
slots declared in `navigator_projection_spec.md`:

- `RecencyScorer` = last-activation blended with revisit count
- `ImportanceScorer` = traversal-centrality-based
- `TaskContinuationParentPicker` = repeated-path-derived parent
  suggestion (aggregate source) or agent-task-region membership (cache
  source)
- `CoActivationAnnotation` = "often active with N others"
- `StableClusterAnnotation` = persistent-cluster chip
- `BridgeAnnotation` = "bridges clusters A and B"

**P3.2 — History Manager.**
Annotated mixed-timeline rows (activity heat, revisit count), session
boundary hints, path-repetition markers.

**P3.3 — Canvas summaries.**
Hotspot edges, cluster halos, activity heat, bridge emphasis — all as
ambient visual effects sourced from GC aggregates (ties to
`2026-03-27_ambient_graph_visual_effects.md` research).

**P3.4 — Contribution assembly (VGCP).**
Per-contribution filtered aggregates and canonicalized entry-edge
rollups — contribution projection is a separate layer but may consume
GC's aggregate tables as input rather than re-walking the substrate.

### Phase 4 — Aggregate invalidation and persistence

**P4.1 — Event-driven invalidation.** GC subscribes to signals:

- substrate mutations (`visit_entry`, `ensure_owner`,
  `replace_linear_history`, `reset_owner`, `delete_owner`)
- graph truth mutations (`AddNode`, `RemoveNode`, edge assertions, tag
  changes)
- WAL timeline events (`NavigateNode`, `AppendNodeAuditEvent`)
- session boundary markers
- lifecycle transitions (Active / Warm / Cold / Tombstone)

Each aggregate declares which events invalidate it. Incremental updates
preferred; full recompute allowed on-demand.

**P4.2 — Persistence shape.** Two stores:

- aggregate tables (P2.1) — durable, rebuildable from substrate + WAL
  on demand, but cached to avoid hot-path recomputation
- agent-output snapshots (P2.2) — versioned, content-addressable, with
  re-run-on-mismatch semantics

Neither is canonical truth. Both are derivations, with the substrate +
live graph + WAL as the authoritative sources.

**P4.3 — Hysteresis policy.** Aggregates that feed Navigator projection
stability (cluster assignment, parent choice) must apply hysteresis:
don't reassign a node's cluster unless confidence has exceeded the
previous assignment's confidence by a threshold. This is load-bearing
for the "don't thrash projections" requirement split out in the earlier
critique.

### Phase 5 — Relation between GC aggregates and `EdgeKind`

Risk: GC aggregates that look edge-shaped ("co-visited frequently",
"often active together") could quietly become a shadow edge taxonomy.
Constraint:

- aggregates are **not** edges. They are rollup views over graph truth
  and substrate history.
- if an aggregate crosses a user-confirmable or agent-confirmable
  threshold and the system wants it to become a durable relation, it is
  promoted to `AgentDerived` (time-decayed) or proposed to the user for
  `UserGrouped` promotion.
- the current `EdgeKind` taxonomy (`UserGrouped`, `TraversalDerived`,
  `AgentDerived`, `Hyperlink`, `ContainmentRelation`,
  `ArrangementRelation`, `ImportedRelation`) is sufficient — GC does
  not add a `BehaviorDerived` kind. Behavior-derived structure lives as
  aggregate tables until promoted to `AgentDerived`.

Navigator renders GC aggregates as annotations (A3 in the projection
pipeline plan), not as edges on the graph — so "inferred relation" and
"authored edge" stay visually and semantically distinct.

### Phase 6 — Agent-inference handoff

GC does not predict. It exposes:

- an **agent input surface**: aggregate tables as read-only views
- an **agent output surface**: a typed ingestion point for agent-produced
  derivations (cluster assignments, task-group membership, bridge
  annotations) with versioning and invalidation metadata
- a **promotion surface**: when an agent output reaches threshold, GC
  emits a `GraphIntent` to propose an `AgentDerived` edge

This is the boundary between this layer (storage + aggregation) and
`AgentRegistry` (continuous, probabilistic, autonomous cognitive agents).

### Phase 7 — Privacy scope

Every aggregate and every agent-output cache declares a privacy scope at
birth:

- `LocalOnly` — never leaves the local Graphshell process
- `DeviceSync` — may sync across user's own devices via Verso
- `Shared` — may surface to Verse community projections

No implicit escalation. A `LocalOnly` aggregate cannot become a
`Shared` one without an explicit promotion path that aligns with the
contribution layer's canonicalization rules (VGCP §8, §15).

This section is the explicit slot referenced in graph-memory note §7.1's
warning about the substrate-level `EntryPrivacy` enum being too
substrate-near. GC may (or may not) absorb user-intent privacy policy —
P1.4 flags the decision.

### Phase 8 — Sequencing and dependencies

Direct dependencies:

- P1 substrate moves (workspace scope, `K`, `X`) must land before GC
  ships aggregates — otherwise GC hardens per-node-isolation
  assumptions that are expensive to unwind.
- Navigator projection pipeline spec (companion plan) must declare
  scorer / parent-picker / annotation slot shapes before GC fills them.

Parallel-safe:

- Phase 0 naming decision
- Phase 2 split into aggregates vs. learned-affinity caches (design-time)
- Phase 5 `EdgeKind` alignment (spec-time)

Follow-ons deferred:

- Exact aggregate inventory (Phase 4.1 initial set vs. follow-on
  aggregates)
- `K` shape decision (Phase P1.2)
- Privacy boundary ownership (Phase P1.4)
- `AgentRegistry` handoff wire format (Phase 6)

### Phase 9 — Non-deliverables

- Redesigning `graph-memory` substrate shape (the substrate is good; its
  §8.3 near-term moves are its own work, not rolled into this layer).
- Owning Navigator projection policy (NAVIGATOR.md authority).
- Running agent inference (AgentRegistry authority).
- Defining a new `EdgeKind` variant.
- Personalization model design (user-specific behavioral learning is a
  separate follow-on; this layer can host user-intent policy but does
  not define what "learning" means).
- Predictive UI features (likely-next-node, likely-lens); those are
  agent-fed, and this layer only exposes the aggregate surface an agent
  would consume.

---

## Findings

- **The substrate's tree shape is correct for its job.** Branch-preserving
  per-owner navigation history is a tree; attempting to flatten or
  multi-parent it inside the substrate would lose the invariants that make
  `history-tree` valuable (owner-scoped forward choice, branch
  preservation, GC semantics).
- **Graph-not-tree structure is already captured.** Live graph holds
  multi-parent/cross-link/cyclic relations via `EdgePayload.kinds` multi-set;
  substrate exposes `AggregatedEntryEdgeView` which is a multi-graph at the
  entry level. What's missing is a *layer* that sees both together and
  derives stable aggregates, not a substrate that stores them differently.
- **Runtime projection is the correct naming anchor** per the graph-memory
  architecture note §8.1. The suggested names `nav-projection` and
  `graph-isometry` were reviewed: `nav-projection` is too narrow (multi-
  consumer: Navigator, History Manager, canvas summaries, contribution);
  `graph-isometry` is technically incorrect (isometry preserves distance;
  this layer derives aggregates, not isometric views). Shortlist:
  `graph-runtime-projection`, `graph-aggregates`, `workspace-memory`.
- **Workspace-scope substrate move is a hard prerequisite** (graph-memory
  note §6, §9). Every new consumer on top of per-node `NodeNavigationMemory`
  hardens per-node-isolation assumptions. This layer cannot be built atop
  per-node-isolated memory without contradiction.
- **Deterministic aggregation vs. learned inference must split** (carried
  from the earlier critique). These have different determinism,
  invalidation, persistence, and cost properties. Initial plan proposes a
  module split; a crate split may emerge if the boundary hardens.
- **Agent prediction is not this layer's job.** `AgentRegistry` exists as
  the canonical home for "autonomous cognitive agents" (TERMINOLOGY.md).
  GC exposes aggregates to agents as input and caches agent outputs as
  versioned snapshots; it does not run inference itself.
- **`EdgeKind` taxonomy is sufficient; no shadow taxonomy.** Aggregates
  render as annotations (projection pipeline plan A3) or get promoted to
  `AgentDerived` edges. A `BehaviorDerived` kind is not needed.
- **Privacy scope must be declared at aggregate birth.** `LocalOnly /
  DeviceSync / Shared` mirrors the substrate's `EntryPrivacy` vocabulary
  at a more appropriate layer. No implicit escalation.

---

## Progress

### 2026-04-21 — Initial plan draft

- Clarified that "graph-not-tree memory" is already achieved by live graph
  + substrate `AggregatedEntryEdgeView` in combination; the missing piece
  is a workspace-scoped aggregate layer on top, not a substrate redesign.
- Confirmed §8.3 substrate moves (workspace-global, richer `K`, real `X`)
  are hard prerequisites, not parallel work.
- Drafted Phase 0–9, including the P2.1 / P2.2 split between deterministic
  aggregates and learned-affinity caches, and the agent handoff boundary.
- Flagged naming shortlist and the `nav-projection` / `graph-isometry`
  trade-offs.
- Cross-linked from the companion Navigator projection pipeline plan's
  C7 open question to this plan's Phase 1 and Phase 3.
- Updated DOC_README index in the same session (DOC_POLICY §6.1).

### Outstanding before implementation can start

- Crate/layer name decision (Phase 0).
- Confirm substrate §8.3 sequencing owner — does this plan drive those
  moves, or is there a separate graph-memory substrate plan that owns
  P1.1/P1.2/P1.3/P1.4?
- Initial aggregate inventory freeze — which of the Phase 4 examples ship
  in v1 vs. follow-on.
- Privacy-policy ownership decision (P1.4) — does GC absorb user-intent
  policy, or defer to a new policy layer?
- Confirm the Phase 6 agent handoff wire format with `AgentRegistry`
  owners before either side hardens an assumption.
