<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Runtime Projection Layer Plan

**Date**: 2026-04-21
**Status**: Active — implementation pending P1 sequencing
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
- [../navigator/navigator_projection_spec.md](../navigator/navigator_projection_spec.md) — active projection pipeline spec; declares scorer/parent-picker/annotation slot shapes GC fills (Phase 3.1)
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — primary consumer
- [../system/register/SYSTEM_REGISTER.md](../system/register/SYSTEM_REGISTER.md) — `AgentRegistry` handoff surface
- [../graph/GRAPH.md](../graph/GRAPH.md) — truth authority this layer reads from
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — `EdgeKind` taxonomy, agent/action distinction

---

## Plan Details

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

Historical shortlist: **`graph-cartography`**, `graph-runtime-projection`,
`graph-aggregates`. The Phase 2 split may still motivate two modules or crates
later, but the layer name is no longer open.

**Decision (2026-04-24): `graph-cartography` selected.** This plan uses
**Graph Cartography (GC)** as the canonical name going forward.

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
at the same URL and diverges from VGCP identity projection. Options reviewed:

- canonical `(URL, content-hash)` composite key (matches VGCP direction
  from §3.1 of the note)
- node UUID + URL tuple (ties substrate entry identity to graph node
  identity when the node is promoted)
- substrate-owned opaque key with a separate `(URL, content-hash) →
  EntryKey` resolution table

**Decision (2026-04-24):** use a substrate-owned opaque `EntryKey` backed by a
resolution table that records `graph_node_id: Option<NodeId>`, normalized
locator/URL, and optional content fingerprint. This combines the node-alignment
benefit of the node UUID + URL option with the migration safety of the opaque
key option. Public GC APIs consume `EntryKey`; contribution projection remains
free to translate local entries into VGCP canonical `(URL, content)` identity
without making the runtime substrate equal to the wire format.

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
`EntryPrivacy` enum move outward into a policy layer. GC is the interim host
for user-intent policy keyed by entry or by aggregate/cache row. Phase 7 owns
the explicit privacy-scope contract until a dedicated policy layer exists.

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

**P4.4 — V1 aggregate inventory freeze (2026-04-24).** V1 ships the minimum
inventory needed to satisfy the Navigator projection slots, History Manager
annotations, canvas summaries, and agent handoff surface without creating a
second history store.

Deterministic aggregate tables (P2.1):

- `EntryEdgeRollup` — elaborates `AggregatedEntryEdgeView` with transition
  counts, latest traversal, session bucket, and privacy scope. Feeds hotspot
  edges, contribution assembly input, and traversal-derived annotations.
- `ActivationFreshness` — records last activation, revisit count, dwell rollup,
  and session bucket per entry/node. Feeds `RecencyScorer`, History Manager
  activity heat, and recency-list ordering.
- `TraversalCentrality` — computes branch/entry bridge frequency over the
  workspace-scoped substrate. Feeds `ImportanceScorer` and bridge-candidate
  input for agents.
- `RepeatedPathPrior` — records recurring parent -> child and parent -> child
  -> grandchild chains by owner/session window. Feeds
  `TaskContinuationParentPicker` without predicting likely-next-node.
- `CoActivationPair` — records node pairs active in the same session/window,
  with count, last seen, and decay metadata. Feeds `CoActivationAnnotation`
  and canvas activity overlays.
- `FrameReformationPattern` — records recurring frame membership patterns
  across sessions. Feeds canvas/workbench summaries and supplies a prior to
  agents; it does not mutate arrangement truth.

Learned-affinity cache tables (P2.2):

- `StableClusterAssignmentSnapshot` — agent-produced cluster membership,
  centroid, label, confidence, version, and hysteresis metadata. Feeds
  `StableClusterAnnotation` and cluster-scope Navigator specs.
- `TaskRegionMembershipSnapshot` — agent-produced task-region membership used
  as an optional cache source for `TaskContinuationParentPicker`.
- `BridgeNodeSnapshot` — agent-produced bridge annotation with source cluster,
  target cluster, confidence, and invalidation version. Feeds
  `BridgeAnnotation`.
- `StableRelationPromotionCandidate` — agent-produced candidate for
  `AgentDerived` relation promotion. GC stores and emits the proposal surface;
  graph truth still owns the actual edge mutation.

Explicit post-v1 follow-ons: likely-next-node prediction, user-personalized
behavioral models, cross-workspace/community aggregate merging, learned labels
without agent provenance, and any `Shared` privacy escalation path.

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

**Wire format authority (2026-04-24):** GC owns the agent handoff wire format
as the interim authority, since `AgentRegistry` is the only agent-side system
that exists today. Long-term, a dedicated intelligence or distillery subsystem
is the intended authority; `AgentRegistry` (and any successor) conforms to
whatever that layer declares. When that subsystem exists, the format definition
migrates there without breaking the GC ingestion surface.

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
substrate-near.

**Privacy policy authority (2026-04-24):** GC absorbs user-intent privacy
policy as the interim authority. The substrate-level `EntryPrivacy` enum should
be treated as deprecated-in-place; policy decisions migrate to the GC layer.
Long-term, a dedicated policy layer is the intended authority; GC will delegate
to it when it exists. Until then, GC is the single place where privacy scope is
declared and enforced. No implicit escalation rule applies regardless of which
layer formally owns it.

### Phase 8 — Sequencing and dependencies

Direct dependencies:

- P1 substrate moves (workspace scope, `K`, `X`) must land before GC
  ships aggregates — otherwise GC hardens per-node-isolation
  assumptions that are expensive to unwind.
- Navigator projection pipeline spec must declare scorer / parent-picker /
  annotation slot shapes before GC fills them. **Resolved:** active spec is
  `navigator/navigator_projection_spec.md`; slot shapes are declared in §5.2–5.3
  and §8.

Already resolved or parallel-safe:

- Phase 0 naming decision
- Phase 2 split into aggregates vs. learned-affinity caches (design-time)
- Phase 5 `EdgeKind` alignment (spec-time)

Resolved design decisions before implementation:

- V1 aggregate/cache inventory is frozen in P4.4.
- `K` shape is resolved in P1.2 as an opaque `EntryKey` with a resolution
  table.
- Privacy boundary ownership is resolved in Phase 7: GC is the interim policy
  authority.
- `AgentRegistry` handoff wire format ownership is resolved in Phase 6: GC owns
  the interim format until a dedicated intelligence/distillery subsystem exists.

Deferred beyond plan completion:

- Contribution projection producing plan and exact VGCP assembly rules.
- Dedicated policy-layer extraction after GC proves the local user-intent
  surface.
- Dedicated intelligence/distillery subsystem extraction after agent handoff
  formats stabilize.
- Threshold tuning for hysteresis, decay curves, and promotion candidates.

### Phase 9 — Non-deliverables

- Redesigning `graph-memory` substrate shape (the substrate is good). Note:
  the §8.3 near-term moves (P1.1–P1.4) are prerequisites driven by this plan
  as the authoritative owner — they are not substrate redesign, just the
  prerequisite wiring GC requires.
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
- **Graph Cartography is the selected name** for the graph-facing runtime
  projection layer named by the graph-memory architecture note §8.1. The
  suggested names `nav-projection` and `graph-isometry` were reviewed:
  `nav-projection` is too narrow (multi-consumer: Navigator, History Manager,
  canvas summaries, contribution); `graph-isometry` is technically incorrect
  (isometry preserves distance; this layer derives aggregates, not isometric
  views). The alternate shortlist names remain historical context only:
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
- **V1 inventory is now frozen.** The first implementation slice ships six
  deterministic tables (`EntryEdgeRollup`, `ActivationFreshness`,
  `TraversalCentrality`, `RepeatedPathPrior`, `CoActivationPair`,
  `FrameReformationPattern`) and four agent-output cache tables
  (`StableClusterAssignmentSnapshot`, `TaskRegionMembershipSnapshot`,
  `BridgeNodeSnapshot`, `StableRelationPromotionCandidate`).
- **Entry identity is opaque at the GC boundary.** `EntryKey` is substrate-owned
  and backed by graph-node, locator, and content-fingerprint resolution data;
  VGCP canonical identity remains a contribution-projection translation.

---

## Progress

### 2026-04-21 — Initial plan draft

- Clarified that "graph-not-tree memory" is already achieved by live graph
  and substrate `AggregatedEntryEdgeView` in combination; the missing piece
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

### 2026-04-24 — Plan completion pass

- Selected `graph-cartography` as the canonical layer/crate name.
- Confirmed this plan owns substrate prerequisite sequencing for P1.1–P1.4.
- Resolved P1.2: GC consumes opaque `EntryKey` values backed by a resolution
  table with optional graph node identity, normalized locator, and optional
  content fingerprint.
- Froze the v1 deterministic aggregate and learned-affinity cache inventory in
  P4.4.
- Resolved privacy-policy ownership: GC is the interim user-intent policy
  authority until a dedicated policy layer exists.
- Resolved agent handoff ownership: GC owns the interim wire format until a
  dedicated intelligence/distillery subsystem exists.

### 2026-04-25 — Initial implementation slice

- Added `crates/graph-cartography` as the first GC crate.
- Landed opaque `EntryKey`, `EntryResolution`, `VisitContext`,
  `WorkspaceOwner`, `WorkspaceGraphMemory`, and GC `PrivacyScope` contracts.
- Landed the P4.4 deterministic aggregate and learned-affinity cache row
  types.
- Implemented initial deterministic builders for `EntryEdgeRollup`,
  `ActivationFreshness`, `TraversalCentrality`, and `RepeatedPathPrior` over
  the existing `graph-memory` API.
- Verified the slice with `cargo test -p graph-cartography --lib`.

### 2026-04-25 — Deterministic table completion slice

- Implemented the remaining P4.4 deterministic builders for
  `CoActivationPair` and `FrameReformationPattern`.
- Current frame-pattern derivation uses deterministic owner/session cohorts
  from `VisitContext::session_bucket` until explicit workbench frame
  membership signals land.
- `DeterministicAggregateTables::from_memory` now fills all six v1
  deterministic aggregate tables.
- Verified the slice with `cargo test -p graph-cartography --lib` (8 tests).

### 2026-04-25 — Snapshot/cache boundary slice

- Added a versioned `CartographySnapshot` handoff shape around deterministic
  aggregate tables and learned-affinity cache tables.
- Added `LearnedAffinityCacheTables` as the first bundle-level cache container
  for the four P4.4 agent-output row contracts.
- Added explicit schema/table version constants so future consumer adapters and
  cache persistence can reject incompatible GC outputs before reading rows.
- Verified the slice with `cargo test -p graph-cartography --lib` (10 tests).

### 2026-04-25 — Snapshot query adapter slice

- Added read-only query helpers over `DeterministicAggregateTables` and
  `CartographySnapshot` so P3 consumers can ask for entry-local deterministic
  facts without reaching into table internals.
- Query helpers cover activation freshness, traversal centrality, inbound and
  outbound edge rollups, co-activation pairs, repeated path priors, and frame
  reformation patterns by `EntryKey`.
- This remains crate-local and non-invasive: no root runtime/app dependency on
  `graph-cartography` was added.

### 2026-04-25 — Snapshot/cache validation and invalidation helper slice

- Added learned-affinity cache query helpers for per-entry cluster, task-region,
  bridge, and relation-promotion rows.
- Added deterministic version collection and staleness detection for learned
  cache rows so future agent/cache producers can reject outdated rows before
  exposing them to consumers.
- Added `CartographySnapshotValidationError` plus snapshot/cache validation for
  version compatibility, empty membership rows, self-loop promotion candidates,
  and empty promotion reasons.
- Verified both follow-on slices with `cargo test -p graph-cartography --lib`
  (13 tests).

### 2026-04-25 — P3 projection-hint adapter slice

- Added owned, serializable `CartographyProjectionHints` as the first P3
  consumer-adapter handoff shape over `CartographySnapshot`.
- The adapter groups GC data by `EntryKey` into recency scorer hints,
  importance scorer hints, parent-picker repeated-path hints, and compact
  annotation hints for edge rollups, co-activation, frame reformation, stable
  clusters, task regions, bridge nodes, and stable relation promotion
  candidates.
- Privacy scope is combined across every emitted hint so downstream Navigator
  consumers can respect GC's aggregate/cache privacy boundary without
  re-reading raw tables.
- Verified the slice with `cargo test -p graph-cartography --lib` (14 tests).

### 2026-04-25 — Scoped projection hint-set slice

- Added `CartographyProjectionHintSet` as the batch/scoped handoff shape for
  Navigator-style projection runs that request GC hints for several `EntryKey`
  candidates at once.
- `CartographySnapshot::projection_hints_for_entries` preserves the first-seen
  scope order, deduplicates repeated requested entries, emits missing entries
  separately, and rolls up the effective privacy scope across all emitted
  hints.
- Verified the slice with `cargo test -p graph-cartography --lib` (15 tests).

### 2026-04-25 — History Manager annotation adapter slice

- Added owned, serializable `CartographyHistoryAnnotations` and
  `HistoryEntryAnnotation` handoff shapes for P3.2 History Manager consumers.
- `CartographySnapshot::history_annotations_for_entry` and
  `history_annotations_for_entries` now assemble activity heat inputs,
  revisit/dwell rollups, repeated-path markers, and co-activation peers from
  the existing deterministic aggregate tables without exposing table internals.
- Batch annotation requests preserve first-seen entry order, deduplicate input
  entries, skip entries without activation data, and combine emitted privacy
  scopes across all rows.

### 2026-04-25 — Canvas summary adapter slice

- Added owned, serializable `CartographyCanvasSummary` with hotspot edges,
  activity heat rows, stable-cluster halos, and bridge emphasis rows for P3.3
  canvas-summary consumers.
- `CartographySnapshot::canvas_summary` maps entry-level aggregates and
  learned-affinity cache rows back to graph-node identities via the activation
  freshness table, keeping the adapter crate-local and read-only.
- Verified both P3.2/P3.3 adapter slices with
  `cargo test -p graph-cartography --lib` (17 tests).

### Implementation readiness

- ~~Crate/layer name decision (Phase 0).~~ **Resolved 2026-04-24: `graph-cartography`.**
- ~~Confirm substrate §8.3 sequencing owner.~~ **Resolved 2026-04-24: this plan
  is the authoritative owner of P1.1–P1.4. No separate graph-memory
  implementation plan exists.**
- ~~Initial aggregate inventory freeze.~~ **Resolved 2026-04-24: v1 inventory
  is frozen in P4.4.**
- ~~`K` shape decision (P1.2).~~ **Resolved 2026-04-24: opaque `EntryKey`
  backed by graph-node, locator, and content-fingerprint resolution data.**
- ~~Privacy-policy ownership decision (P1.4).~~ **Resolved 2026-04-24: GC is
  the interim authority; future policy layer is the long-term home. See
  Phase 7.**
- ~~Confirm Phase 6 agent handoff wire format.~~ **Resolved 2026-04-24: GC
  owns the format as interim authority; distillery/intelligence subsystem is
  the intended long-term owner. See Phase 6.**

Implementation may start with P1 substrate prerequisites, then the P4.4 table
set, then P3 consumer adapters. Remaining deferred items are follow-on plans,
not blockers for GC v1.
