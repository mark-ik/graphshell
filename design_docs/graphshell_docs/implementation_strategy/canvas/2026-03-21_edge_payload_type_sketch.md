# Edge Payload Type Sketch

**Date**: 2026-03-21
**Status**: Draft / implementation-oriented planning note
**Kind**: Canvas graph-model type sketch

**Related**:

- `2026-03-21_edge_family_and_provenance_expansion_plan.md` - family/sub-kind expansion recommendation and Provenance-family rationale
- `2026-03-14_graph_relation_families.md` - current canonical family-policy vocabulary
- `../subsystem_history/edge_traversal_spec.md` - traversal truth, append rules, and archive constraints
- `graph_node_edge_interaction_spec.md` - edge inspector and operability expectations
- `../../TERMINOLOGY.md` - `Edge`, `EdgePayload`, `Traversal`, `NavigationTrigger`
- `../../../model/graph/mod.rs` - current `EdgeType`, `EdgeKind`, `Traversal`, `EdgePayload` implementation

---

## 1. Purpose

This sketch exists to turn the edge-family expansion plan into a code-shaped
proposal.

It answers:

- what the next `EdgePayload` should roughly look like,
- which current types survive,
- which current types should be split or removed,
- how persistence and reducer APIs should adapt,
- how to keep traversal truth explicit while making structural relations richer.

This document is not claiming exact final field names. It is defining the
**carrier model** the implementation should converge toward.

---

## 2. Current Model Pressure Points

The current graph model in `model/graph/mod.rs` has three useful properties:

- one durable `EdgePayload` per node pair,
- traversal events live on the payload instead of in a disconnected history store,
- structural relation data already has family-specific sidecars (`arrangement`,
  `containment`, `user_grouped`).

But it also has four structural problems:

1. `EdgeType` mixes family, sub-kind, trigger semantics, and provisional UI
   state in one enum.
2. `EdgeKind` is an internal family-ish tag set, but it is not the public
   family vocabulary and it cannot express provenance cleanly.
3. Some payload data is family-scoped (`arrangement`, `containment`) while other
   families are represented only by a tag with no typed metadata
   (`ImportedRelation`, `Hyperlink`, `AgentDerived`).
4. `History` as an `EdgeType` conflates family presence with traversal data
   existence.

The next shape should make those axes explicit.

---

## 3. Design Rules

1. **Family is policy.**
   - Family determines visibility, durability defaults, layout force, and
     inspector framing.
2. **Sub-kind is meaning.**
   - Sub-kind carries the user-legible relation label inside a family.
3. **Traversal remains event-first.**
   - Traversal records are temporal events, not just another structural label.
4. **Payload is additive.**
   - One edge may carry multiple assertions across families when they refer to
     the same node pair.
5. **Typed family data beats generic JSON blobs.**
   - If a family needs metadata, give it a typed sidecar.

---

## 4. Proposed Core Types

### 4.1 Family and sub-kind split

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EdgeFamily {
    Semantic,
    Traversal,
    Containment,
    Arrangement,
    Imported,
    Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SemanticSubKind {
    Hyperlink,
    UserGrouped,
    AgentDerived,
    Cites,
    Quotes,
    Summarizes,
    Elaborates,
    ExampleOf,
    Supports,
    Contradicts,
    Questions,
    SameEntityAs,
    DuplicateOf,
    CanonicalMirrorOf,
    DependsOn,
    Blocks,
    NextStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TraversalSubKind {
    LinkClick,
    Back,
    Forward,
    AddressEntry,
    PanePromotion,
    Programmatic,
    Redirect,
    ReopenSession,
    JumpAnchor,
    InPageSearchJump,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContainmentSubKind {
    Domain,
    UrlPath,
    FileSystem,
    UserFolder,
    ClipSource,
    NotebookSection,
    CollectionMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArrangementSubKind {
    FrameMember,
    TileGroup,
    SplitPair,
    TabNeighbor,
    ActiveTab,
    PinnedInFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImportedSubKind {
    BookmarkFolder,
    HistoryImport,
    RssMembership,
    FileSystemImport,
    ArchiveMembership,
    SharedCollection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProvenanceSubKind {
    ClippedFrom,
    ExcerptedFrom,
    SummarizedFrom,
    TranslatedFrom,
    RewrittenFrom,
    GeneratedFrom,
    ExtractedFrom,
    ImportedFromSource,
}
```

The important shift is not the exact member list. The important shift is that
each family owns its own sub-kind enum rather than being flattened into a single
catch-all `EdgeType`.

### 4.2 Public assertion enum

The public call surface should move from `EdgeType` to an assertion-oriented
enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeAssertion {
    Semantic {
        sub_kind: SemanticSubKind,
        data: Option<SemanticAssertionData>,
    },
    Containment {
        sub_kind: ContainmentSubKind,
        data: Option<ContainmentAssertionData>,
    },
    Arrangement {
        sub_kind: ArrangementSubKind,
        data: Option<ArrangementAssertionData>,
    },
    Imported {
        sub_kind: ImportedSubKind,
        data: Option<ImportedAssertionData>,
    },
    Provenance {
        sub_kind: ProvenanceSubKind,
        data: Option<ProvenanceAssertionData>,
    },
}
```

Why omit Traversal here?

- traversal should be appended through a dedicated event API, not asserted as a
  static relation kind;
- that avoids recreating the current `History` ambiguity under a new name.

### 4.3 Traversal record and trigger split

`NavigationTrigger` should be renamed into the Traversal-family vocabulary and
grown into a clearer event type:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraversalEvent {
    pub timestamp_ms: u64,
    pub trigger: TraversalSubKind,
    pub direction: TraversalDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    Forward,
    Backward,
}
```

This aligns the code shape with the traversal spec, which already treats
direction as first-class even though the current implementation infers it from
the trigger.

---

## 5. Proposed EdgePayload Shape

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgePayload {
    pub families: BTreeSet<EdgeFamily>,
    pub semantic: Option<SemanticEdgeData>,
    pub traversal: Option<TraversalEdgeData>,
    pub containment: Option<ContainmentEdgeData>,
    pub arrangement: Option<ArrangementEdgeData>,
    pub imported: Option<ImportedEdgeData>,
    pub provenance: Option<ProvenanceEdgeData>,
}
```

Family presence remains additive. Sidecars become consistent: every family that
exists on an edge either has typed data or a deliberate empty marker type.

### 5.1 Family data sidecars

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SemanticEdgeData {
    pub sub_kinds: BTreeSet<SemanticSubKind>,
    pub label: Option<String>,
    pub agent: Option<AgentDerivedMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraversalEdgeData {
    pub traversals: Vec<TraversalEvent>,
    pub metrics: TraversalMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainmentEdgeData {
    pub sub_kinds: BTreeSet<ContainmentSubKind>,
    pub source_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArrangementEdgeData {
    pub sub_kinds: BTreeSet<ArrangementSubKind>,
    pub durable_sub_kinds: BTreeSet<ArrangementSubKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportedEdgeData {
    pub sub_kinds: BTreeSet<ImportedSubKind>,
    pub source_records: Vec<ImportSourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvenanceEdgeData {
    pub sub_kinds: BTreeSet<ProvenanceSubKind>,
    pub derivations: Vec<ProvenanceRecord>,
}
```

Rules:

- `semantic.label` is only meaningful for user-curated/grouping-style semantic
  assertions and should not be abused as a universal edge title.
- `agent` metadata belongs inside Semantic because agent-derived suggestions are
  a semantic assertion with special lifecycle rules, not a new family.
- `provenance.derivations` is where capture-time details like source extractor,
  summarizer identity, or transform revision should live.

### 5.2 Metadata records

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDerivedMetadata {
    pub agent_id: String,
    pub confidence_milli: Option<u16>,
    pub asserted_at_ms: Option<u64>,
    pub decay_progress_milli: Option<u16>,
    pub reasoning_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSourceRef {
    pub source_id: String,
    pub source_label: Option<String>,
    pub import_batch_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRecord {
    pub source_node_id: Option<String>,
    pub operation_label: Option<String>,
    pub actor_id: Option<String>,
    pub tool_id: Option<String>,
    pub recorded_at_ms: Option<u64>,
}
```

The sketch keeps these records intentionally conservative: enough to support
inspection and audit without prematurely designing the full capture pipeline.

---

## 6. API Direction

### 6.1 Replace `add_edge_type(...)`

The current API:

```rust
payload.add_edge_kind(edge_type, label)
payload.add_edge_type(edge_type)
payload.has_edge_type(edge_type)
payload.remove_edge_type(edge_type)
```

should become two explicit channels:

```rust
payload.assert_relation(assertion)
payload.retract_relation(assertion_selector)
payload.has_relation(relation_selector)
payload.push_traversal(event)
```

This is the key separation the current model lacks: structural assertions and
temporal traversal events should not share the same write API.

### 6.2 Selector types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationSelector {
    Semantic(SemanticSubKind),
    Containment(ContainmentSubKind),
    Arrangement(ArrangementSubKind),
    Imported(ImportedSubKind),
    Provenance(ProvenanceSubKind),
    Family(EdgeFamily),
}
```

This keeps querying ergonomic without rebuilding the old `EdgeType` confusion.

---

## 7. Mapping From Current Model

| Current shape | Proposed shape | Notes |
| --- | --- | --- |
| `EdgeType::Hyperlink` | `EdgeAssertion::Semantic { sub_kind: Hyperlink, ... }` | semantic assertion remains durable |
| `EdgeType::History` | removed | replaced by `push_traversal(TraversalEvent)` and Traversal family presence |
| `EdgeType::UserGrouped` | `EdgeAssertion::Semantic { sub_kind: UserGrouped, ... }` | label stays in semantic data |
| `EdgeType::ArrangementRelation(sub_kind)` | `EdgeAssertion::Arrangement { sub_kind, ... }` | arrangement sidecar remains |
| `EdgeType::ContainmentRelation(sub_kind)` | `EdgeAssertion::Containment { sub_kind, ... }` | containment grows broader vocabulary |
| `EdgeType::ImportedRelation` | `EdgeAssertion::Imported { sub_kind, ... }` | imported becomes explicit subtype family |
| `EdgeType::AgentDerived { decay_progress }` | `EdgeAssertion::Semantic { sub_kind: AgentDerived, data: ... }` | move decay/confidence metadata into typed semantic sidecar |
| none | `EdgeAssertion::Provenance { sub_kind, ... }` | new family |

---

## 8. Persistence Direction

The persisted edge shape should stop serializing a legacy-style `PersistedEdgeType`
as the main semantic discriminator.

Target shape:

```rust
pub struct PersistedEdge {
    pub families: Vec<PersistedEdgeFamily>,
    pub semantic: Option<PersistedSemanticEdgeData>,
    pub traversal: Option<PersistedTraversalEdgeData>,
    pub containment: Option<PersistedContainmentEdgeData>,
    pub arrangement: Option<PersistedArrangementEdgeData>,
    pub imported: Option<PersistedImportedEdgeData>,
    pub provenance: Option<PersistedProvenanceEdgeData>,
}
```

Migration rule:

- because this is a prototype, prefer a direct persistence-shape replacement over
  maintaining long-lived dual read/write compatibility;
- if one bridging pass is needed, keep it local to persistence replay, not in
  the steady-state graph model API.

---

## 9. Invariants

1. `EdgeFamily::Traversal` is present if and only if `traversal` data exists and
   at least one real traversal event has been recorded.
2. Family presence must agree with sidecar existence.
3. Display-derived state must not be stored in `EdgePayload`.
4. `Provenance` defaults to hidden-on-canvas and durable-in-storage, but those
   are policy decisions derived from family, not flags copied onto every edge.
5. Agent-derived decay metadata must not determine family identity.

---

## 10. Migration Sequence

### Step 1

- Introduce `EdgeFamily` and the family-specific sub-kind enums.

### Step 2

- Replace `EdgeKind` with `EdgeFamily` inside `EdgePayload`.

### Step 3

- Replace `EdgeType` call sites with `RelationSelector` and `EdgeAssertion`.

### Step 4

- Rename `Traversal` to `TraversalEvent` and make direction explicit.

### Step 5

- Introduce `ImportedEdgeData` and `ProvenanceEdgeData`.

### Step 6

- Update persistence types and replay paths to hydrate the new shape directly.

---

## 11. Recommendation

The model should stop treating edge semantics as a mostly-flat enum with a few
optional sidecars bolted on.

The next implementation pass should converge on:

- explicit family vocabulary,
- family-owned sub-kind enums,
- traversal as a first-class event type,
- typed sidecars for every active family,
- direct persistence of the new carrier model.

That is the cleanest way to make the new Provenance family real without making
the rest of the edge system harder to reason about.