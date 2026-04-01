# Semantic Tagging and Knowledge Registry - Interaction Spec

**Date**: 2026-03-06
**Status**: Canonical interaction contract — updated 2026-04-01 after UDC plan closure and archival
**Priority**: Implementation-ready

**Related**:

- `GRAPH.md`
- `node_badge_and_tagging_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `faceted_filter_surface_spec.md`
- `facet_pane_routing_spec.md`
- `2026-03-11_graph_enrichment_plan.md`
- `../system/register/knowledge_registry_spec.md`
- `../system/register/canvas_registry_spec.md`
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **KnowledgeRegistry** - parsing, distance, label, and search contracts.
2. **Semantic index** - the runtime data/system separation and reconciliation path.
3. **Semantic physics** - the library force and its tuning contract.
4. **UDC tag semantics** - hierarchical matching rules, canonicalization, multi-class behavior.
5. **Workbench auto-grouping** - "Group tabs by subject" action contract.
6. **Facet/routing parity** - UDC behavior across faceted filtering and facet-pane routing.

Tag assignment UI is covered in `node_badge_and_tagging_spec.md`.

---

## 2. KnowledgeRegistry Contract

`KnowledgeRegistry` is an atomic registry responsible for semantic tag definitions. It implements the strategy pattern via providers routed by tag prefix.

### 2.1 Provider Routing

| Prefix | Provider | Examples |
|--------|----------|---------|
| `udc:` | `UdcProvider` | `udc:51`, `udc:519.6` |
| `schema:` | `SchemaProvider` (future) | `schema:Article` |
| (none) | User-defined (no provider; pass-through) | `work`, `research` |

Unknown prefixes route to a fallback pass-through that accepts the tag as user-defined.

### 2.2 UdcProvider Contracts

- `parse(code: &str) -> Result<UdcPath>` - parses a `udc:` string into a path representation (for example, `[5, 1, 9, 6]` for `udc:519.6`). Must reject malformed codes.
- `distance(a: &UdcPath, b: &UdcPath) -> f32` - returns semantic distance based on shared prefix length. Same code = `0.0`; no shared prefix = `1.0`.
- `label(path: &UdcPath) -> Option<String>` - returns a human-readable label if the code is in the known dataset (for example, `"Mathematics"` for `udc:51`). Returns `None` for unknown deep codes; the tag is still valid and clusters correctly with known parents.
- `search(query: &str) -> Vec<TagSuggestion>` - fuzzy search over known labels and codes. Returns ranked suggestions including label-first matches (for example, `"math"` -> `[TagSuggestion { tag: "udc:51", label: "Mathematics", score: ... }]`).

### 2.2A UDC Canonicalization and Grammar Contract

The UDC parser must canonicalize accepted values to one stable normalized form.

Accepted input shape:

- Required prefix: `udc:`
- Required code segment: decimal UDC notation (`0-9` and `.` separators)
- Optional whitespace around the full tag is allowed on input but removed at parse time.

Normalization rules:

1. Prefix is normalized to lowercase `udc:`.
2. Internal whitespace is not allowed inside the code segment.
3. Consecutive separators (`..`) are invalid.
4. Leading and trailing separators are invalid.
5. Superfluous trailing zero groups after a decimal are trimmed when they do not change semantic depth.
6. Output string form is stable; equivalent inputs map to one canonical tag string.

Examples:

- `" UDC:51 "` -> canonical `"udc:51"`
- `"udc:519.600"` -> canonical `"udc:519.6"`
- `"udc:51..9"` -> invalid
- `"udc:.51"` -> invalid

Canonicalization invariant:

- The same semantic code must produce identical canonical string and identical `UdcPath`.
- Canonicalization is pure and deterministic (no locale or runtime-state dependence).

### 2.3 Validate Contract

`KnowledgeRegistry::validate(tag: &str) -> ValidationResult`

- `ValidationResult::Valid` - tag is accepted; may include a label hint.
- `ValidationResult::Warning { reason }` - tag is accepted but carries a non-blocking warning (for example, unrecognized `#` prefix).
- `ValidationResult::Invalid { reason }` - tag is rejected (for example, malformed UDC code).

Invalid tags must not be emitted as `TagNode` intents. Warning tags may be emitted; the UI shows the warning indicator but does not block.

**Performance contract**: All `KnowledgeRegistry` operations called from the UI frame loop (`validate`, `search`, `label`) must complete within frame budget. They must not block on I/O.

---

## 3. Semantic Index Contract

### 3.1 Data / System Separation

- **Data** (app-owned): `GraphBrowserApp` holds `semantic_tags: HashMap<NodeKey, HashSet<String>>` and `semantic_index_dirty: bool`.
- **System** (registry-owned): `KnowledgeRegistry` holds parsing/matching logic; no mutable app state.

The reducer path must not call `KnowledgeRegistry` directly. The UI frame loop runs reconciliation after intent application.

### 3.2 Reconciliation Path

After `apply_reducer_intents()`:

1. `TagNode` / `UntagNode` intents update `semantic_tags` and set `semantic_index_dirty = true`.
2. Frame loop calls `knowledge::reconcile_semantics(app, registry)`.
3. Reconcile checks dirty flag; if clean, returns immediately.
4. Parses tags via `KnowledgeRegistry`, updates `app.semantic_index: HashMap<NodeKey, SemanticClassVector>`, prunes stale node keys.
5. Clears dirty flag.

`SemanticClassVector`:

```
SemanticClassVector {
  classes: Vec<CompactCode>,        // deduplicated canonical UDC classes for node
  primary_code: Option<CompactCode> // deterministic reduction for scalar consumers
}
```

`CompactCode`: A compact representation of one canonical UDC path (for example, `u64` or byte-vector) used for fast prefix-distance evaluation.

Deterministic reduction rule (`primary_code`):

1. Canonicalize and deduplicate all valid UDC classes for the node.
2. Sort by `(path_depth DESC, canonical_tag_lex ASC)`.
3. Select the first element as `primary_code`.

Rationale: deepest class preserves specificity; lexical tiebreak keeps reduction stable and reproducible.

**Invariant**: `semantic_index` must always be derivable from `semantic_tags` + `KnowledgeRegistry`. It is a cache, not a primary source of truth.

Multi-class invariant:

- `classes` is the canonical source for semantic distance and clustering behavior.
- `primary_code` exists only for compatibility with legacy scalar callsites and diagnostics summaries.
- New semantic algorithms must consume `classes` rather than `primary_code`.

### 3.3 Faceted Filter Projection Contract (`udc_classes`)

For faceted filtering (`faceted_filter_surface_spec.md`), `udc_classes` is a collection-valued facet projected from canonicalized UDC tags.

Projection rules:

1. Only valid canonicalized UDC tags participate in `udc_classes`.
2. Non-UDC tags are excluded from `udc_classes`.
3. Duplicate equivalent UDC codes collapse to one canonical value.

Operator behavior for `udc_classes`:

- `ContainsAny` matches when at least one operand code matches node classes.
- `ContainsAll` matches when all operand codes are present.
- Hierarchical parent-prefix matching is enabled for UDC operands:
  - Operand `udc:51` matches node class `udc:519.6`.
  - Operand `udc:519.6` does not match node class `udc:51`.
- `Eq` on `udc_classes` is exact canonical equality and does not imply parent-prefix expansion.

Type-safety invariant:

- Non-collection operators applied to `udc_classes` resolve to no match and emit type-mismatch diagnostics.

---

## 4. Semantic Physics Contract (Library Force)

### 4.1 Force Model

A `SemanticGravity` custom force is registered as an `ExtraForce` in the physics engine (see `layout_behaviors_and_physics_spec.md`). It runs alongside Fruchterman-Reingold via the post-physics injection hook.

For every node pair (A, B) where both have UDC semantic tags:

```
classes_a = semantic_index[A].classes
classes_b = semantic_index[B].classes

pair_similarity(a, b) = max(1.0 - distance(ca, cb)) for all ca in classes_a, cb in classes_b
F = k_semantic * similarity * (position_B - position_A)
```

Where:

- `similarity = pair_similarity(A, B)`.
- If either class set is empty, `similarity = 0.0`.
- Aggregation by `max` preserves bridge behavior for multi-class nodes while keeping attraction monotonic under added compatible classes.

`k_semantic` is tunable (default from `PhysicsProfile`; also exposed as a "Semantic Strength" control in the Physics settings panel).

### 4.2 Performance

For large graphs (N nodes), O(N^2) pairwise calculation is too expensive. The centroid optimization applies:

1. Identify active UDC clusters (groups of nodes sharing a UDC prefix).
2. Compute cluster centroid.
3. Apply attraction to centroid (O(N)) rather than every peer node (O(N^2)).

This approximation is acceptable for library force behavior; exact pairwise calculation is reserved for small graphs (N <= threshold configured in `CanvasRegistry`).

### 4.3 Multi-Class Behavior

A node may carry multiple UDC tags (for example, `udc:51` and `udc:93`). The node calculates attraction to all matching clusters. The node drifts toward the spatial region between its semantic homes.

Reduction compatibility rule:

- Any UI surface that requires a single displayed "primary class" uses `primary_code`.
- Physics and clustering computations must continue using full `classes` set semantics.

### 4.4 Tuning

| Control | Location | Effect |
|---------|----------|--------|
| Semantic Strength slider | Physics panel in settings | `k_semantic` coefficient |
| Semantic force on/off | `CanvasRegistry.semantic_gravity_enabled` | Enables/disables `SemanticGravity` `ExtraForce` |

Semantic force must not overpower topological structure (link-based forces). The default `k_semantic` must leave topological layout recognizable.

---

## 5. Workbench Auto-Grouping Contract

### 5.1 Action

Command: `"Group Tabs by Subject"` - available in the Command Palette and the workbench action surface.

### 5.2 Algorithm

1. Collect all ungrouped active tabs in the current workbench.
2. Cluster by UDC prefix at depth N (configurable; default depth = 1, for example, `udc:5` = Science).
3. For each cluster with >=2 members: create a `UserGrouped` edge set or a visual Tab Group in the workbench.
4. Emit as a batch of `GraphIntent` mutations (not a single atomic transaction).

### 5.3 Grouping Depth

The clustering depth N is configurable per invocation (exposed as a parameter in the command). Shallower depth (N=1) creates broader groups; deeper depth (N=3) creates narrow groups.

### 5.4 Invariant

Auto-grouping must not delete nodes or edges. It only creates new `UserGrouped` edges or workbench container assignments. The operation is reversible by undoing the emitted intents.

---

## 5A. Facet Rail and Pane Routing Parity (UDC)

UDC classification must be consistently visible and routable through facet-rail flows defined in `facet_pane_routing_spec.md`.

Parity rules:

1. For a single selected node, `Space` facet pane route includes canonicalized `udc_classes` in pane payload.
2. The same node rendered in filter pane and routed pane shows identical canonical UDC strings.
3. Invalid/non-canonical UDC inputs are never surfaced in routed pane payloads.
4. If no valid UDC class exists, the `Space` pane remains routable and explicitly shows empty classification state.

Focus/UX invariant:

- Enter-to-pane routing failures caused by malformed/unsupported UDC payloads are `Warn`-level blocked routes and must preserve facet-rail focus context.

---

## 5B. UxTree Exposure (UDC)

UDC semantic information must be visible to UxTree consumers as host-UI semantics.

Required UxTree projections:

- Node semantic-summary surfaces expose canonical UDC labels/codes in `Space` context.
- Facet-filter chips for UDC constraints expose operator and canonical code value.
- `Space` pane route status surfaces expose whether UDC-derived payload is present/empty.

These projections are normative for UxProbe/UxHarness coverage and must not depend on web-content accessibility trees.

---

## 5C. Diagnostics Contract (UDC)

| Channel | Severity | Required fields |
|---|---|---|
| `ux:udc_canonicalized` | `Info` | `raw_tag`, `canonical_tag`, `node_key` |
| `ux:udc_invalid_tag` | `Warn` | `raw_tag`, `reason`, `node_key` |
| `ux:udc_filter_match` | `Info` | `graph_id`, `operand`, `match_count`, `operator` |
| `ux:udc_filter_type_mismatch` | `Warn` | `operator`, `operand_type`, `facet_key` |
| `ux:udc_route_payload_blocked` | `Warn` | `node_key`, `facet_route_key`, `reason` |

Severity rule: parse/normalization and routing precondition failures are `Warn`; successful canonicalization/match events are `Info`.

### 5D. Shared Channel Registry Anchor (UDC/Facet/Tagging)

This table is the canonical diagnostics naming/severity anchor for UDC-related
contracts used by:

- `semantic_tagging_and_knowledge_spec.md`
- `node_badge_and_tagging_spec.md`
- `faceted_filter_surface_spec.md`
- `facet_pane_routing_spec.md`

Cross-spec rule:

- If a channel appears in any of the above specs, its name and baseline severity
  must match this table unless a spec explicitly documents a stricter local rule.
- Renames or severity changes must be updated here in the same slice.

| Channel | Baseline severity | Contract intent |
|---|---|---|
| `ux:udc_canonicalized` | `Info` | Accepted UDC input normalized to canonical form |
| `ux:udc_invalid_tag` | `Warn` | Rejected malformed/unsupported UDC input |
| `ux:udc_filter_match` | `Info` | Successful UDC facet-filter evaluation summary |
| `ux:udc_filter_type_mismatch` | `Warn` | Invalid operator/type use against UDC facet projection |
| `ux:udc_route_payload_blocked` | `Warn` | UDC payload precondition blocked for facet-pane route |
| `ux:facet_filter_applied` | `Info` | Faceted filter set applied successfully |
| `ux:facet_filter_invalid_query` | `Warn` | Faceted query parse/validation failed |
| `ux:facet_filter_eval_failure` | `Error` | Faceted evaluation runtime failure |
| `ux:facet_pane_route_attempt` | `Info` | Facet-pane route attempt emitted |
| `ux:facet_pane_route_blocked` | `Warn` | Facet-pane route blocked by precondition |
| `ux:facet_pane_route_failed` | `Error` | Facet-pane routing runtime failure |
| `ux:facet_pane_focus_return` | `Info` | Focus return result after pane dismiss/back |

---

## 6. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| `udc:51` recognized as parent of `udc:519` | Test: `distance(parse("udc:51"), parse("udc:519")) < distance(parse("udc:51"), parse("udc:93"))` |
| Malformed UDC code rejected | Test: `validate("udc:abc")` -> `ValidationResult::Invalid` |
| Unknown deep code accepted with `None` label | Test: `label(parse("udc:999.999.999"))` -> `None`; `validate` -> `Valid` |
| Equivalent UDC forms canonicalize identically | Test: `parse(" UDC:519.600 ")` and `parse("udc:519.6")` produce same canonical tag/path |
| Dirty-flag reconcile only re-indexes on change | Test: call reconcile twice with no intent -> second call is no-op |
| `primary_code` reduction is deterministic | Test: same multi-tag input set in different insertion orders yields identical `primary_code` |
| Semantic gravity separates unconnected clusters | Test: 10 Math nodes + 10 Art nodes, no edges, physics run -> two distinct spatial groups |
| Centroid optimization produces comparable clustering to pairwise | Test: centroid result vs. pairwise result within 10% spatial distance |
| Multi-class node drifts between clusters | Test: node with `udc:51` + `udc:93` -> position between Math and History centroids |
| Pair similarity uses multi-class set semantics | Test: node A (`udc:51`,`udc:93`) vs node B (`udc:93`) yields higher similarity than A vs node C (`udc:62`) |
| `k_semantic = 0` disables semantic separation | Test: all nodes converge regardless of tags |
| Auto-group creates `UserGrouped` edges, not deletions | Test: "Group by Subject" -> no node or edge deletions in intent log |
| `udc_classes` `ContainsAny`/`ContainsAll` behavior is deterministic | Test: faceted query over mixed-node set yields stable match sets for both operators |
| Parent-prefix facet matching works only in collection operators | Test: `ContainsAny(udc:51)` matches `udc:519.6`; `Eq(udc:51)` does not |
| Space facet pane payload preserves canonical UDC classes | Scenario test: Enter route to `facet:space` emits canonicalized `udc_classes` only |
| UxTree exposes UDC chips and route status | Probe test: UDC filter chips and `Space` route-status nodes are present with expected values |

Green-exit for UDC classification closure requires all criteria above and at least one UxHarness scenario that covers:

1. canonicalization,
2. filter application with `ContainsAny`,
3. Enter-to-pane `Space` routing return path.
