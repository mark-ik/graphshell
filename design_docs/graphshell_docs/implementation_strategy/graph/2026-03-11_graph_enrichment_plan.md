# Graph Enrichment Plan (2026-03-11)

**Status**: Active umbrella plan (revised for prototype UX reset)
**Lane**: `lane:knowledge-capture` (`#98`)
**Goal**: unify node tagging, badges, UDC classification, import/clip enrichment, and visible graph effects into one authoritative capture-to-surface pipeline.

**Relates to**:

* `semantic_tagging_and_knowledge_spec.md`
* `node_badge_and_tagging_spec.md`
* `2026-02-24_layout_behaviors_plan.md`
* `multi_view_pane_spec.md`
* `faceted_filter_surface_spec.md`
* `../workbench/graph_first_frame_semantics_spec.md`
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

## Prototype Reality Check (2026-03-11)

Current code is ahead in semantic plumbing and behind in user-facing payoff.

What already exists in some form:

* runtime tag transport,
* `KnowledgeRegistry` UDC parsing and label-first search,
* semantic index reconciliation,
* semantic clustering force,
* semantic placement-anchor suggestion,
* tag suggestion scaffolding.

What is still largely missing from the actual product experience:

* a graph inspector that explains a selected node's enrichment,
* a persistent filter/facet surface or omnibar-first filter flow,
* a tested badge budget and overflow policy,
* a user-facing minimap/navigation aid instead of diagnostics-only navigation,
* durable provenance/confidence/status storage for inferred metadata.

That mismatch matters. A prototype does not get better by capturing more hidden semantics that users cannot see, trust, filter, or navigate.

## Prototype UX Reset

For this lane, Graphshell should optimize for prototype legibility before semantic breadth.

Priority rules:

1. **Explain before automate**
   * It does not make sense to expand agent suggestions or sync semantics before the UI can show why a node is tagged/classified and let the user accept, reject, or ignore that state.

2. **Filter before decorate**
   * It does not make sense to add more badge types if the same metadata cannot drive real find/filter/group actions.

3. **Navigate before densify**
   * It does not make sense to increase graph density through enrichment if users still lack a first-class minimap, inspector-driven focus flow, and predictable spawn behavior near the source context.

4. **Use canonical workbench semantics**
   * Enrichment-triggered organization must follow `Frame` / `Frame membership` / `Frame-affinity region` authority, not drift back into treating `MagneticZone` as a separate semantic object.

5. **Do not deepen runtime-only transport**
   * The current runtime tag carrier is acceptable as a transition path. It is not acceptable as the long-term carrier for provenance-bearing import, suggestion, or community-synced enrichment.

The canonical pipeline for this lane is:

### Canonical Pipeline

Capture -> Classify -> Store -> Surface -> Filter/Explain -> Affect graph behavior

The lane is about making the graph progressively more meaningful without collapsing semantic identity, presentation, and runtime/backend state into a single undifferentiated metadata blob.

If a slice reaches `Capture` or `Classify` but does not reach `Filter/Explain`, it should be treated as partial rather than product-meaningful.

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
   * graph inspector / sidecar explanation surface
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
* treat badge count growth as proof of semantic progress while explanation/filter surfaces remain absent,
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

## Current Baseline and Blocking Gaps

This plan should assume the following baseline until code says otherwise:

### Already present in code

* runtime tag transport and reducer-owned tag intents,
* UDC parsing and fuzzy lookup through `KnowledgeRegistry`,
* semantic clustering force and semantic placement-anchor suggestion,
* initial suggestion scaffolding.

### Still missing or insufficient

* durable `NodeClassification`-style records with provenance/confidence/status,
* a first-class graph inspector for selected-node enrichment,
* filter/facet surfaces that query enrichment metadata,
* badge overflow/priority behavior beyond isolated one-off badges,
* a user-facing minimap/navigation aid outside diagnostics surfaces.

Any stage claiming product-level enrichment should be read against this baseline.

## Stages

### Stage 0: Prototype legibility reset

Purpose: make enrichment visible, explorable, and navigable before scaling capture breadth.

Include:

* graph inspector or sidecar showing selected-node tags/classifications and "why" explanation,
* omnibar and command-surface routing into the canonical faceted-filter contract,
* a persistent or summonable filter surface for enrichment metadata,
* badge budget/overflow policy before adding more semantic badge density,
* user-facing minimap/navigation surface promotion or explicit rejection of minimap as a product direction,
* node spawn behavior that prefers source-context or semantic placement anchors over center-spawn where possible,
* graph-search provenance/history/pinning controls so semantic slices are inspectable and reversible,
* bounded neighborhood expansion controls for anchor-driven slices.

Rules:

* Stage 0 may use current transport only for read-only explanation/filter scaffolding.
* Stage 0 must not justify new provenance-bearing capture paths staying on runtime-only transport.

Done gate:

* a user can answer "why is this node tagged/classified and why is it here?",
* a user can isolate nodes by enrichment metadata without opening diagnostics-only tooling,
* dense graph navigation no longer depends on a diagnostics pane,
* badge growth is capped by explicit priority and overflow behavior,
* semantic slices can be revisited, pinned, and distinguished by provenance without relying on transient memory.

### Stage 0 implementation snapshot (prototype status as of 2026-03-11)

Landed in the prototype:

* selected-node enrichment inspector with semantic tags, suggestions, and placement-anchor explanation,
* runtime-index-backed graph search that matches semantic tags and UDC-style queries,
* graph node semantic badge budget with overflow coverage,
* semantic-aware node spawn placement from the current source/anchor context,
* clickable semantic tag chips that drive graph slices directly,
* active graph-search status pill with provenance, highlight/filter toggles, clear, back history, and recent restore,
* pinnable semantic slices plus a compact pinned-slice canvas badge,
* anchor-driven slice actions including bounded neighborhood expansion (1-hop and 2-hop),
* transient semantic-slice feedback via toasts.

Still incomplete relative to Stage 0 intent:

* there is still no real persistent facet surface beyond the search/slice path,
* navigation support is better but still not a true product-grade minimap or overview surface,
* provenance/confidence/status are still UI-level slice semantics, not durable node-level enrichment records.

### Stage A: Schema and intent closure

Purpose: make enrichment durable and authoritative.

Include:

* durable tag model on nodes
* classification record shape
* reducer intents and validation rules
* persistence roundtrip coverage
* sync/merge policy for tags and classifications

Blocking note:

* import, suggestion, and sync-facing enrichment must not be treated as done while provenance-bearing metadata still depends on runtime-only carriers.

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

Blocking note:

* Stage B is not honestly done if badges exist but the user still cannot inspect or filter the same metadata through a canonical surface.

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

Boundary note:

* clipping is a **concrete Stage C producer**, not the owner of broader document-analysis logic.
* the viewer clipping lane owns page inspection, element selection, explicit clip materialization, and clip-local metadata capture.
* outbound-link extraction, selector-recipe extraction, and other broader document-analysis batches belong in separate analysis/projection follow-ons, not in the clipping viewer plan itself.

Blocking note:

* new capture paths must terminate in the same visible explanation/filter surfaces landed in Stage 0 and Stage B, not in hidden metadata only.

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
* grouping and organization actions based on shared classification prefixes and canonical `Frame` semantics

Terminology rule:

* When this stage touches spatial organization, use `Frame membership` / `Frame-affinity region` authority from `graph_first_frame_semantics_spec.md`. Do not reintroduce `MagneticZone` as a separate semantic authority.

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
6. selected-node inspector explains the active tag/classification state without diagnostics-only tooling
7. badge overflow remains legible when semantic and runtime badges coexist

## Immediate Execution Order

Recommended order for the next concrete slices:

1. Stage 0 prototype legibility reset: inspector + filter route + badge budget + minimap/navigation decision + spawn behavior cleanup
2. durable tag/classification schema + reducer intents
3. visible badge/filter/inspector surface using persisted metadata
4. one capture path: clip or import -> enrichment -> visible graph effect
5. semantic placement-anchor path consuming enrichment on spawn
6. suggestion workflow scaffolding after the trust boundary is proven

## Relationship to Existing Plans

This document is the umbrella plan.

Subsidiary plan responsibilities:

* `node_badge_and_tagging_spec.md`: canonical badge visuals and tag assignment UI contract.
* `semantic_tagging_and_knowledge_spec.md`: UDC parsing, semantic distance, canonicalization, diagnostics, and label-first classification.
* `2026-02-24_layout_behaviors_plan.md`: semantic placement, semantic gravity, and downstream layout behaviors.
* `faceted_filter_surface_spec.md`: canonical filter semantics and authority boundaries for enrichment queries.

When these plans disagree, this umbrella document should be updated to resolve the conflict rather than allowing silent drift.
