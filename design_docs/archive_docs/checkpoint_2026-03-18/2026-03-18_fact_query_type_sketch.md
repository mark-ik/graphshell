<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Fact Store and Query Type Sketch

**Doc role:** Rust-facing type sketch for the first `fact_store` / `query` slice
**Status:** Draft / implementation-oriented planning note
**Kind:** Register implementation sketch
**Related docs:**
- [../../../technical_architecture/2026-03-18_event_log_fact_store_query_architecture.md](../../../technical_architecture/2026-03-18_event_log_fact_store_query_architecture.md) (`event_log` / `fact_store` / `query` split)
- [../../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md](../../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md) (portable core boundary)
- [../../subsystem_history/2026-03-18_mixed_timeline_contract.md](../../subsystem_history/2026-03-18_mixed_timeline_contract.md) (mixed timeline consumer)
- [../../subsystem_history/node_navigation_history_spec.md](../../subsystem_history/node_navigation_history_spec.md) (node navigation history)
- [../../subsystem_history/node_audit_log_spec.md](../../subsystem_history/node_audit_log_spec.md) (node audit history)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (routing and ownership guardrails)

**Interpretation note**:

- this document is a type sketch, not a claim that these exact names must ship unchanged
- typed subsystem seams matter more than exact module layout
- the first implementation slice should optimize for semantic clarity and portability, not maximal genericity

---

## 1. Purpose

This sketch makes the new read architecture implementation-shaped.

It answers:

- what the first projected fact types look like
- where projection from `LogEntry` happens
- what the in-process `fact_store` owns
- what the first `query` APIs look like
- how current persistence helpers migrate without breaking active surfaces

It does **not** try to solve:

- a user-facing query language
- persistent index compaction
- Verse-wide query families beyond the initial placeholder seam

---

## 2. First-Slice Scope

The first slice should cover only the current history event families already in
active use:

- `AddNode`
- `RemoveNode`
- `NavigateNode`
- `AppendNodeAuditEvent`
- `AppendTraversal`

These are enough to migrate:

- `mixed_timeline_entries()`
- `node_navigation_history()`
- `node_audit_history()`

Rule:

- do not block the first slice on a universal fact ontology
- land one narrow, typed, replayable seam first

---

## 3. Core Opaque Types

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FactId(pub Uuid);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueryCursor {
    pub timestamp_ms: u64,
    pub log_position: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventSourceRef {
    pub log_position: u64,
}
```

Rules:

- `FactId` is internal fact identity, not user-facing identity
- `EventSourceRef` preserves reversible provenance back to the WAL
- `QueryCursor` allows future paginated queries without changing ordering semantics

---

## 4. Fact Types

### 4.1 Temporal Envelope

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactEnvelope {
    pub fact_id: FactId,
    pub source: EventSourceRef,
    pub timestamp_ms: u64,
}
```

For the first slice, every projected fact is temporal because every supported
source event already carries `timestamp_ms`.

### 4.2 Fact Kind

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectedFactKind {
    Traversal {
        from_node_id: String,
        to_node_id: String,
        trigger: PersistedNavigationTrigger,
    },
    NodeNavigation {
        node_id: String,
        from_url: String,
        to_url: String,
        trigger: PersistedNavigationTrigger,
    },
    NodeAudit {
        node_id: String,
        event: NodeAuditEventKind,
    },
    GraphStructure {
        node_id: String,
        is_addition: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedFact {
    pub envelope: FactEnvelope,
    pub kind: ProjectedFactKind,
}
```

Design intent:

- `ProjectedFact` is still close to the active history model on purpose
- the first slice should preserve current semantics before it attempts deeper normalization
- later fact families may split this further into smaller graph/provenance/object facts

---

## 5. Projection Boundary

The first projector should be pure and deterministic.

```rust
pub trait FactProjector {
    fn project(
        &self,
        log_position: u64,
        entry: &LogEntry,
    ) -> SmallVec<[ProjectedFact; 2]>;
}
```

Reference implementation sketch:

```rust
pub struct HistoryFactProjector;

impl FactProjector for HistoryFactProjector {
    fn project(
        &self,
        log_position: u64,
        entry: &LogEntry,
    ) -> SmallVec<[ProjectedFact; 2]> {
        match entry {
            LogEntry::AppendTraversal {
                from_node_id,
                to_node_id,
                trigger,
                timestamp_ms,
            } => smallvec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId(Uuid::new_v4()),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::Traversal {
                    from_node_id: from_node_id.clone(),
                    to_node_id: to_node_id.clone(),
                    trigger: *trigger,
                },
            }],
            LogEntry::NavigateNode {
                node_id,
                from_url,
                to_url,
                trigger,
                timestamp_ms,
            } => smallvec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId(Uuid::new_v4()),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::NodeNavigation {
                    node_id: node_id.clone(),
                    from_url: from_url.clone(),
                    to_url: to_url.clone(),
                    trigger: *trigger,
                },
            }],
            LogEntry::AppendNodeAuditEvent {
                node_id,
                event,
                timestamp_ms,
            } => smallvec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId(Uuid::new_v4()),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::NodeAudit {
                    node_id: node_id.clone(),
                    event: event.clone(),
                },
            }],
            LogEntry::AddNode {
                node_id,
                timestamp_ms,
                ..
            } => smallvec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId(Uuid::new_v4()),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::GraphStructure {
                    node_id: node_id.clone(),
                    is_addition: true,
                },
            }],
            LogEntry::RemoveNode {
                node_id,
                timestamp_ms,
            } => smallvec![ProjectedFact {
                envelope: FactEnvelope {
                    fact_id: FactId(Uuid::new_v4()),
                    source: EventSourceRef { log_position },
                    timestamp_ms: *timestamp_ms,
                },
                kind: ProjectedFactKind::GraphStructure {
                    node_id: node_id.clone(),
                    is_addition: false,
                },
            }],
            _ => SmallVec::new(),
        }
    }
}
```

Implementation note:

- if deterministic rebuild identity matters, `fact_id` may later derive from
  `(log_position, per-entry fact ordinal)` rather than `Uuid::new_v4()`
- the first slice should prefer deterministic ids if convenient

---

## 6. Fact Store Shape

The first `fact_store` can be fully in-memory and rebuilt from WAL on startup.

```rust
pub struct FactStore {
    facts: Vec<ProjectedFact>,
    by_node_id: HashMap<String, Vec<usize>>,
    by_kind: HashMap<ProjectedFactDiscriminant, Vec<usize>>,
}
```

Helper enum:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectedFactDiscriminant {
    Traversal,
    NodeNavigation,
    NodeAudit,
    GraphStructure,
}
```

Rules:

- `facts` stores canonical projected read records
- side indexes store positions, not duplicate fact payloads
- indexes are an implementation detail and may be rebuilt at any time

Suggested API:

```rust
impl FactStore {
    pub fn rebuild_from_log(
        projector: &impl FactProjector,
        entries: impl Iterator<Item = (u64, LogEntry)>,
    ) -> Self;

    pub fn append_projected(
        &mut self,
        projector: &impl FactProjector,
        log_position: u64,
        entry: &LogEntry,
    );

    pub fn facts(&self) -> &[ProjectedFact];
}
```

Rule:

- `FactStore::append_projected()` is invoked by host-owned persistence/replay integration
- UI code never appends facts directly

---

## 7. Query Layer

### 7.1 Query Input Types

```rust
#[derive(Debug, Clone, Default)]
pub struct FactQueryFilter {
    pub kinds: Option<Vec<ProjectedFactDiscriminant>>,
    pub node_id: Option<String>,
    pub after_ms: Option<u64>,
    pub before_ms: Option<u64>,
    pub text_contains: Option<String>,
}

pub enum GraphQuery {
    MixedTimeline {
        filter: FactQueryFilter,
        limit: usize,
    },
    NodeNavigationHistory {
        node_id: String,
        limit: usize,
    },
    NodeAuditHistory {
        node_id: String,
        limit: usize,
    },
}
```

### 7.2 Query Output Types

The first query layer may continue to return existing surface-facing row types.

```rust
pub enum GraphQueryResult {
    TimelineEvents(Vec<HistoryTimelineEvent>),
    NodeNavigationEntries(Vec<LogEntry>),
    NodeAuditEntries(Vec<LogEntry>),
}
```

Important migration rule:

- query results may adapt back into active surface types during the first slice
- this is acceptable as long as the semantic source is projected facts rather than direct bespoke WAL traversal

### 7.3 Query Engine Shape

```rust
pub struct GraphQueryEngine {
    facts: FactStore,
}

impl GraphQueryEngine {
    pub fn execute(&self, query: GraphQuery) -> GraphQueryResult;
}
```

For the first slice, `execute()` may:

- filter `facts`
- sort by `(timestamp_ms DESC, log_position DESC)`
- adapt matching facts into existing result structs

This keeps migration risk low while establishing the new semantic seam.

---

## 8. Adapters to Existing APIs

The current persistence APIs should remain callable during migration, but they
should become thin adapters over `query`.

Target direction:

```rust
impl GraphStore {
    pub fn mixed_timeline_entries(
        &self,
        filter: &HistoryTimelineFilter,
        limit: usize,
    ) -> Vec<HistoryTimelineEvent> {
        self.query_engine()
            .execute(GraphQuery::MixedTimeline {
                filter: fact_filter_from_history_filter(filter),
                limit,
            })
            .into_timeline_events()
    }

    pub fn node_navigation_history(
        &self,
        node_id: &str,
        limit: usize,
    ) -> Vec<LogEntry> {
        self.query_engine()
            .execute(GraphQuery::NodeNavigationHistory {
                node_id: node_id.to_string(),
                limit,
            })
            .into_node_navigation_entries()
    }

    pub fn node_audit_history(
        &self,
        node_id: &str,
        limit: usize,
    ) -> Vec<LogEntry> {
        self.query_engine()
            .execute(GraphQuery::NodeAuditHistory {
                node_id: node_id.to_string(),
                limit,
            })
            .into_node_audit_entries()
    }
}
```

Migration rule:

- preserve public behavior first
- move implementation ownership second

---

## 9. Suggested Module Shape

Illustrative module layout:

```text
services/
  persistence/
    types.rs          // LogEntry, HistoryTimelineEvent, existing durable schema
    mod.rs            // GraphStore integration and adapter methods
  facts/
    mod.rs            // FactStore
    projector.rs      // FactProjector, HistoryFactProjector
    types.rs          // ProjectedFact, envelopes, discriminants
  query/
    mod.rs            // GraphQueryEngine
    types.rs          // GraphQuery, GraphQueryResult, FactQueryFilter
    adapters.rs       // history filter/result adapters
```

Alternative if this moves into `graphshell-core` later:

- `graphshell_core::event_log::*`
- `graphshell_core::facts::*`
- `graphshell_core::query::*`

---

## 10. Portability Notes

The first-slice types should stay compatible with the long-term portable-core
goal.

Portable:

- `ProjectedFact`
- `FactEnvelope`
- `FactStore` semantics
- projector logic
- `GraphQuery`
- pure query execution

Host-only for now:

- WAL iteration over fjall storage
- startup rebuild orchestration
- persistence of materialized indexes
- UI-specific row rendering

This keeps the seam usable across:

- desktop surfaces
- embedded tools
- web-facing companions
- headless inspection/test harnesses

---

## 11. First Implementation Sequence

1. Add `services/facts/types.rs` with `ProjectedFact`, envelopes, and discriminants.
2. Add `HistoryFactProjector` covering `AddNode`, `RemoveNode`, `NavigateNode`,
   `AppendNodeAuditEvent`, and `AppendTraversal`.
3. Add in-memory `FactStore::rebuild_from_log(...)`.
4. Add `GraphQueryEngine` with `MixedTimeline`, `NodeNavigationHistory`, and
   `NodeAuditHistory`.
5. Re-route existing `GraphStore` helper methods through query adapters.
6. Keep surface structs unchanged for the first pass.
7. Only after parity is proven, consider introducing surface-specific result
   types that no longer expose raw `LogEntry`.

---

## 12. Done Gates

- [x] `ProjectedFact` and `ProjectedFactKind` exist for the first history event families.
- [x] Projection from `LogEntry` to `ProjectedFact` is deterministic and replayable.
- [x] `FactStore` can rebuild from WAL-derived entries in-memory.
- [x] `GraphQueryEngine` can answer mixed timeline, node navigation history, and
      node audit history queries.
- [x] Existing `GraphStore` helper APIs can delegate to the query layer without
      behavior regressions.
- [x] The first-slice type shapes remain free of egui, Servo, and host-UI types.
