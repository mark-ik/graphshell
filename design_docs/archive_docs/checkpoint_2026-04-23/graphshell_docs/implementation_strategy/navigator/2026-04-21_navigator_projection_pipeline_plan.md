<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Navigator Projection Pipeline Plan

**Date**: 2026-04-21
**Status**: Archived — canonical spec produced
**Scope**: Produce the missing `navigator_projection_spec.md` (referenced from
NAVIGATOR.md §12 related-docs but not present), plus the cross-cutting
composition/annotation/portal/diff rules that Navigator sections, graphlets,
and specialty layouts all depend on. Align Navigator projection behavior with
the existing graph-memory substrate and SUBSYSTEM_HISTORY traversal authority.

> **Archive note (2026-04-23)**: This producing plan moved to
> `archive_docs/checkpoint_2026-04-23/` after the canonical active contract
> landed in `graphshell_docs/implementation_strategy/navigator/navigator_projection_spec.md`.
> Keep this file as design history and findings, not as the active projection
> authority.

**Authority discipline**:

- The Navigator domain (NAVIGATOR.md) remains the sole policy authority for
  this work; this plan and its produced spec define contracts and shape, not
  policy.
- Traversal/history truth remains owned by SUBSYSTEM_HISTORY; all projection
  work here reads through history-owned aggregates or graph-memory projections,
  never defines a parallel recents store (SUBSYSTEM_HISTORY §0A.7).
- Graph-memory substrate decisions stay owned by
  `2026-04-17_graph_memory_architecture_note.md`; this plan consumes, does not
  redesign.

**Related**:

- [NAVIGATOR.md](NAVIGATOR.md) — Navigator domain spec and authority
- [navigator_interaction_contract.md](navigator_interaction_contract.md) — click grammar
- [navigator_backlog_pack.md](navigator_backlog_pack.md) — NV01–NV25 and scenario track
- [2026-04-09_constellation_projection_plan.md](2026-04-09_constellation_projection_plan.md) — first specialty projection
- [../subsystem_history/SUBSYSTEM_HISTORY.md](../subsystem_history/SUBSYSTEM_HISTORY.md) — traversal authority and shared-projection policy
- [../subsystem_history/2026-04-17_graph_memory_architecture_note.md](../subsystem_history/2026-04-17_graph_memory_architecture_note.md) — substrate lineage and invariants
- [../subsystem_history/2026-03-18_mixed_timeline_contract.md](../subsystem_history/2026-03-18_mixed_timeline_contract.md) — mixed timeline consumed by time-axis projection
- [../../technical_architecture/graphlet_model.md](../../technical_architecture/graphlet_model.md) — graphlet semantics Navigator projects
- [../../technical_architecture/graph_tree_spec.md](../../technical_architecture/graph_tree_spec.md) — ProjectionLens integration target
- [../../technical_architecture/domain_projection_matrix.md](../../technical_architecture/domain_projection_matrix.md) — cross-domain projection catalog and mechanism inventory
- [../system/register/SYSTEM_REGISTER.md](../system/register/SYSTEM_REGISTER.md) — registry pattern for annotation contributions
- [../../TERMINOLOGY.md](../../TERMINOLOGY.md) — canonical terms (note: `Lens` is Layout+Theme+Physics+Filter and is NOT reused in this plan)

---

## Projection Pipeline Plan

The goal is a single canonical spec (`navigator_projection_spec.md`) that
defines the five-stage projection pipeline every Navigator row, graphlet, and
specialty layout passes through, plus the composition rules and cross-cutting
primitives (annotation registry, portal gestures, projection diff).

### Terminology note (load-bearing)

- `Lens` is reserved for Layout+Theme+Physics+Filter composition
  (TERMINOLOGY.md Visual System). This plan does **not** overload `Lens`.
- The term for a configured projection pipeline is **Projection Spec**.
- The term for composing multiple projection specs is **Projection
  Composition** (not "lens stack").
- `Section` remains the canonical section-model term (Recent, Frames, Graph,
  Relations, Import Records — NAVIGATOR.md §8).
- `Graphlet` retains its canonical semantics (anchors, backbone, migration
  proposals — TERMINOLOGY.md Tile Tree Architecture).

### Phase A — Canonical spec: `navigator_projection_spec.md`

Produce the spec file in `navigator/` with these sections. Each section has a
definition-of-done that can be evidenced by a same-session reviewer.

**A1. Projection Pipeline (five stages)**

Every Navigator output — section, graphlet derivation, specialty layout,
overview swatch — is produced by a pure pipeline with these stages:

1. **Scope** — which subgraph is under consideration (active node, anchor set,
   frame, cluster, time-window, recent-session, user selection, full graph).
2. **Shape** — how the scoped subgraph collapses into a readable structure
   (tree with parent-picker, ranked list with scorer, graphlet with derivation
   rule, specialty layout like constellation/corridor/atlas/timeline, or
   summary like overview swatch).
3. **Annotation** — what discarded structure is surfaced as compact hints
   (see A3 registry).
4. **Presentation** — which Navigator host form factor renders it (Sidebar /
   Toolbar; NAVIGATOR.md §12 hosts).
5. **Portal** — how click-behavior routes back into the graph/workbench
   authorities (see A4 gestures).

Pipeline is pure over `(graph truth, graph-memory projections, workbench
arrangement state, projection spec, host state)`. No projection output is
persisted. Projection specs and per-host configuration persist in
`WorkbenchProfile` (NAVIGATOR.md §12.6); outputs never do.

**A2. Composition Rules**

When two projection specs compose (e.g., `cluster` × `recency-scorer`, or
`constellation` × `time-window`), the composition is defined by:

- Which stages are short-circuited by the outer spec vs. delegated inward.
- Ordering: inner produces a candidate node set + structure; outer re-projects
  or re-orders within it.
- Conflict resolution when inner and outer disagree on scope or root set
  (outer wins; inner's contribution becomes scope-restricted).
- Invalid compositions are rejected at spec level with a `whyInvalid` reason.
- Valid initial compositions the spec must enumerate:
  - `cluster` × `recency-scorer` → trees per cluster, children recency-ordered
  - `frame-scope` × `graphlet` → local neighborhood restricted to one frame
  - `time-window` × `constellation` → constellation with temporal axis
  - `cluster` × `constellation` → constellation frontier candidates grouped by
    cluster membership

Composition fills the biggest gap in the prior draft: what stacking actually
*means* when two specs meet.

**A3. NodeAnnotation Registry**

Promote row annotations to a registered primitive following the atomic-registry
pattern (system/register/SYSTEM_REGISTER.md). New lenses, sections, and
subsystem surfaces contribute `NodeAnnotation` implementations without
modifying Navigator row rendering code.

Built-in annotation contributors (from existing canonical surfaces):

- cross-link count (from `EdgePayload.kinds` beyond the primary projected
  relation)
- cluster / frame-membership chip (from `ArrangementRelation` edges and
  cluster membership)
- recency / activity heat (from graph-memory `AggregatedEntryEdgeView` +
  `compositor:tile_activity` ring; consumed through SUBSYSTEM_HISTORY
  projections only — never a parallel index)
- trust / permission summary (NAVIGATOR.md §11A — already canonical; this
  plan formalizes it as an annotation, not new policy)
- focused-content status badge (NAVIGATOR.md §11B — same)
- hidden-neighbors count
- "also in" section cross-references

**Density rule**: annotation visibility is a property of the active projection
spec's `annotation_stack`, not a global toggle. Minimal by default; more
annotations render only when the active spec explicitly requests them.

**A4. Portal Gesture Taxonomy**

Extend NAVIGATOR.md §7 I5 (Reveal invariant) with three distinct gestures
that together define "portal back into graph/workbench context":

- **Locate** — pan camera to node, highlight, rest of graph stable. Default
  primary click on a node row (already live per I4 and I5).
- **Reveal-in-place** — the projection itself expands: a cross-link badge
  becomes a stub edge inside the sidebar; a cluster chip exposes its members
  inline. Does not move the camera and does not change selection.
- **Lift** — the sub-projection spills onto a canvas overlay, preserving its
  projected layout, as a temporary view. Routes through workbench authority
  (pane open / arrangement mutation), not Navigator-owned mutation. Not a
  synonym for pane promotion; the lift overlay is ephemeral unless explicitly
  promoted.

All three gestures must route through graph or workbench intents — not
Navigator-local state mutation (NAVIGATOR.md §4, reducer-only mutation).

**A5. Projection Diff (animation primitive)**

When a Navigator host switches projection specs, scrubs a time-axis cursor,
or re-runs after a refresh trigger, the transition must animate, not
teleport. The animation primitive is a **projection diff**: for each node
present in both projections, tween position, grouping, and annotation state;
entering/leaving nodes fade from neighbor positions.

Projection diff is a v1 requirement: without it, composition and time-axis
scrubbing feel like mode-switching rather than continuous camera-in-
projection-space. The spec must cover:

- Identity rule for duplicate rows (multi-parent visit or multi-section
  membership).
- Timing envelope (default 180 ms ease-out; configurable per host).
- Refresh-trigger vs. user-initiated transition distinction (refresh triggers
  from A1's signal path may animate only the delta).
- Cost bound: diff is not permitted to block the phase-3 signal publish
  (`phase3_publish_workbench_projection_refresh_requested`).

**A6. Time-Axis Specialty Projection**

Add time-axis as a specialty layout in the NAVIGATOR.md §8A family. It
renders nodes along a temporal axis with a scrubbable cursor; cursor position
defines an effective `time-window` scope for downstream composition.

The time-axis projection:

- Consumes `mixed_timeline_entries` exclusively (SUBSYSTEM_HISTORY
  `2026-03-18_mixed_timeline_contract.md`) — does not open a parallel index.
- Applies to one Navigator host at a time; cursor scope is per-host, not
  global (see open question C5).
- Is valid as outer spec in composition (time-axis × constellation, time-axis
  × cluster).
- Is not a second Navigator instance — preserves NAVIGATOR.md §12 "one
  Navigator, many hosts" rule.

**A7. Projection ↔ Layout inheritance**

Projection specs declare `layout_inheritance`: `own` (list, tree, time-axis),
`canvas` (graphlet, overview swatch — inherits canvas layout coords), or
`canvas-compressed` (minimap-like summaries). Explicit declaration lets
layout-derivative specs get cheap updates on canvas layout ticks and lets
layout-independent specs debounce layout independently.

**A8. Cost Classification**

Projection specs declare `cost_class`:

- `live` — recomputes on every relevant refresh trigger (per-frame ok)
- `debounced` — batches over a cadence
- `on-demand` — runs only when the host opens the spec or user refreshes

`live` is further split into **incrementally updatable** (delta-applied from
refresh trigger payload) vs **recompute-from-scratch**. A live spec that
runs O(n log n) or worse must be incrementally updatable or downgraded.

### Phase B — Cross-reference updates (same session as spec)

- **B1.** Update NAVIGATOR.md §12 related-docs link to point at the new spec
  (currently a broken reference). Do not move policy into the new spec —
  NAVIGATOR.md remains the policy authority per DOC_POLICY §11.
- **B2.** Update `navigator_backlog_pack.md` to map existing NV-IDs onto
  projection pipeline sections:
  - NV01 → A1 pipeline boundary
  - NV04 → A1 stage 1 (Scope) and §8 Section Model alignment
  - NV10 → A1 refresh-trigger signal-path reference
  - NV14 → A1 + A8 (Recent as recency-scored projection, cost class declared)
  - NV15 → A1 + §8 Frames / NAVIGATOR.md §8A graphlet projection
  - NV18 → A3 annotation registry (cross-link / relation-family surfacing)
  - NV24A–C → A7 layout inheritance + A8 cost class for overview swatch
  - Add new IDs for A3 (registry), A4 (lift/reveal-in-place), A5 (diff),
    A6 (time-axis), A7 (layout inheritance), A8 (cost classification).
- **B3.** Cross-link SUBSYSTEM_HISTORY §4.1 Navigator traversal projection to
  the time-axis specialty projection section (no policy move — just a
  reference that Navigator consumes `mixed_timeline_entries`).
- **B4.** Update DOC_README.md index to include the new spec and this plan,
  per rule 6.1.

### Phase C — Open questions (carried in spec or resolved before it ships)

- **C1. Composition conflict resolution (invariants).** When inner and outer
  specs disagree on scope set (e.g., inner graphlet includes node N, outer
  time-window excludes it), the outer-wins rule is a starting point — but
  the spec must say whether excluded-by-outer members render as annotation
  hints ("also exists outside window") or disappear entirely. Propose:
  annotation hint, with a per-spec `conflict_mode: hint | hide` override.
- **C2. Portal identity for duplicate rows.** A multi-parent visit (or a
  node appearing in multiple sections) has multiple projected row positions.
  Portal gestures must pick one:
  - Locate: always uses canonical graph-truth position — no ambiguity.
  - Reveal-in-place: uses the clicked row's position.
  - Lift: uses the clicked row's subtree.
  Projection diff identity: keyed by `(node_id, projection_path)`, not
  `node_id` alone.
- **C3. NodeAnnotation cost bounds.** Each annotation contributor declares
  its own cost class. Registry must reject live contributors whose cost
  exceeds a declared budget under a reference graph size (open: what
  reference size?).
- **C4. Projection diff timing policy.** Per-host override vs. single
  system-wide envelope? First cut: single envelope, per-host override
  deferred behind a flag.
- **C5. Time-axis cursor scope.** Per-host (simpler, may feel fragmented
  across hosts) vs. global (one cursor drives every host's time-window,
  more coherent). First cut: per-host cursor, with a host-option to bind
  to a shared cursor (deferred to a follow-on).
- **C6. Lift promotion.** Whether a lifted sub-projection can be promoted
  into a durable workbench frame via a user gesture, or remains
  always-ephemeral. First cut: always-ephemeral; promotion is a separate
  explicit graphlet-fork action routed through graph authority.
- **C8. ProjectionLens vs projection pipeline naming convergence.** GraphTree
  defines `ProjectionLens` (graph_tree_spec.md,
  workbench/2026-04-10_graph_tree_implementation_plan.md) as a mechanism
  intended to "collapse the Navigator/Workbench projection gap." This plan
  defines a five-stage projection pipeline as the Navigator's projection
  mechanism. These are both pipeline mechanisms for the same conceptual
  job; they may be the same thing under two names, or they may have
  diverged. Before the projection pipeline spec hardens, reconcile: is
  `ProjectionLens` the concrete implementation of this plan's pipeline
  stages (A1), a distinct but compatible mechanism, or a legacy name that
  should be retired? Carries the broader "projection is polymorphism
  across domain pairs" observation — see Findings note below on uses of
  the term `projection` in this codebase.

  **Resolution direction (2026-04-21):** `ProjectionLens` is **a Shape-stage
  mechanism for tree-family projections**, not a full pipeline. It is a
  Rust enum inside the `graph-tree` crate whose variants (Traversal,
  Arrangement, Containment, Recency, …) parameterize which edge family
  drives tree parent-child. The Navigator projection pipeline wraps it:

  - **Scope** — member filter (implicit in some lens variants, explicit
    scope config for others)
  - **Shape (tree-family)** — `ProjectionLens` + `LayoutMode` inside
    `GraphTree`
  - **Shape (non-tree)** — separate mechanisms still to be designed
    (graphlet-as-graph, time-axis, summary/minimap)
  - **Annotation** — pipeline-level (A3 registry); GraphTree does not hold
    annotation logic
  - **Presentation** — `GraphTreeRenderer` adapter (egui / iced / web) for
    tree shapes; other adapters for non-tree shapes
  - **Portal** — `NavAction` / `TreeIntent` inside GraphTree, extended
    with the A4 three-gesture taxonomy (Locate / Reveal-in-place / Lift)

  Outcome: `ProjectionLens` stays. It is the Shape-stage implementation
  for tree presentations. Non-tree Shape-stage mechanisms remain as
  future work wrapped by the same pipeline contract. Naming discipline
  from the Findings note holds: `ProjectionLens` is a **mechanism name**
  (compound noun), not a pattern. A projection produced via
  `ProjectionLens` is "a projected tree under the Recency lens" —
  pattern + outcome + mechanism aligned.

- **C7. Graph-memory consumer positioning.** Does the Navigator consume
  graph-memory through `branch_projection()` and `semantic_summary()`
  directly, or via a new runtime-projection adapter per the graph-memory
  note §8.1? Depends on whether the graph-facing runtime projection layer
  in that note is built alongside or after this spec. Prefer: build the
  adapter in the same slice as this spec, so Navigator consumers don't
  hard-code against `NodeNavigationMemory` directly and the per-node
  isolation assumption is not spread further (graph-memory note §6, §9).

  **Resolution direction:** the companion plan
  [2026-04-21_graph_runtime_projection_layer_plan.md](../subsystem_history/2026-04-21_graph_runtime_projection_layer_plan.md)
  produces that adapter layer (shortlist-leading name: Graph Cartography /
  GC; alternatives `graph-runtime-projection`, `graph-aggregates`). This
  plan's A1–A8 scorer / parent-picker / annotation slots are
  the pluggable surface GC's Phase 3.1 fills. Sequencing: substrate §8.3
  moves → GC layer contract → this spec's pipeline consumes GC outputs.
  Navigator must not reach into `NodeNavigationMemory` directly once GC
  is in place.

### Phase D — Non-deliverables (explicit)

The following do **not** belong in this plan or its spec:

- Redefining graph truth, traversal capture, or archive invariants
  (SUBSYSTEM_HISTORY authority).
- Changing graph-memory substrate shape (`K`, `E`, `X`, workspace-global
  vs per-node — `2026-04-17_graph_memory_architecture_note.md` §8.3
  authority).
- Defining workbench arrangement mutations (WORKBENCH.md authority).
- Redefining the section model (NAVIGATOR.md §8 is canonical; this plan
  adds composition rules on top of it, does not replace it).
- Introducing a second Navigator instance (NAVIGATOR.md §12 canonical rule).
- Overloading `Lens` (already Layout+Theme+Physics+Filter).

---

## Findings

Research from this session that shaped the plan:

- **Navigator is already its own domain** (NAVIGATOR.md, 2026-03-25). The
  prior Workbench-sidebar framing is superseded. Projection authority and
  graphlet derivation live here; arrangement remains under Workbench.
- **"Lens" is taken.** TERMINOLOGY.md defines it as
  Layout+Theme+Physics+Filter. The prior draft's "lens stack" concept must
  be renamed — this plan uses **Projection Composition** instead.
- **NAVIGATOR.md §12 references `navigator_projection_spec.md`** in its
  related-docs table, but the file does not exist in the tree. This plan
  produces that spec, closing the broken reference.
- **SUBSYSTEM_HISTORY policy §0A.7** explicitly forbids Navigator from
  defining a parallel recents store. Projections must read from
  history-owned aggregates. The prior draft's proposed "interaction log"
  already exists in this repo as `mixed_timeline_entries` over WAL entries
  (`NavigateNode`, `AppendNodeAuditEvent`, traversal events, structural
  events) — no new log is needed.
- **Graph Memory substrate is already well-factored.** The
  `2026-04-17_graph_memory_architecture_note.md` describes a generic
  `EntryRecord / VisitRecord / OwnerRecord / OwnerBinding` model with
  branch-preserving history, owner-scoped forward semantics, and snapshot
  persistence that rebuilds projections rather than serializing them. The
  prior draft's proposed "split structural and interaction logs" is not a
  fit — the substrate already treats temporal parentage as structural
  truth and runtime/contribution projections as separate downstream
  layers (note §8.1, §8.2). Navigator's projection pipeline aligns with
  §8.1's graph-facing runtime projection role.
- **Frame duality is already canonical.** Frames are graph-first
  organizational entities with `verso://frame/<FrameId>` addresses and
  `ArrangementRelation` edges; they project into Navigator sections and
  into canvas MagneticZones without a redesign. The prior draft's
  "frames as dual canvas object + navigator scope" is already true here.
- **Constellation projection (2026-04-09) is the first specialty
  projection in the family.** It defines anchor/frontier/related-cluster
  semantics. The projection pipeline this plan defines should be able to
  host constellation as one Shape stage output among several.
- **Existing NV backlog items map directly onto projection pipeline
  stages.** NV01 (boundary), NV04 (section mapping), NV10 (refresh
  triggers), NV14 (Recents), NV15 (arrangement), NV18 (edges/relations),
  NV24A-C (overview swatch) already name the shape of this work; the
  pipeline spec formalizes their shared substrate.
- **Annotation primitives already exist as separate chrome concerns** in
  NAVIGATOR.md §11A (trust/permission) and §11B (focused content status).
  Formalizing these as registry-contributed `NodeAnnotation` is a
  consolidation, not a new policy — the policies stay in NAVIGATOR.md.
- **Reducer-only mutation (system/2026-03-06) constrains portal
  gestures.** Lift and reveal-in-place must route through `GraphReducerIntent`
  or `WorkbenchIntent`, not Navigator-local mutations.
- **"Projection" is polymorphism across domain pairs, not a collision.** The
  term is used in at least three shapes: aggregation projections (Navigator,
  Cartography, contribution, branch, `AggregatedEntryEdgeView`);
  correspondence projections (TERMINOLOGY.md "nodes project as tiles");
  pipeline projections (projection pipeline, `ProjectionLens`,
  `ProjectionSpec`). All three instantiate `fn(source_domain_state, config)
  -> target_representation` at different domain pairs. The tightening move
  is discipline ("never say `projection` bare — always `X projection` or
  `projection of Y into Z`"), plus reconciling the one real mechanism-
  overlap (C8). A potential follow-on: `TERMINOLOGY.md` umbrella entry for
  "Projection" as a first-class pattern, and a `domain_projection_matrix.md`
  in `technical_architecture/` enumerating which domain pairs have named
  projections today.

---

## Progress

### 2026-04-21 — Plan draft session

- Read DOC_POLICY.md, DOC_README.md index, TERMINOLOGY.md (through
  Subsystems section), NAVIGATOR.md, navigator_backlog_pack.md, SUBSYSTEM_HISTORY.md,
  2026-04-17_graph_memory_architecture_note.md, 2026-04-09_constellation_projection_plan.md.
- Confirmed NAVIGATOR.md §12 references `navigator_projection_spec.md`
  which is absent from the tree — plan deliverable fills this gap.
- Confirmed `Lens` is taken; chose **Projection Spec** and **Projection
  Composition** as the canonical terms for this plan.
- Confirmed SUBSYSTEM_HISTORY §0A.7 shared-projection policy — Navigator
  projections consume history-owned aggregates, never a parallel store.
- Confirmed graph-memory substrate §8.1 graph-facing runtime projection
  role aligns with Navigator projection pipeline; C7 flagged so a new
  consumer does not harden per-node isolation assumptions further.
- Drafted Phase A (spec sections A1–A8), Phase B (cross-ref updates),
  Phase C (open questions C1–C7), Phase D (non-deliverables).
- Next session: produce `navigator_projection_spec.md` per Phase A,
  execute B1–B4 in the same slice, land C1/C2 resolutions in the spec and
  leave C3–C7 flagged for follow-on.

### 2026-04-21 — Completion slice

- Produced [navigator_projection_spec.md](navigator_projection_spec.md) as the
  canonical five-stage projection contract for Navigator hosts.
- Landed the C1/C2/C4/C5/C6/C7 resolution direction in the spec:
  `conflict_mode` defaults to `hint`, duplicate-row diff identity is
  `(node_id, projection_path)`, the default diff envelope is `180 ms ease-out`,
  time-axis cursor scope is host-local in v1, `Lift` remains ephemeral by
  default, and Navigator consumes Graph Cartography outputs rather than
  `NodeNavigationMemory` directly.
- Updated [navigator_backlog_pack.md](navigator_backlog_pack.md) with
  projection-pipeline mappings and new backlog IDs `NV24D`-`NV24I`.
- Updated [domain_projection_matrix.md](../../technical_architecture/domain_projection_matrix.md)
  so Navigator projection is no longer marked "spec in flight."
- Updated [DOC_README.md](../../../DOC_README.md) and related-doc references so
  the new spec is discoverable in the same session.

### Outstanding before implementation can start

- Graph Cartography implementation sequencing still blocks any runtime that
  would hard-code against substrate internals; the spec now assumes the GC
  adapter exists.
- Decide which built-in `NodeAnnotation` contributors ship in the first
  implementation slice versus follow-on slices after the registry primitive
  lands.
- Time-axis is now specified, but implementation should still respect
  `mixed_timeline_entries` query-correctness readiness in SUBSYSTEM_HISTORY.
