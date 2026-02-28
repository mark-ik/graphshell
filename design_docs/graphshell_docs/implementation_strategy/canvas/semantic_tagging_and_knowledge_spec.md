# Semantic Tagging and Knowledge Registry — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Implementation-ready (Phase 1 in progress)

**Related**:

- `CANVAS.md`
- `node_badge_and_tagging_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `2026-02-23_udc_semantic_tagging_plan.md`
- `../system/register/knowledge_registry_spec.md`
- `../system/register/canvas_registry_spec.md`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **KnowledgeRegistry** — parsing, distance, label, and search contracts.
2. **Semantic index** — the runtime data/system separation and reconciliation path.
3. **Semantic physics** — the library force and its tuning contract.
4. **UDC tag semantics** — hierarchical matching rules, multi-class behavior.
5. **Workbench auto-grouping** — "Group tabs by subject" action contract.

Tag assignment UI is covered in `node_badge_and_tagging_spec.md`.

---

## 2. KnowledgeRegistry Contract

`KnowledgeRegistry` is an atomic registry responsible for semantic tag definitions. It implements the **strategy pattern** via providers routed by tag prefix.

### 2.1 Provider Routing

| Prefix | Provider | Examples |
|--------|----------|---------|
| `udc:` | `UdcProvider` | `udc:51`, `udc:519.6` |
| `schema:` | `SchemaProvider` (future) | `schema:Article` |
| (none) | User-defined (no provider; pass-through) | `work`, `research` |

Unknown prefixes route to a fallback pass-through that accepts the tag as user-defined.

### 2.2 UdcProvider Contracts

- `parse(code: &str) -> Result<UdcPath>` — parses a `udc:` string into a path representation (e.g., `[5, 1, 9, 6]` for `udc:519.6`). Must reject malformed codes.
- `distance(a: &UdcPath, b: &UdcPath) -> f32` — returns semantic distance based on shared prefix length. Same code = 0.0; no shared prefix = 1.0.
- `label(path: &UdcPath) -> Option<String>` — returns a human-readable label if the code is in the known dataset (e.g., `"Mathematics"` for `udc:51`). Returns `None` for unknown deep codes; the tag is still valid and clusters correctly with known parents.
- `search(query: &str) -> Vec<TagSuggestion>` — fuzzy search over known labels and codes. Returns ranked suggestions including label-first matches (e.g., `"math"` → `[TagSuggestion { tag: "udc:51", label: "Mathematics", score: … }]`).

**Performance contract**: All `KnowledgeRegistry` operations called from the UI frame loop (`validate`, `search`, `label`) must complete within the frame budget. They must not block on I/O.

### 2.3 Validate Contract

`KnowledgeRegistry::validate(tag: &str) -> ValidationResult`

- `ValidationResult::Valid` — tag is accepted; may include a label hint.
- `ValidationResult::Warning { reason }` — tag is accepted but carries a non-blocking warning (e.g., unrecognized `#` prefix).
- `ValidationResult::Invalid { reason }` — tag is rejected (e.g., malformed UDC code).

Invalid tags must not be emitted as `TagNode` intents. Warning tags may be emitted; the UI shows the warning indicator but does not block.

---

## 3. Semantic Index Contract

### 3.1 Data / System Separation

- **Data** (app-owned): `GraphBrowserApp` holds `semantic_tags: HashMap<NodeKey, HashSet<String>>` and `semantic_index_dirty: bool`.
- **System** (registry-owned): `KnowledgeRegistry` holds parsing/matching logic; no mutable app state.

The reducer path must not call `KnowledgeRegistry` directly. The UI frame loop runs reconciliation after intent application.

### 3.2 Reconciliation Path

After `apply_intents()`:

1. `TagNode` / `UntagNode` intents update `semantic_tags` and set `semantic_index_dirty = true`.
2. Frame loop calls `knowledge::reconcile_semantics(app, registry)`.
3. Reconcile checks dirty flag; if clean, returns immediately.
4. Parses tags via `KnowledgeRegistry`, updates `app.semantic_index: HashMap<NodeKey, CompactCode>`, prunes stale node keys.
5. Clears dirty flag.

**CompactCode**: A compact representation of the UDC path optimized for O(1) distance calculation (e.g., `u64` or byte-vector). Derived by the reconcile step; never stored independently of `semantic_tags`.

**Invariant**: The `semantic_index` must always be derivable from `semantic_tags` + `KnowledgeRegistry`. It is a cache, not a primary source of truth.

---

## 4. Semantic Physics Contract (Library Force)

### 4.1 Force Model

A `SemanticGravity` custom force is registered as an `ExtraForce` in the physics engine (see `layout_behaviors_and_physics_spec.md §7.2`). It runs alongside Fruchterman-Reingold via the post-physics injection hook.

For every node pair (A, B) where both have UDC semantic tags:

```
similarity = 1.0 - distance(semantic_index[A], semantic_index[B])
F = k_semantic * similarity * (position_B - position_A)
```

`k_semantic` is tunable (default from `PhysicsProfile`; also exposed as a "Semantic Strength" control in the Physics settings panel).

### 4.2 Performance

For large graphs (N nodes), O(N²) pairwise calculation is too expensive. The centroid optimization applies:

1. Identify active UDC clusters (groups of nodes sharing a UDC prefix).
2. Compute cluster centroid.
3. Apply attraction to centroid (O(N)) rather than every peer node (O(N²)).

This approximation is acceptable for the library force; exact pairwise calculation is reserved for small graphs (N ≤ threshold configured in `CanvasRegistry`).

### 4.3 Multi-Class Behavior

A node may carry multiple UDC tags (e.g., `udc:51` and `udc:93`). The node calculates attraction to all matching clusters. The node drifts toward the spatial region between its semantic homes.

### 4.4 Tuning

| Control | Location | Effect |
|---------|----------|--------|
| Semantic Strength slider | Physics panel in settings | `k_semantic` coefficient |
| Semantic force on/off | `CanvasRegistry.semantic_gravity_enabled` | Enables/disables `SemanticGravity` ExtraForce |

Semantic force must not overpower topological structure (link-based forces). The default `k_semantic` must leave topological layout recognizable.

---

## 5. Workbench Auto-Grouping Contract

### 5.1 Action

Command: `"Group Tabs by Subject"` — available in the Command Palette and the workbench action surface.

### 5.2 Algorithm

1. Collect all ungrouped active tabs in the current workbench.
2. Cluster by UDC prefix at depth N (configurable; default depth = 1, e.g., `udc:5` = Science).
3. For each cluster with ≥2 members: create a `UserGrouped` edge set or a visual Tab Group in the workbench.
4. Emit as a batch of `GraphIntent` mutations (not a single atomic transaction).

### 5.3 Grouping Depth

The clustering depth N is configurable per invocation (exposed as a parameter in the command). Shallower depth (N=1) creates broader groups ("Science", "Arts"); deeper depth (N=3) creates narrow groups ("Computational Mathematics").

### 5.4 Invariant

Auto-grouping must not delete nodes or edges. It only creates new `UserGrouped` edges or workbench container assignments. The operation is reversible by undoing the emitted intents.

---

## 6. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| `udc:51` recognized as parent of `udc:519` | Test: `distance(parse("udc:51"), parse("udc:519")) < distance(parse("udc:51"), parse("udc:93"))` |
| Malformed UDC code rejected | Test: `validate("udc:abc")` → `ValidationResult::Invalid` |
| Unknown deep code accepted with `None` label | Test: `label(parse("udc:999.999.999"))` → `None`; `validate` → `Valid` |
| Dirty-flag reconcile only re-indexes on change | Test: call reconcile twice with no intent → second call is no-op |
| Semantic gravity separates unconnected clusters | Test: 10 Math nodes + 10 Art nodes, no edges, physics run → two distinct spatial groups |
| Centroid optimization produces comparable clustering to pairwise | Test: centroid result vs. pairwise result within 10% spatial distance |
| Multi-class node drifts between clusters | Test: node with `udc:51` + `udc:93` → position between Math and History centroids |
| `k_semantic = 0` disables semantic separation | Test: all nodes converge regardless of tags |
| Auto-group creates `UserGrouped` edges, not deletions | Test: "Group by Subject" → no node or edge deletions in intent log |
