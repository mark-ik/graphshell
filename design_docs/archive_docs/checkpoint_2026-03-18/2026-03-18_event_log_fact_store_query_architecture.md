<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Event Log / Fact Store / Query Architecture

**Date**: 2026-03-18
**Status**: Active architecture note
**Scope**: Defines the durable-write, projected-read, and query-surface split
for Graphshell so history, provenance, Verse objects, and agent-facing
knowledge all share one canonical read architecture.

**Related docs**:

- [`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md) - portable core boundary
- [`../implementation_strategy/system/register/2026-03-18_fact_query_type_sketch.md`](../implementation_strategy/system/register/2026-03-18_fact_query_type_sketch.md) - first-slice Rust type sketch
- [`../implementation_strategy/subsystem_history/2026-03-08_unified_history_architecture_plan.md`](../implementation_strategy/subsystem_history/2026-03-08_unified_history_architecture_plan.md) - history taxonomy and ownership
- [`../implementation_strategy/subsystem_history/2026-03-18_mixed_timeline_contract.md`](../implementation_strategy/subsystem_history/2026-03-18_mixed_timeline_contract.md) - first mixed-history query consumer
- [`../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md`](../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md) - Verse object and sync model
- [`../implementation_strategy/system/2026-03-06_reducer_only_mutation_enforcement_plan.md`](../implementation_strategy/system/2026-03-06_reducer_only_mutation_enforcement_plan.md) - durable mutation authority boundary

---

## 1. Purpose

Graphshell already has the beginnings of a strong write model:

- reducer-owned durable mutation authority
- append-only WAL records
- typed history tracks
- explicit runtime vs durable state separation

What it does not yet have is an equally strong **read architecture**.

Today, many read surfaces still grow as feature-local query helpers or direct
WAL scans. That is acceptable for a prototype phase, but it will not scale to
the intended product shape:

- shared storage/query logic across desktop surfaces
- embedded tools and agents
- web-facing companions
- Verse knowledge exchange
- provenance and lineage inspection
- future replay, diagnostics, and semantic search surfaces

This note defines the missing split:

1. **`event_log`** - append-only durable mutation truth
2. **`fact_store`** - normalized projected read facts derived from the event log
3. **`query`** - canonical user/system question surfaces over those facts

The architecture goal is simple:

> Reducer/WAL remains the only durable write authority. All rich read surfaces
> consume projected facts and queries instead of inventing bespoke traversal
> logic over raw storage.

---

## 2. Canonical Subsystems

### 2.1 `event_log`

The `event_log` subsystem is the append-only durable mutation truth.

It owns:

- WAL entry schema
- append order / log position
- replay contract
- snapshot + log recovery
- durable write authority boundary

It does **not** own:

- feature-specific mixed-history rendering
- ad hoc UI projection shapes
- user-facing search/query language
- arbitrary per-feature indexes

### 2.2 `fact_store`

The `fact_store` subsystem is the canonical normalized read model projected
from the `event_log`.

It owns:

- fact schema
- projection rules from event types to facts
- derived identity and temporal envelopes for facts
- optional materialized indexes needed by many consumers

It does **not** own:

- durable writes
- reducer authority
- UI-specific row models
- platform-specific storage backends

### 2.3 `query`

The `query` subsystem is the public question-answering surface over the
`fact_store`.

It owns:

- query types and filters
- result ordering/grouping contracts
- reusable read APIs for UI, agents, diagnostics, and Verse tooling
- query-time composition across history, provenance, graph, and Verse domains

It does **not** own:

- durable persistence authority
- direct UI interaction policy
- runtime worker orchestration

---

## 3. Core Rules

1. **Reducer/WAL is the only durable write path.**
   Facts are never mutated directly by UI, workers, or agents.
2. **Facts are projected, not hand-authored.**
   Every fact must be derivable from canonical durable truth or a clearly named
   imported authority.
3. **Queries consume facts, not raw WAL scans, by default.**
   Raw log scans remain acceptable only as a temporary adapter path while the
   corresponding fact family is being introduced.
4. **UI surfaces are query consumers.**
   History Manager, node panels, replay, diagnostics, and future Verse tools do
   not become semantic owners of feature-local read models.
5. **Projection is additive.**
   New event kinds may project new fact families without invalidating old query
   consumers.
6. **Fact schema is host-owned, portable, and UI-agnostic.**
   It must be suitable for desktop, embedded tools, and web/WASM companions.
7. **Indexes are optimization, not ontology.**
   Materialized indexes may change; fact semantics and query contracts remain
   stable.

---

## 4. Architectural Shape

```text
Reducer Intent
    -> apply_intents()
        -> event_log append (durable truth)
            -> snapshot/WAL replay
                -> fact projector
                    -> fact_store
                        -> query API
                            -> History Manager / node panels / replay / diagnostics / Verse tools / agents
```

Read direction:

- durable events become normalized facts
- facts become query results
- query results become surface-specific presentation

Write direction:

- user/system intent
- reducer-owned mutation
- event append

No surface writes facts directly. No query surface mutates event-log truth.

---

## 5. Fact Families

The fact model should start small and typed. It is not a generic triple store in
v1. It is a Graphshell-owned set of normalized fact families.

Illustrative first families:

```rust
pub enum ProjectedFactKind {
    NodeExists {
        node_id: String,
    },
    NodeAddressAt {
        node_id: String,
        url: String,
        trigger: Option<PersistedNavigationTrigger>,
    },
    NodeAudit {
        node_id: String,
        event: NodeAuditEventKind,
    },
    Traversal {
        from_node_id: String,
        to_node_id: String,
        trigger: PersistedNavigationTrigger,
    },
    GraphStructure {
        node_id: String,
        is_addition: bool,
    },
    VerseObject {
        object_id: String,
        object_kind: VerseObjectKind,
        author_id: String,
        cid: String,
    },
}

pub struct ProjectedFact {
    pub fact_id: Uuid,
    pub source_log_position: u64,
    pub timestamp_ms: Option<u64>,
    pub kind: ProjectedFactKind,
}
```

Notes:

- `ProjectedFact` is a normalized read record, not a UI row.
- `source_log_position` preserves provenance back to durable truth.
- `timestamp_ms` is optional because not every future fact family will be
  temporal in the same way, though history facts generally are.
- Fact families should stay typed; schema-on-read flexibility is achieved by
  composable query contracts, not by discarding type safety.

---

## 6. Projection Rules

Projection is deterministic and replayable.

Example v1 mapping:

| Event-log entry | Fact(s) projected |
| --- | --- |
| `AddNode { node_id, timestamp_ms, .. }` | `NodeExists { node_id }`, `GraphStructure { node_id, is_addition: true }` |
| `RemoveNode { node_id, timestamp_ms }` | `GraphStructure { node_id, is_addition: false }` |
| `NavigateNode { node_id, from_url, to_url, trigger, timestamp_ms }` | `NodeAddressAt { node_id, url: to_url, trigger }` |
| `AppendNodeAuditEvent { node_id, event, timestamp_ms }` | `NodeAudit { node_id, event }` |
| `AppendTraversal { from_node_id, to_node_id, trigger, timestamp_ms }` | `Traversal { from_node_id, to_node_id, trigger }` |

Projection rules:

1. The projector is pure with respect to semantic output.
2. Replaying the same event log yields the same facts.
3. Facts may be one-to-many with respect to an event.
4. Facts must preserve source provenance (`source_log_position`).
5. Projection may be incremental or rebuild-from-log; query semantics must not
   depend on which strategy was used.

---

## 7. Query Surface

Queries are the canonical read entrypoint for all consumers that need semantic
answers instead of raw event inspection.

Illustrative query categories:

- history timeline queries
- node lifecycle and navigation queries
- provenance and lineage queries
- graph structure and recency queries
- Verse object lookup and authorship queries
- agent-facing semantic retrieval over local knowledge

Illustrative query shapes:

```rust
pub enum GraphQuery {
    MixedTimeline(HistoryTimelineFilter),
    NodeHistory { node_id: String, limit: usize },
    NodeAudit { node_id: String, limit: usize },
    ProvenanceChain { object_id: String, depth: usize },
    VerseObjectsByAuthor { author_id: String, limit: usize },
}

pub enum GraphQueryResult {
    TimelineEvents(Vec<HistoryTimelineEvent>),
    NodeHistory(Vec<NodeHistoryEntry>),
    NodeAudit(Vec<NodeAuditRow>),
    ProvenanceChain(ProvenanceChainResult),
    VerseObjects(Vec<VerseObjectSummary>),
}
```

The exact API shape may differ, but the architectural rule stands:

> mixed timeline is one query consumer, not the center of the architecture.

---

## 8. First Adopters

### 8.1 Mixed History Timeline

The current `mixed_timeline_entries()` contract is the first direct adopter.

Near-term rule:

- existing WAL scan implementation may remain as an adapter during migration
- target implementation should read from `fact_store` / `query`

This means `HistoryTimelineEvent` remains a valid surface type, but it should
eventually be produced by query composition over facts rather than by a
feature-local raw log scan.

### 8.2 Node Navigation History

`node_navigation_history()` should move from "scan WAL for `NavigateNode`" to
"query node-address facts ordered by time."

Benefits:

- shared behavior with mixed timeline
- one ordering/provenance policy
- easier future joins with audit/provenance data

### 8.3 Node Audit History

Node audit panel and filtered history views should consume node-audit facts
through query contracts rather than maintaining panel-local interpretation
rules.

### 8.4 Verse Objects

When Verse objects land as durable local records, their user-facing lookup
should enter through the same query subsystem:

- object by id
- objects by author
- objects linked to node/activity
- local provenance chains

This is the point where the architecture becomes broader than "history."

---

## 9. Portable Core Boundary

This split should align with `graphshell-core`.

Portable candidates:

- event-log record schema
- fact schema
- projector logic
- query types and pure query execution over projected facts

Host-owned concerns:

- storage backend implementation
- cache/index persistence strategy
- network transport
- UI row rendering
- async worker orchestration

Implication:

Graphshell can share storage/query logic across desktop surfaces, embedded
tools, web-facing companions, and future headless Verse nodes without making UI
types or host runtime policy part of the semantic core.

---

## 10. Migration Plan

### Stage Q1 - Naming and ownership

- Adopt the subsystem language:
  - `event_log`
  - `fact_store`
  - `query`
- Update active docs to stop treating bespoke feature queries as the end-state
  architecture.

### Stage Q2 - Minimal fact projector

- Introduce `ProjectedFact` and `ProjectedFactKind`
- Add deterministic projection from existing history-related WAL entries
- Keep implementation simple: replay + in-memory projection is acceptable

### Stage Q3 - Query adapter layer

- Add query entrypoints that can serve:
  - mixed timeline
  - node navigation history
  - node audit history
- Permit temporary adapters from old helper APIs to new query APIs

### Stage Q4 - Surface adoption

- History Manager consumes query results
- node history panel consumes query results
- node audit panel consumes query results
- replay/preview surfaces consume query results where applicable

### Stage Q5 - Index/materialization pass

- Materialize hot indexes only after real bottlenecks are observed
- Keep index structures replaceable without changing query contracts

### Stage Q6 - Verse and agent expansion

- Add Verse object fact families
- add provenance/lineage query families
- add agent-facing local semantic retrieval surfaces

---

## 11. Non-Goals

This note does not require Graphshell to:

- replace the reducer with a database engine
- replace WAL with direct fact writes
- adopt an untyped generic triple store immediately
- ship a user-facing Datalog language now
- solve all sync/index/storage concerns before the query architecture exists

The prototype-friendly posture is:

- keep write authority strict
- make read semantics composable
- optimize only where pressure is real

---

## 12. Acceptance Criteria

- [x] Graphshell has an explicit architecture note naming `event_log`,
      `fact_store`, and `query` as distinct concerns.
- [x] Reducer/WAL remains the sole durable write authority.
- [x] At least one projected fact family is defined for current history events.
- [x] At least one query API is defined as a consumer of facts rather than raw
      feature-local storage traversal.
- [x] Mixed timeline, node history, and node audit are named as first adopters.
- [x] The architecture is stated as portable across desktop, embedded, and
      web/WASM-facing contexts.
