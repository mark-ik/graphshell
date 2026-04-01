# UDC Semantic Tagging & Layout Plan (2026-02-23)

**Status**: Closed / Archived 2026-04-01 — historical execution record only

**Canonical authority now lives in**:

- `semantic_tagging_and_knowledge_spec.md` — KnowledgeRegistry, canonicalization, semantic index, UDC facet parity, and diagnostics
- `node_badge_and_tagging_spec.md` — tag assignment UI, canonical UDC chip behavior, and badge exposure
- `2026-03-11_graph_enrichment_plan.md` — active knowledge-capture lane and remaining user-facing payoff work
- `layout_behaviors_and_physics_spec.md` and `2026-02-24_layout_behaviors_plan.md` — semantic clustering force and layout follow-on work

## Closure Summary

- `KnowledgeRegistry` parsing, search, canonicalization, and reconcile flow landed in runtime code.
- The plan's old `workspace.semantic_tags` transport model is stale; durable tag truth now lives on `Node.tags`, while `semantic_index` remains a derived runtime cache.
- UDC filter/routing parity, tag-panel inference, canonicalization, and diagnostics moved into the canonical specs above.
- The only meaningful open tail from this plan is no longer owned here: visible semantic payoff, enrichment UX, and any remaining semantic-physics tuning belong to the enrichment and layout/physics docs.

Historical plan text is retained below as implementation record only; unchecked tasks are not active authority.

## Context
Current tags (`#starred`, `research`) are "flat"—the system knows string equality but not relationship. UDC provides a hierarchical, faceted ontology (e.g., `51` Mathematics is a parent of `519.6` Computational Mathematics).

By adopting UDC, we enable **Semantic Physics**: nodes shouldn't just cluster because they link to each other (topology), but because they are *about* the same thing (ontology).

## Architecture

### 1. Data Model: Tags as Transport
To avoid breaking the existing `Node` schema or persistence layer during the current registry migration phase, we keep semantic tags in app-owned runtime state.

*   **Runtime Transport (Current)**: `GraphBrowserApp` stores semantic tags in `semantic_tags: HashMap<NodeKey, HashSet<String>>`. This keeps the reducer path simple while `KnowledgeRegistry` is still stabilizing.
*   **Persistence (Planned)**: Persist semantic tags once the registry phase is complete and schema strategy is finalized.
*   **Runtime (Semantic Index)**: The `KnowledgeRegistry` maintains a high-performance index mapping `NodeKey` $\to$ `CompactCode`.
    *   **CompactCode**: A `u64` or byte-vector representation of the UDC path (e.g., `[5, 1, 9, 6]`) optimized for O(1) distance calculations.
    *   **Sync (Reconciliation)**: `graph_app.rs` does not call the registry directly. Instead, `shell/desktop/ui/gui.rs` runs a reconciliation step after `apply_intents` to update the index if tags changed.

### 2. The Knowledge Registry
A new Atomic Registry (see `2026-02-22_registry_layer_plan.md`) responsible for semantic definitions.

*   **Architecture**: Implements the **Strategy Pattern** via Providers.
    *   **Router**: Routes tags by prefix (`udc:`, `schema:`) to the appropriate provider.
    *   **Decoupling**: The physics engine asks `distance(NodeA, NodeB)`, unaware of the underlying logic (UDC prefix length vs. Vector cosine similarity).
*   **UdcProvider**:
    *   Parses strings into `CompactCode`.
    *   Calculates distance based on shared prefix length.
    *   Provides labels ("Mathematics").

### 3. Architectural Refinement: Data vs. System
To respect the "Access through Intents" rule and avoid coupling the reducer to the registry:
*   **Data**: `GraphBrowserApp` holds `semantic_tags`, `semantic_index` (HashMap), and a `semantic_index_dirty` flag.
*   **System**: `KnowledgeRegistry` (in `AppServices`) holds the logic (parsing, matching).
*   **Flow**:
    1. `TagNode` / `UntagNode` intents update `app.semantic_tags` and set `dirty = true`.
    2. Frame loop calls `knowledge::reconcile_semantics(app, registry)`.
    3. Reconcile checks dirty flag, parses tags using registry, updates `app.semantic_index`, and prunes stale node keys.

### 4. First-Class Categorization
*   **Inference (Label-First)**: Users type natural language ("math"), not codes. The registry uses fuzzy search (`nucleo`) to map "math" $\to$ `udc:51`.
*   **Autocomplete**: Tag Input UI (`T` key) queries registry for suggestions.
    *   Input: "calc" $\to$ Suggestion: "Calculus (udc:517)".
 *   **Visuals**: `GraphNodeShape` queries registry for color hints.
 *   **Unknown Codes**: Users can apply UDC codes deeper than the registry's known subset (e.g. `udc:519.68`).
     *   **Physics**: These cluster correctly with known parents (attracted to `udc:519` and `udc:51`) due to prefix matching.
     *   **Display**: Rendered as raw codes if no label is found.

### 5. Semantic Physics (The "Library Force")
A custom force added to the physics engine that runs alongside Fruchterman-Reingold.

*   **Logic**: For every pair of nodes $(A, B)$:
    *   If both have UDC tags, calculate `similarity = common_prefix_length(A, B)`.
    *   Apply attraction force proportional to `similarity`.
*   **Result**: The graph naturally self-organizes into "shelves" or "sections" (Math nodes cluster here, History nodes cluster there) even if they don't hyperlink to each other.

---

## Implementation Phases

### Phase 1: Registry & Parsing
**Goal**: System can recognize and parse UDC tags.

**Phase 1 progress (2026-02-23):**
- `KnowledgeRegistry` runtime parser/search is wired and seeded with MVP UDC definitions.
- `shell/desktop/ui/gui.rs` reconciliation path runs via `knowledge::reconcile_semantics(...)` after intent application.
- Added focused ontology unit coverage:
    - UDC parse path (`udc:519.6`)
    - label-first search query hit (`math` -> includes `51`)
    - reconcile dirty-flag/index update + stale key pruning behavior.
- Selected-node enrichment now shows semantic tag status/provenance text, suggestion rationale, and semantic placement-anchor context.
- Graph search/filter now honors hierarchical `udc_classes` semantics for UDC-shaped queries (`udc:51` matches descendant classes such as `udc:519.6`).
- A visible semantic graph payoff now exists in the current Graph View via `ViewDimension::ThreeD { z_source: UdcLevel }`, exposed from the selected-node enrichment card as a UDC depth view action.
- Remaining Phase 1 work is dataset breadth + richer provider routing beyond MVP defaults.

1.  **Implement `KnowledgeRegistry`**:
    *   Add `udc` crate (or lightweight parser module).
    *   Implement `parse(code: &str) -> Result<UdcPath>`.
    *   Implement `distance(a: &UdcPath, b: &UdcPath) -> f32`.
    *   Implement `SemanticIndex` in `KnowledgeRegistry` (NodeKey map).
    *   **Load UDC Dataset**: Embed a lightweight JSON/CSV of UDC codes and labels.
2.  **Tag Input Inference**:
    *   Integrate `nucleo` fuzzy search in `KnowledgeRegistry::search(query)`.
    *   Update Tag Assignment UI (`T` key) to call `search` and display UDC matches alongside existing tags.
    *   Selecting a UDC match applies the `udc:<code>` tag.
    *   Implement reconciliation logic in `shell/desktop/runtime/registries/knowledge.rs` and wire into `shell/desktop/ui/gui.rs`.

### Phase 2: Semantic Physics
**Goal**: Graph layout reflects semantic similarity.

1.  **Force Implementation**:
    *   Add `SemanticGravity` struct to physics engine.
    *   In `update()`: iterate node pairs (optimized via spatial hash or random sampling for large graphs).
    *   Apply force vector: $\vec{F} = k \cdot \text{similarity} \cdot (\vec{p}_B - \vec{p}_A)$.
2.  **Tuning**:
    *   Add "Semantic Strength" slider to Physics Panel.
    *   Ensure semantic force doesn't overpower topological structure (links should still matter).

### Phase 3: Workbench Auto-Grouping
**Goal**: "Organize my tabs" button.

1.  **Algorithm**:
    *   Collect all un-grouped active tabs.
    *   Cluster by UDC prefix at depth $N$ (configurable).
    *   Example: Group all `004.*` (Comp Sci) together, all `7.*` (Arts) together.
2.  **UI Action**:
    *   Command Palette: "Group Tabs by Subject".
    *   Creates `UserGrouped` edges or visual Tab Groups in the workbench.

---

## Technical Challenges

### 1. The "Multi-Class" Problem
A node might be about "History of Mathematics" (`93:51`).
*   **Strategy**: Nodes can have multiple `udc:` tags.
*   **Physics**: Calculate attraction to *all* matching clusters (node will drift between Math and History clusters).

### 2. Performance
Calculating semantic distance for $N^2$ pairs is expensive.
*   **Optimization**: Pre-calculate cluster centroids.
    *   Identify active UDC clusters (e.g., "The Math Cluster").
    *   Calculate centroid of that cluster.
    *   Pull individual nodes toward the *centroid* (O(N)) instead of every other node (O(N^2)).
    *   This aligns with the "Magnetic Zones" concept from `2026-02-19_layout_advanced_plan.md`.

---

## Validation

1.  **Parser Test**: `udc:51` is recognized as parent of `udc:519`.
2.  **Physics Test**: Create 10 "Math" nodes and 10 "Art" nodes with no edges. Run physics. They should separate into two distinct clouds.
3.  **Grouping Test**: "Group by Subject" correctly buckets tabs into containers labeled "Mathematics", "Arts", etc.

---

## Future: Automated Classification
Once the infrastructure exists, we can use LLMs or simple keyword heuristics (in `AgentRegistry`) to *suggest* UDC tags for untagged pages, closing the loop.
