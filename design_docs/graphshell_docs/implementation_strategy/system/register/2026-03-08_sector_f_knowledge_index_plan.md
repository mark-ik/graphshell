<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector F — Knowledge, Index & Diagnostics Registry Development Plan

**Doc role:** Implementation plan for the knowledge, index, and diagnostics registry sector
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `DiagnosticsRegistry`, `KnowledgeRegistry`, `IndexRegistry`
**Specs:** [diagnostics_registry_spec.md](diagnostics_registry_spec.md), [knowledge_registry_spec.md](knowledge_registry_spec.md), [index_registry_spec.md](index_registry_spec.md)

---

## Purpose

`DiagnosticsRegistry`, `KnowledgeRegistry`, and `IndexRegistry` form the foundation of the
system's observability and semantic intelligence layers.

Development order within this sector is **Diagnostics first**, because it is a prerequisite for
confident cross-sector test harness work (PLANNING_REGISTER §1 subsystem implementation order
places Diagnostics at #1). Knowledge second, because the `KnowledgeRegistry` currently exists
as a reconcile shim and must be promoted to a real query surface. Index third, as it depends
on `KnowledgeRegistry` for semantic augmentation.

For servoshell debt-clear sequencing, only the narrow diagnostics slices that make
the renderer boundary observable should be treated as in-flight companions to
debt-clear changes. Full Sector F closure is not a blocker for starting or
continuing debt-clear.

```
DiagnosticsRegistry   ← channels, schemas, retention — test confidence floor
KnowledgeRegistry     ← semantic taxonomy, tag validation, UDC seed floor
IndexRegistry         ← search/lookup providers, timeline, multi-source fanout
```

---

## Current State

| Registry | Struct | Completeness | Key gaps |
|---|---|---|---|
| `DiagnosticsRegistry` | ✅ (atomic) | Partial | 100+ `DIAG_*` constants but no versioned schema; `ChannelSeverity` missing on most; no config roundtrip |
| `KnowledgeRegistry` | ⚠️ shim | Reconcile-only | No query API; no tag validation; no semantic distance; no signal emission |
| `IndexRegistry` | ❌ | Not started | No struct, no search provider abstraction |

---

## Phase F1 — DiagnosticsRegistry: Versioned schemas and channel registration

**Priority: highest in this sector.** Diagnostics is a prerequisite for all cross-sector
testing.

### F1.1 — Add `ChannelSeverity` to all `DIAG_*` constants

Per CLAUDE.md: "All new `DiagnosticChannelDescriptor` literals need a `severity` field."
Per PLANNING_REGISTER §3 quick-win #9: "Add ChannelSeverity to diagnostics."

The ~100 `DIAG_*` channel ID constants in `registries/mod.rs` need to be paired with
`ChannelSeverity` assignments. Audit each channel group and assign:
- `ChannelSeverity::Error` — failure channels (resolution failures, action errors, load failures).
- `ChannelSeverity::Warn` — fallbacks, missing resources, conflicts.
- `ChannelSeverity::Info` — normal operation events.

```rust
pub const DIAG_PROTOCOL_RESOLVE: DiagnosticChannelId = DiagnosticChannelId("protocol.resolve");
pub const DIAG_PROTOCOL_RESOLVE_SEVERITY: ChannelSeverity = ChannelSeverity::Warn;
// ... or bundle into a descriptor:
pub const DIAG_PROTOCOL_RESOLVE: DiagnosticChannelDescriptor = DiagnosticChannelDescriptor {
    id: "protocol.resolve",
    severity: ChannelSeverity::Warn,
    schema_version: 1,
};
```

**Done gates:**
- [ ] All `DIAG_*` constants in `registries/mod.rs` have explicit `ChannelSeverity`.
- [ ] Audit table in doc comments: Error/Warn/Info designation with rationale for each group.
- [ ] No new `DiagnosticChannelDescriptor` literal is accepted without a severity field
  (enforced by the struct — `severity` field added as non-optional).

### F1.2 — Versioned payload schema contract

The `diagnostics_registry_spec.md`'s `schema-authority` policy: diagnostic channels are
first-class contracts with versioned payloads. Schema compatibility must be managed deliberately.

```rust
pub struct DiagnosticChannelDescriptor {
    pub id: DiagnosticChannelId,
    pub severity: ChannelSeverity,
    pub schema_version: u32,
    pub payload_schema: DiagnosticPayloadSchema,  // structured, not free-text
    pub retention: RetentionPolicy,
    pub sampling: SamplingPolicy,
}

pub enum DiagnosticPayloadSchema {
    Structured(Vec<PayloadField>),
    FreeText,  // only for legacy/unversioned channels; new channels must use Structured
}
```

All channels emitted in Sector A–G implementation plans must use `Structured` payloads.

**Done gates:**
- [ ] `DiagnosticChannelDescriptor` struct updated with `schema_version`, `payload_schema`,
  `retention`, `sampling`.
- [ ] At least 5 high-value channels (protocol, viewer, action, identity, renderer) converted to
  `Structured` payloads with field definitions.
- [ ] `DiagnosticsRegistry::register_channel()` validates schema on registration.

### F1.3 — Config roundtrip

The `config-roundtrip` policy: diagnostic channel config (sampling rate, retention, enabled/disabled)
must be serialisable and restorable.

```rust
pub fn get_config(&self, channel_id: &DiagnosticChannelId) -> Option<DiagnosticConfig>
pub fn set_config(&mut self, channel_id: &DiagnosticChannelId, config: DiagnosticConfig)
    -> Result<(), DiagnosticsError>
```

Config is persisted via `GraphIntent::SetDiagnosticConfig` through the WAL.

**Done gates:**
- [ ] `get_config()` and `set_config()` implemented.
- [ ] `DiagnosticConfig` includes enabled flag, sampling rate, retention window.
- [ ] Config roundtrip test: set config → save → restore → config matches.

### F1.4 — `non-silent-orphan` policy enforcement

The `diagnostics_registry_spec.md`'s `non-silent-orphan` policy: orphaned channel emissions
(emitting to a channel that was never registered) must not be silent.

```rust
impl DiagnosticsRegistry {
    pub fn emit(&self, channel_id: &DiagnosticChannelId, payload: DiagnosticPayload) {
        if !self.channels.contains_key(channel_id) {
            log::warn!("diagnostics: emit to unregistered channel {:?}", channel_id);
        }
        // ...
    }
}
```

**Done gates:**
- [ ] `emit()` warns on unregistered channel.
- [ ] All `DIAG_*` channel IDs have corresponding `register_channel()` calls in `RegistryRuntime::new()`.
- [ ] Test: orphan emit produces log warning, does not panic.

---

## Phase F2 — KnowledgeRegistry: Query API and semantic validation

**Unlocks:** `LENS_ID_SEMANTIC_OVERLAY` (Sector A); knowledge-capture lane (#98); UDC seed floor.

### F2.1 — Promote `KnowledgeRegistry` from shim to real registry

The current `registries/knowledge.rs` is a `reconcile_semantics()` function that updates the
app's semantic index when dirty. The `KnowledgeRegistry` struct exists in the atomic layer
but is not surfaced through a query API.

Expose the full atomic query surface through `RegistryRuntime`:

```rust
pub fn query_by_tag(&self, tag: &str) -> Vec<NodeKey>
pub fn tags_for_node(&self, key: &NodeKey) -> Vec<String>
pub fn validate_tag(&self, tag: &str) -> TagValidationResult
pub fn get_label(&self, code: &str) -> Option<String>
pub fn get_color_hint(&self, code: &str) -> Option<Color>
pub fn semantic_distance(&self, a: &str, b: &str) -> f32
```

These correspond directly to the canonical interfaces in `knowledge_registry_spec.md`.

**Done gates:**
- [ ] `query_by_tag()`, `tags_for_node()`, `validate_tag()` exposed on `RegistryRuntime`.
- [ ] `get_label()` and `get_color_hint()` exposed (UDC colour scheme per spec).
- [ ] `semantic_distance()` implemented (Jaccard similarity on UDC class paths as initial implementation).
- [ ] Unit tests for each query method.

### F2.2 — UDC seed floor

The `knowledge_registry_spec.md`'s `seed-floor` policy: UDC (Universal Decimal Classification)
defaults form the offline seed floor. The app must be semantically functional with no mods loaded.

Load a bundled UDC class set (top-level classes + 2 levels of depth) as the default seed:

```
0 — Science and knowledge
1 — Philosophy and psychology
2 — Religion
3 — Social sciences
4 — (vacant / language)
5 — Mathematics and natural sciences
6 — Applied sciences
7 — Arts and recreation
8 — Language and literature
9 — History and geography
```

These are embedded as `include_bytes!` from a compact CBOR or JSON file.

**Done gates:**
- [ ] UDC seed data file at `assets/knowledge/udc_seed.cbor` (or equivalent).
- [ ] `KnowledgeRegistry::new()` loads UDC seed floor.
- [ ] `get_label("5")` returns "Mathematics and natural sciences".
- [ ] `get_color_hint("7")` returns a distinct colour for arts/recreation tags.
- [ ] PLANNING_REGISTER §1C knowledge-capture lane (#98) UDC gate met.

### F2.3 — Tag validation

The `semantic-validation` policy: tags entering the system via user input or import must be
validated against the ontology before acceptance.

```rust
pub enum TagValidationResult {
    Valid { canonical_code: String, display_label: String },
    Unknown { suggestions: Vec<String> },
    Malformed { reason: String },
}
```

`validate_tag()` is called from:
- Node tag edit path (user adds tags in the UI).
- Import path (PLANNING_REGISTER §1C knowledge-capture lane: DOI/clipping import).
- Lens resolution (Sector A) when `LensDescriptor::requires_knowledge` is true.

**Done gates:**
- [ ] `validate_tag()` implemented against UDC seed floor.
- [ ] Malformed tags emit `DIAG_KNOWLEDGE` at `Warn` severity.
- [ ] Unknown tags emit a suggestion list; at least 3 nearest UDC candidates returned.

### F2.4 — Signal emission on semantic index update

When the semantic index is updated via `reconcile_semantics()`, emit a
`SignalKind::Lifecycle(SemanticIndexUpdated)` via `SignalRoutingLayer`. This allows `LensRegistry`
and overlay rendering to react without polling.

**Done gates:**
- [ ] `reconcile_semantics()` emits `SignalKind::Lifecycle(SemanticIndexUpdated)`.
- [ ] `LensRegistry` observer subscribes to this signal to trigger lens re-evaluation (Sector A A4.3).

### F2.5 — Semantic suggestions for node spawning

PLANNING_REGISTER §3 quick-win #4: "Spawn new nodes near semantic parent."

`KnowledgeRegistry::suggest_placement_anchor(key: NodeKey) -> Option<NodeKey>` returns the
node most semantically related to the given node by tag overlap. This is used as the placement
hint when a new node is created from search.

**Done gates:**
- [ ] `suggest_placement_anchor()` implemented using `semantic_distance()`.
- [ ] Used in node-creation path to bias initial position toward semantic kin.

---

## Phase F3 — IndexRegistry: Search and lookup providers

**Unlocks:** Unified omnibar (PLANNING_REGISTER §2 #7); multi-source search fanout; timeline.

### F3.1 — `SearchProvider` trait and `IndexRegistry`

The `index_registry_spec.md`'s `multi-provider fanout`: multiple search providers are
registered; queries fan out to all and results are merged with source metadata.

```rust
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> SearchProviderId;
    fn display_name(&self) -> &str;
    fn search(&self, query: &str, limit: usize) -> Vec<SearchResult>;
}

pub struct SearchResult {
    pub title: String,
    pub url: Option<Url>,
    pub snippet: Option<String>,
    pub source: SearchProviderId,
    pub relevance: f32,
    pub semantic_tags: Vec<String>,  // from KnowledgeRegistry
}

pub struct IndexRegistry {
    providers: HashMap<SearchProviderId, Box<dyn SearchProvider>>,
    local_floor: LocalSearchProvider,  // always available, offline
}
```

Built-in providers:
- `index:local` — search open nodes in the current graph (title, URL, tags).
- `index:history` — search traversal history.
- `index:knowledge` — search UDC taxonomy labels.

**Done gates:**
- [ ] `SearchProvider` trait defined.
- [ ] `IndexRegistry` struct in `shell/desktop/runtime/registries/index.rs`.
- [ ] `LOCAL`, `HISTORY`, `KNOWLEDGE` built-in providers registered.
- [ ] `search(query, limit) -> Vec<SearchResult>` fans out to all providers, merges results.
- [ ] Added to `RegistryRuntime`.
- [ ] `DIAG_INDEX_SEARCH` channel (Info severity).

### F3.2 — Wire into omnibar

The omnibar currently triggers `ACTION_OMNIBOX_NODE_SEARCH` in `ActionRegistry` which calls
`graph_app` search directly. Replace with `IndexRegistry::search()` fanout.

**Done gates:**
- [ ] `execute_omnibox_node_search_action()` calls `IndexRegistry::search()`.
- [ ] Results from `index:local` + `index:history` + `index:knowledge` merged and displayed.
- [ ] PLANNING_REGISTER §2 #7 (Unified Omnibar) bootstrap gate met.

### F3.3 — `local-floor` policy

The `local-floor` policy: local search must function offline with zero configured providers.
`LocalSearchProvider` is always registered and cannot be removed.

**Done gates:**
- [ ] Removing all providers from `IndexRegistry` still leaves `LocalSearchProvider` active.
- [ ] `LocalSearchProvider::search()` searches open graph nodes by title and URL fragment.
- [ ] Test: search with no registered external providers returns local results.

### F3.4 — Timeline provider (prospective)

PLANNING_REGISTER §2 #2: "Temporal Navigation / Time-Travel Preview (Stage F)." The timeline
search provider returns nodes as they existed at a past point in time using the WAL.

This is a stub registration; implementation depends on Sector G history subsystem.

**Done gate (deferred):**
- [ ] `index:timeline` provider stub registered; `search()` returns empty until history subsystem lands.

---

## Acceptance Criteria (Sector F complete)

- [ ] All `DIAG_*` constants have `ChannelSeverity`; all are registered; orphan emits warn.
- [ ] `DiagnosticChannelDescriptor` has versioned schema; 5 high-value channels use `Structured`.
- [ ] Diagnostic config roundtrip works.
- [ ] `KnowledgeRegistry` exposes full query API: tag query, validation, distance, UDC labels.
- [ ] UDC seed floor is bundled and loaded at startup.
- [ ] `reconcile_semantics()` emits semantic-index-updated signal.
- [ ] `IndexRegistry` fans out to local + history + knowledge providers.
- [ ] Omnibar uses `IndexRegistry::search()` instead of direct graph search.
- [ ] All three registries are in `RegistryRuntime` with diagnostics coverage.

---

## Related Documents

- [diagnostics_registry_spec.md](diagnostics_registry_spec.md)
- [knowledge_registry_spec.md](knowledge_registry_spec.md)
- [index_registry_spec.md](index_registry_spec.md)
- [2026-03-08_sector_a_content_pipeline_plan.md](2026-03-08_sector_a_content_pipeline_plan.md) — LensRegistry semantic wiring
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
