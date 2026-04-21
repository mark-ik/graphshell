<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Memory Architecture Note

**Date**: 2026-04-17
**Status**: Current code-backed architecture note
**Scope**: Document the `graph-memory` substrate now present in Graphshell,
its lineage from `history-tree`, the invariants it currently enforces, and the
boundary between the memory model itself and the next graph-facing and
contribution-facing projection work.

**Related**:

- `../../../../crates/graph-memory/src/lib.rs`
- `../../../../crates/graphshell-core/src/graph/mod.rs`
- [2026-03-08_unified_history_architecture_plan.md](2026-03-08_unified_history_architecture_plan.md)
- [2026-04-09_owner_scoped_temporal_branching_follow_on.md](2026-04-09_owner_scoped_temporal_branching_follow_on.md)
- `../../../verse_docs/technical_architecture/2026-04-17_verse_graph_contribution_protocol_v0_1.md` (§8, §9.2, §9.5, §11.1, §15)
- `../../../../../../source/repos/history-tree/history-tree.lisp`
- `../../../../../../source/repos/history-tree/README.org`

---

## 1. Why This Note Exists

Graphshell now has a real `graph-memory` crate, but the conceptual picture is
easy to blur because three layers sit close together:

- the upstream `history-tree` lineage,
- the new generic Rust substrate in `crates/graph-memory`,
- the current Graphshell wrapper that uses it as per-node navigation memory.

This note makes those boundaries explicit.

The practical reason to do this now is simple:

- `graph-memory` is already load-bearing enough to justify canonical docs,
- the next planned work is no longer just one projection layer,
- and those downstream layers should build on the memory substrate instead of
  accidentally redefining it.

The two immediate downstream consumers are different enough to name here:

- a graph/runtime projection for History Manager, Navigator, and canvas-facing
  summaries,
- a contribution projection for VGCP/VGCP-adjacent artifact assembly.

---

## 2. Lineage From `history-tree`

`graph-memory` is not a random reinvention. It is a Rust adaptation of the
same structural idea used by `history-tree`.

The shared concepts are:

- **Entry**: deduplicated identity of visited content.
- **Owner**: a cursor-bearing actor such as a tab, pane, or view.
- **Binding**: the owner-local relationship to a visited point, including the
  owner-specific forward choice.
- **Branch-preserving history**: going back and then visiting somewhere else
  does not destroy the previous forward path.
- **Creator/origin linkage**: one owner can spawn another from its current
  position.
- **Owner-scoped garbage collection**: branches are only collected when they
  become ownerless.

The key rename is:

- `history-tree` **node** becomes `graph-memory` **visit**.

That rename is deliberate. In Graphshell, the word "node" already belongs to
the graph domain, so the temporal occurrence layer uses `Visit` instead.

The other important shift is presentational:

- `history-tree` is described as a shared/global history tree.
- `graph-memory` is described as a graph-oriented memory substrate whose edges
  are projections over visit parentage rather than separately stored truth.

So the ancestry is direct, but the vocabulary is made safe for Graphshell's own
graph model.

---

## 3. Core Data Model

`graph-memory` has one structural authority: **visits own the tree**.

### 3.1 Entry

An `EntryRecord<K, E>` is the deduplicated identity layer.

It carries:

- a uniqueness key `K`,
- payload `E`,
- `first_seen_at_ms`,
- `last_seen_at_ms`,
- `visit_count`,
- privacy (`LocalOnly`, `ShareCandidate`, `Shared`).

Multiple visits may point at the same entry. This is how repeated navigation to
the same URL can remain semantically one resource while still producing
multiple temporal occurrences.

Two clarifications matter:

- the generic shape `<K, E>` is the right architectural move,
- the current Graphshell instantiation of that shape is still bootstrap-grade.

In particular, VGCP v0.1 treats canonical `(URL, content)` composition as the
important shareable entry identity, not URL alone. That matters because
Graphshell's current `K = String` URL key is URL-only; it discards content
change at the same URL, which VGCP's identity projection distinguishes.

Concretely, the current wrapper choice means two visits to the same URL with
materially different canonicalized content still collapse to one local entry in
`NodeNavigationMemory`. That is acceptable as a bootstrap shortcut, but it
diverges from VGCP's projection model and should be read as provisional.

The cleaner long-term shape is not necessarily for local memory to adopt VGCP's
exact identity scheme. The more defensible direction is:

- local memory grows a less-primitive `K` than raw URL string,
- local memory keeps the identity shape that is useful for Graphshell's own
  runtime needs,
- the contribution projection translates that local identity into VGCP's
  canonical `(URL, content)` entry identity at assembly time,
- the contribution projection can also derive exact-content equivalence
  fingerprints for cross-URL clustering without making those fingerprints the
  substrate's primary key.

So the pressure here is not "make the substrate equal VGCP." It is "do not let
the substrate stay so primitive that the projection layer must reconstruct all
meaning from unrelated state." See VGCP §8 and §9.2.

### 3.2 Visit

A `VisitRecord<X>` is one concrete occurrence in navigation history.

It carries:

- the referenced `entry`,
- `parent`,
- `children`,
- `created_at_ms`,
- context payload `X`,
- inbound transition metadata,
- per-owner bindings.

Every new arrival is a new visit. Re-visiting the same entry does not reuse an
old visit.

The important nuance is that `graph-memory` already stores the inbound
`TransitionKind` structurally via `TransitionRecord`; `X` is therefore not the
only place visit-local semantics can live. But Graphshell's current `X = ()`
still means the wrapper has not yet committed to any richer local visit context
beyond transition/time/parentage.

That should be treated as an unresolved substrate decision, not as a stable
shape. The near-term likely direction is that `X` grows into explicit local
behavioral metadata for a visit, while `TransitionRecord` remains the minimal
structural arrival record.

The intended kind of data is:

- transition semantics beyond the bare inbound kind,
- optional referrer/local-origin context,
- optional dwell or departure metadata,
- other local-only facts needed by runtime heuristics or contribution
  assembly.

A minimal future shape would look roughly like:

```rust
struct VisitContext {
    transition: Transition,
    referrer_entry: Option<EntryKey>,
    dwell_ms: Option<u64>,
}

enum Transition {
    LinkClick,
    UrlTyped,
    Back,
    Forward,
    Reload,
    Redirect,
    TabSpawn,
    Restore,
    Unknown,
}
```

The exact field set is still open, but the architectural point is not: `X = ()`
is placeholder, and it should be replaced before persisted snapshots and more
callers harden around the empty shape.

### 3.3 Owner

An `OwnerRecord<O>` is a cursor-bearing actor.

It carries:

- stable owner identity,
- `origin`,
- `current`,
- `creator`,
- `pending_origin_parent`,
- the set of `owned_visits`.

This is the owner-local scope that makes back/forward semantics and branching
local rather than global.

### 3.4 Binding

`OwnerBinding` is the per-owner relationship to a visit.

It carries:

- `forward_child`,
- `last_accessed_at_ms`.

This is the crucial owner-scoped rule from `history-tree`: the same visit may
have different default forward children for different owners.

### 3.5 Projected Edge Views

`graph-memory` also exposes projected graph views:

- `EdgeView`: one parent -> child visit edge.
- `AggregatedEntryEdgeView`: an entry-level rollup of repeated traversals.

These are projections over visit parentage. They are not stored as an
independent authority.

Current aggregation semantics are intentionally local and simple:

- `traversal_count` is a raw count of repeated parent -> child traversals,
- `latest_transition_at_ms` is the latest local visit timestamp seen for that
  entry-pair,
- `transition_counts` is a raw per-`TransitionKind` count map.

This is not a trust-weighted or community-aware aggregate. VGCP-style
attestation-weighted aggregation is a downstream concern and must be recomputed
at the contribution/community layer.

---

## 4. Persistence Shape

The persisted form is `GraphMemorySnapshot<K, E, O, X>`.

It stores three arrays:

- `entries`,
- `visits`,
- `owners`.

Visits and owners refer to each other by snapshot indices. Visit bindings are
persisted explicitly, so owner-local forward choices survive serialization.

Important boundary:

- snapshots persist structural truth,
- branch views and edge projections are rebuilt from that truth,
- they are not serialized as separate canonical state.

This is the right tradeoff for Graphshell because the durable substrate stays
small and composable while multiple UI or analytics surfaces can derive their
own read models from the same persisted memory.

---

## 5. Operational Rules And Invariants

### 5.1 Entry Deduplication, Visit Multiplicity

Entries are deduplicated by key. Visits are not.

That means:

- repeated navigation to the same resource updates one entry,
- but produces distinct visits,
- and repeated traversals can later be aggregated at the entry-edge layer.

### 5.2 Visits Own The Tree

The parent/child structure is attached to visits.

This matters because:

- branch preservation is temporal,
- owner-local cursor semantics are temporal,
- graph-style summaries should be projections over temporal structure, not a
  replacement for it.

### 5.3 Forward Is Owner-Scoped

`back()` walks to the visit parent.

`forward()` does not mean "pick the only child". It means:

- inspect the binding for this owner on the current visit,
- follow that owner's `forward_child`.

So one shared visit can forward to different children depending on which owner
is traversing it.

### 5.4 Owner Creation Preserves Provenance

`ensure_owner(identity, creator)` captures the creator relationship and the
creator's current visit as `pending_origin_parent`.

The first visit created for the spawned owner then attaches to that parent.

This is the substrate for "open in new tab/pane/view from here" semantics.

### 5.5 Branches Are Preserved Until Owner GC

The model is intentionally retentive.

- `visit_entry()` never overwrites an old child path.
- alternate-forward navigation creates sibling children.
- `delete_owner()` only collects branches that have become ownerless.
- `reset_owner()` collapses an owner's history to a new root at the current
  entry without deleting the owner itself.

This preserves temporal truth while still allowing bounded cleanup.

### 5.6 Rebinding Is Explicit

`rebind_owner_to_path()` is the low-level operation that lets Graphshell keep a
chosen owner path authoritative without flattening the underlying tree.

Operationally, it preserves these invariants:

- every visit in the supplied path must already exist,
- old bindings for that owner are removed from previously owned visits,
- `forward_child` is re-established only along the supplied path,
- `origin`, `current`, and `owned_visits` are rewritten for that owner,
- visit parent/child structure is not rewritten,
- alternate branches remain present in the tree even when they are no longer on
  the active owner path.

That is the key bridge from browser-style linear history updates into a
branch-preserving substrate.

---

## 6. Current Graphshell Integration

Today Graphshell uses `graph-memory` through `NodeNavigationMemory` in
`crates/graphshell-core/src/graph/mod.rs`.

That wrapper currently chooses a narrow but useful slice:

- one `GraphMemorySnapshot<String, String, NodeHistoryOwner, ()>` per graph
  node,
- one owner identity (`NodeHistoryOwner::Primary`) per node,
- URL strings as both entry key and payload,
- empty visit context for now.

So the current integration is **node-local navigation memory**, not a shared
workspace-global graph-memory fabric.

That is a real architectural commitment, not just an incidental wrapper choice.

Per-node snapshots mean:

- each graph node has its own owner namespace,
- each graph node has its own visit space,
- each graph node has its own GC lifecycle,
- cross-node owner continuity does not exist in the substrate,
- spawn/origin semantics only compose inside one node-local tree.

That matters because the original `history-tree` value proposition was not just
branch-preserving local history, but cross-owner-within-one-tree continuity.
If Graphshell eventually wants workbench-wide owner continuity such as
"open from here into another pane/view and keep that relationship inside one
memory tree," then a workspace-global memory fabric is the likely next
architectural move, not just one possible future expansion.

That point is stronger than a mere future option:

- per-node isolation treats each graph node as its own browser,
- workspace-global memory treats the workbench as one browser with many
  cursors/owners,
- the original `history-tree` design fits the second framing natively.

So the next architectural move worth naming explicitly is:

- one `GraphMemorySnapshot` per workspace,
- graph nodes or node-presentations modeled as owners within that shared
  snapshot,
- per-node history views expressed as owner-scoped projections over the shared
  tree rather than as isolated trees.

That would make these things first-class in the substrate instead of
reconstructed outside it:

- spawn provenance across the workbench,
- shared entry identity across multiple graph nodes/presentations,
- aggregate traversal over one contributor/workspace memory surface,
- coherent workspace-level GC and projection logic.

So the current per-node wrapper should be read as a bootstrap integration
boundary, not as proof that per-node isolation is the final desired shape.

Naming this now matters because every consumer of `projection()`,
`branch_projection()`, `semantic_summary()`, and `replace_linear_history()`
implicitly hardens around per-node isolation semantics. The migration is much
cheaper before those assumptions spread further.

Concretely: this decision should be made before any consumer outside
`graphshell-core/src/graph/mod.rs` takes a dependency on
`NodeNavigationMemory`. Every new caller hardens per-node isolation
assumptions that are expensive to unwind.

That wrapper already exposes four important read/write surfaces:

- `projection()`: linear history for browser-like consumers.
- `branch_projection()`: current owner path plus alternate children.
- `semantic_summary()`: current URL, last-visit time, visit count.
- `replace_linear_history()`: update from browser-provided linear history
  while preserving alternate branches where possible.

The important detail in `replace_linear_history()` is that it does not blindly
recreate memory on every update. If the root still matches, it tries to reuse
existing visits and only creates new visits where the browser history has
diverged.

In current code, "root still matches" means only this:

- the first existing entry payload equals the first incoming browser-history
  entry,
- which for the current wrapper means first-URL equality.

That is a deliberately weak bootstrap heuristic. It is good enough to preserve
branches across common browser-history updates, but it is not yet a deep entry
identity check and should not be mistaken for one.

The current wrapper also makes three important bootstrap choices that should be
named explicitly instead of treated as neutral defaults:

- `K = String` means local entry identity is URL-based today, which conflicts
  with VGCP's long-term canonical `(URL, content)` notion of shareable
  identity.
- `E = String` means the entry payload is also only URL-level today, so richer
  canonicalized entry metadata is not yet carried in the substrate wrapper.
- `X = ()` means Graphshell has not yet committed to a richer visit-local
  context payload, even though the generic slot exists and downstream
  projections will likely need it.

The two most important of these are substrate decisions, not wrapper trivia:

- whether the long-term local memory identity is URL-shaped or canonical
  content-shaped,
- whether visit-local context is persisted inside the visit record or pushed
  into side channels.

Near-term implication:

- the generic crate shape is ahead of the current Graphshell instantiation,
- but the instantiation will need to change if contribution assembly is meant
  to be cheap and direct instead of reconstructed from unrelated state.

That is how Graphshell gets branch preservation even when the incoming signal is
still a linear browser history list.

---

## 7. What `graph_memory` Is Responsible For

`graph-memory` is responsible for:

- deduplicated entry identity,
- visit-level temporal structure,
- owner-local cursor and forward semantics,
- branch preservation,
- owner spawn/origin provenance,
- owner-scoped garbage collection,
- derived edge projections over temporal structure.

It is not responsible for:

- graph-node layout,
- semantic similarity or overlap scoring,
- Navigator grouping,
- History Manager presentation policy,
- canvas rendering,
- community-specific privacy policy,
- collaborative/community sharing policy.

Those consumers should read from the substrate or from explicit projections on
top of it.

### 7.1 Privacy Boundary Clarification

The current `EntryPrivacy` enum exists in the substrate, but it should be read
carefully.

This is another unresolved substrate decision rather than a settled design.

Three tensions are present in the current shape:

- privacy/sharing decisions are community-scoped in the contribution world, but
  the enum is community-agnostic,
- `Shared` is not verifiable by the substrate because the substrate cannot know
  whether some community still accepts an artifact,
- privacy is a contribution/workflow concern while the rest of `graph-memory`
  is mostly structural substrate.

Operationally, the least-confusing reading today is:

- `LocalOnly`: do not project this entry outward by default.
- `ShareCandidate`: locally eligible for projection, but not evidence that it
  has been published or accepted anywhere.
- `Shared`: local bookkeeping hint that the entry has been shared before, not a
  claim that any community currently accepts it.

That last point matters. `graph-memory` cannot know whether a Verse community
currently accepts an artifact, and privacy/publication policy is community
scoped while the enum is not.

So this enum is weaker than VGCP's structural privacy boundary. It is best read
as a local authoring/projection hint, not as canonical community truth. Longer
term, it may want to move outward into a dedicated projection/policy layer.

For the contribution-side privacy boundary, see VGCP §8 and §15. The memory
substrate should not try to duplicate those community-facing rules.

The likely design split is:

- **user intent** lives in a separate local policy/preferences layer keyed by
  entry,
- **community membership/acceptance** lives in contribution/community state,
- **per-contribution filtering** happens during contribution assembly,
- `graph-memory` remains focused on structural temporal truth.

If the enum stays in the substrate for compatibility, it should be understood as
user-intent-ish policy only, not as a statement about active community state.
Renaming it later to something closer to `SharePolicy` would better match that
meaning.

---

## 8. What The Next Layer Should Be

The next clean move is not to make `graph-memory` itself more UI-shaped.

There are actually two next layers, and they should stay separate.

### 8.1 Graph-Facing Runtime Projection

One next layer is a **graph-facing runtime projection** over live memory, for
example:

- shared semantic overlap between nodes,
- hotspot-edge summaries,
- attention or revisit clusters,
- graph-level summaries that combine recency with branching structure.

That projection should treat `graph-memory` as the temporal substrate.

### 8.2 Contribution Projection

The other next layer is a **contribution projection** that turns selected live
memory into canonicalized, signed, shareable artifacts for VGCP-style exchange.

That layer has different rules:

- canonical entry identity,
- contribution-scoped filtering,
- projection-strips-owner semantics,
- explicit transition/edge assembly,
- deterministic serialization.

In other words:

- `graph-memory` answers "what visits happened, how are they branched, and who
  owns which forward path?"
- the runtime projection answers "what graph-relevant summaries can we derive
  for local UI/runtime consumers?"
- the contribution projection answers "what bytes-on-wire artifact can we
  derive from selected local temporal structure under canonicalization and
  privacy rules?"

Keeping that separation matters because the runtime projection will likely want
to evolve heuristics quickly, while the contribution projection will want
deterministic, conservative rules. Neither pressure should distort the
substrate.

### 8.3 Likely Near-Term Substrate Moves

Before either downstream layer accumulates too many consumers, the substrate
shape most likely to need explicit revision is:

- whether Graphshell keeps per-node memory islands or moves toward a
  workspace-global memory fabric, with the latter the more natural fit for the
  original shared-owner model,
- whether local `K` remains URL-shaped or becomes canonical-content-shaped,
- what explicit visit-local context `X` should carry and how persisted snapshot
  migration should work once `X` is no longer `()`,
- whether privacy remains a substrate hint or moves outward into projection
  policy.

These are cheap to name now and expensive to rename later because they change:

- what information persisted snapshots actually retain,
- what assumptions runtime consumers make about owner scope,
- whether sharing workflows can evolve independently of the memory substrate.

---

## 9. Current Practical Reading

Right now the simplest accurate mental model is:

1. `history-tree` supplied the branch-preserving owner/entry/binding idea.
2. `graph-memory` ports that idea into Rust with `Visit` replacing `node` and
   with graph-style edge projections derived from visit parentage.
3. Graphshell currently mounts that substrate as per-node navigation memory,
  which is useful but is also a stronger isolation commitment than it first
  appears.
4. The next architectural question is likely whether to move toward a
  workspace-global memory fabric before too many callers hard-code the current
  per-node isolation assumptions; that is the next architectural move worth
  evaluating explicitly, not just a vague possible future.
5. The next downstream layers are not one thing but two: runtime/UI
  projections and contribution/canonicalization projections.
6. The next substrate decisions most worth resolving before more consumers land
  are the owner scope model, the shape of `X`, and whether privacy remains in
  the substrate at all.
7. The substrate should remain small and defended from both kinds of pressure.

## 10. Concurrency And Ownership Model

`graph-memory` does not provide its own synchronization layer.

Current practical reading:

- it is an ordinary Rust data structure built from `SlotMap`, `HashMap`, and
  `HashSet`,
- it expects callers to control mutation ordering,
- if shared mutable access is needed across threads, callers should provide the
  synchronization boundary themselves,
- whether one particular instantiation is `Send` or `Sync` depends on the
  generic parameter types, but the crate does not itself define a concurrency
  policy.

That is the conceptual handoff this note is meant to lock down.
