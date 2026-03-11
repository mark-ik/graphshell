# Graph Enrichment Plan (2026-03-11)

**Status**: Active umbrella plan
**Lane**: `lane:knowledge-capture` (`#98`)
**Goal**: unify node tagging, badges, UDC classification, import/clip enrichment, and visible graph effects into one authoritative capture-to-surface pipeline.

**Relates to**:

* `2026-02-23_udc_semantic_tagging_plan.md`
* `2026-02-20_node_badge_and_tagging_plan.md`
* `2026-02-24_layout_behaviors_plan.md`
* `2026-02-22_multi_graph_pane_plan.md`
* `../system/2026-03-06_reducer_only_mutation_enforcement_plan.md`
* `../../TERMINOLOGY.md`

## Purpose

Graphshell already has partial plans for:

* user tags and system tags,
* badge display,
* UDC semantic classification,
* clip/import enrichment,
* semantic placement and clustering,
* future tag/classification suggestions.

Those plans are related enough that they should be executed as one semantic lane, not as isolated UI or physics slices.

The canonical pipeline for this lane is:

### Canonical Pipeline

Capture -> Classify -> Store -> Surface -> Affect graph behavior

The lane is about making the graph progressively more meaningful without collapsing semantic identity, presentation, and runtime/backend state into a single undifferentiated metadata blob.

## Scope

This umbrella plan covers five tightly related concerns:

1. **Capture**
   * user-applied tags
   * imported metadata from history/bookmarks/files
   * clip-derived metadata
   * future agent/model suggestions

2. **Classification**
   * UDC classes and labels
   * content kind classification
   * confidence/provenance for derived metadata

3. **Storage and mutation authority**
   * reducer-owned intents
   * persistent node metadata
   * sync/merge semantics

4. **Surface and explanation**
   * graph badges
   * tab/pane badges
   * filter/search facets
   * inspector and explanation UI

5. **Behavioral consumption**
   * semantic placement at spawn time
   * semantic gravity / clustering
   * layout/lens/presentation hooks

## Non-Goals

This plan does not:

* redefine pane identity or backend selection semantics,
* make viewer backend or render mode part of graph-semantic classification,
* permit silent direct writes from UI widgets or background agents to graph metadata,
* replace dedicated plans for Verse publication, community governance, or model execution.

## Canonical Separation of Concerns

This lane depends on keeping four concerns separate even when they are displayed together:

1. **Semantic identity**
   * pane kind
   * content kind
   * user/system tags
   * UDC classification

2. **Presentation**
   * badges
   * color hints
   * label expansion
   * filter chips and inspector displays

3. **Behavioral policy**
   * semantic gravity
   * placement anchors
   * grouping rules
   * lens/layout consumption

4. **Runtime/provider metadata**
   * viewer backend
   * render mode
   * diagnostics-only traits

Graph enrichment may surface all of these, but it must not conflate them.

## Core Data Model

Each enriched node should be able to carry the following conceptual metadata:

* `tags_user`: explicit user-authored tags
* `tags_system`: reserved tags with system behavior
* `classifications`: one or more semantic classifications
* `classification_primary`: optional primary class for compact presentation
* `content_kind`: user-facing semantic content class
* `provenance`: where a tag or classification came from
* `confidence`: confidence score for inferred metadata
* `status`: accepted, suggested, rejected, verified, imported

### Recommended classification record

```rust
struct NodeClassification {
    scheme: ClassificationScheme,   // Udc, ContentKind, future custom scheme
    value: String,                  // e.g. "udc:519.6"
    label: Option<String>,          // e.g. "Computational mathematics"
    confidence: f32,
    provenance: ClassificationProvenance,
    status: ClassificationStatus,
    primary: bool,
}
```

### Provenance requirements

At minimum, provenance should distinguish:

* `UserAuthored`
* `Imported`
* `InheritedFromSource`
* `RegistryDerived`
* `AgentSuggested`
* `CommunitySynced`

This distinction is required so the UI can explain why a node is tagged the way it is and so future automation does not silently overwrite user intent.

## Mutation Authority

All graph-enrichment changes must go through reducer-owned intents.

Required intent families:

* add/remove user tags
* add/remove reserved system tags where user-triggered
* assign/unassign classifications
* accept/reject suggested classifications
* set/clear primary classification
* update provenance-bearing imported metadata

Required invariants:

* no widget writes directly to node metadata,
* no registry mutates graph state directly,
* no background agent can commit enrichment silently without explicit acceptance policy,
* enrichment changes are replayable and sync-safe.

## Stages

### Stage A: Schema and intent closure

Purpose: make enrichment durable and authoritative.

Include:

* durable tag model on nodes
* classification record shape
* reducer intents and validation rules
* persistence roundtrip coverage
* sync/merge policy for tags and classifications

Done gate:

* tags/classifications survive restart and replay,
* reducer-only mutation boundary is enforced,
* conflicting updates have a documented merge rule.

### Stage B: Visible graph enrichment

Purpose: make enrichment legible to users.

Include:

* badge priority system on graph nodes and tabs
* distinction between semantic badges and runtime/backend badges
* filter/search facets for tags and classification
* inspector surface showing tags, classifications, provenance, and confidence
* minimal explanation UI for inferred metadata

Done gate:

* a user can see and explain why a node is classified/tagged,
* graph view and tab view do not overload badges beyond defined caps,
* search/filter surfaces can query enrichment metadata.

### Stage C: Capture and ingestion paths

Purpose: make enrichment creation practical.

Include:

* tag assignment UI
* UDC search/lookup UI
* history/bookmark/file import mapping into tags/classifications
* clip inheritance from source node and extracted text
* suggestion queue for future agent/model proposals

Done gate:

* at least one end-to-end import or clip path produces visible enrichment,
* UDC assignment works through label-first search,
* inherited metadata is marked with provenance.

### Stage D: Graph behavior consumption

Purpose: make enrichment affect spatial organization in a controlled way.

Include:

* semantic placement anchor consumption at spawn time
* semantic gravity / clustering
* optional lens hooks consuming enrichment
* grouping and organization actions based on shared classification prefixes

Done gate:

* semantic tags influence at least one graph behavior path,
* that behavior is toggleable/policy-driven,
* diagnostics can explain when enrichment changed layout behavior.

### Stage E: Agent and sync follow-ons

Purpose: scale enrichment without losing trust boundaries.

Include:

* suggestion-only tag/classification agents
* acceptance/rejection workflow
* Verse/community sync policy for enrichment metadata
* moderation/verification semantics for shared classifications

Done gate:

* no silent automation path exists,
* shared metadata carries provenance,
* local and synced enrichment remain distinguishable.

## Badge Policy

Badges should be treated as a surface, not the data model.

Recommended badge layers:

1. **Critical state badges**
   * crash, unread, pinned, archive, privacy-sensitive states

2. **Semantic badges**
   * primary UDC label/code
   * selected user/system tags
   * pane/content kind badges where useful

3. **Runtime/backend badges**
   * viewer backend
   * native overlay/runtime traits
   * diagnostics-only markers

Rules:

* semantic badges outrank backend badges in the graph view,
* backend badges are secondary hints and must not replace semantic identity,
* overflow and hover expansion behavior must be explicit and testable.

## KnowledgeRegistry Role

`KnowledgeRegistry` remains the semantic logic layer for:

* parsing and validating UDC codes,
* label-first lookup and fuzzy matching,
* semantic distance calculations,
* color/presentation hints,
* future provider routing across schemes beyond UDC.

It should not become a second mutation authority.

The registry answers questions such as:

* "is this classification valid?"
* "what label should this code show?"
* "how similar are these two classifications?"
* "what hints should presentation use?"

The reducer remains responsible for committing graph metadata.

## Diagnostics and Validation

This lane needs explicit instrumentation because enrichment is easy to fake in the UI while still being semantically or behaviorally incomplete.

Required diagnostics:

* invalid classification attempts
* unknown UDC labels vs. valid-but-unlabeled codes
* stale badge derivation
* provenance/confidence inconsistencies
* semantic-layout behavior toggles and effect summaries

Required validation paths:

1. import or clip -> tags/classification assigned -> visible badge
2. tag/classification survives persistence roundtrip
3. search/filter can find by enrichment metadata
4. semantic placement or clustering consumes enrichment
5. suggestion acceptance updates provenance and status correctly

## Immediate Execution Order

Recommended order for the next concrete slices:

1. durable tag/classification schema + reducer intents
2. visible badge/filter/inspector surface using persisted metadata
3. one capture path: clip or import -> enrichment -> visible graph effect
4. semantic placement-anchor path consuming enrichment on spawn
5. suggestion workflow scaffolding after the trust boundary is proven

## Relationship to Existing Plans

This document is the umbrella plan.

Subsidiary plan responsibilities:

* `2026-02-20_node_badge_and_tagging_plan.md`: badge visuals and tag assignment UI.
* `2026-02-23_udc_semantic_tagging_plan.md`: UDC parsing, semantic distance, and label-first classification.
* `2026-02-24_layout_behaviors_plan.md`: semantic placement, semantic gravity, and downstream layout behaviors.

When these plans disagree, this umbrella document should be updated to resolve the conflict rather than allowing silent drift.
