# Clipping And DOM Extraction — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical viewer-lane interaction contract, updated for current runtime reality
**Priority**: Active

**Related**:

- `VIEWER.md`
- `2026-04-03_clipping_viewer_follow_on_plan.md`
- `../graph/2026-03-11_graph_enrichment_plan.md`
- `../aspect_projection/ASPECT_PROJECTION.md`
- `../aspect_distillery/ASPECT_DISTILLERY.md`
- `../system/2026-03-12_architectural_inconsistency_register.md`

---

## 1. Scope

This spec defines the canonical viewer-lane contracts for:

1. context-menu / contextual-surface entry into clipping,
2. read-only DOM extraction for single-hit clip and inspector-candidate discovery,
3. inspector-first selection before multi-element materialization,
4. clip node materialization and source linkage,
5. clip-route/address behavior at the current bridge stage,
6. viewer-lane boundaries versus projection, enrichment, analysis, and distillery follow-ons.

This spec does **not** define a general document-analysis system.

Out of scope here:

- site-wide link harvesting as graph mutation,
- reusable selector-recipe APIs,
- automatic materialization of full document structure into the user graph,
- typed extraction artifacts beyond clip-local metadata needed for enrichment handoff.

---

## 2. Current Runtime Reality

The older drafts of this feature mixed current behavior, historical proposals, and future ideas too freely. The current effective runtime shape is:

- backend context meaning is used as an adapter into Graphshell-owned clip/inspect surfaces,
- Graphshell has a real clip inspector state and panel,
- single and batch clip materialization helpers are landed,
- clip nodes participate in the graph model through `Address::Clip(...)` / `AddressKind::GraphshellClip`,
- runtime accepts both `verso://clip/...` and legacy `graphshell://clip/...` during the bridge period,
- clip creation already hands off inherited classifications into the enrichment lane with explicit inherited provenance.

**Invariant**: this feature must be documented as a current-state viewer contract, not as a greenfield proposal.

---

## 3. Context Entry Contract

### 3.1 Entry path

Clipping begins from a viewer-context action over web content.

Current runtime seam is backend context-menu plumbing feeding Graphshell-owned clip/inspect actions. The old `GraphSemanticEvent::ContextMenu` story remains useful as design history, but it is not the best description of the current runtime path.

**Invariant**: browser-native context meaning is an adapter seam. Graphshell-owned command and inspection surfaces remain authoritative.

### 3.2 Two entry modes

Graphshell supports two user-facing entry modes:

- **Direct clip**: one-hit capture for simple cases.
- **Inspect page elements**: bounded candidate discovery and in-Graphshell selection for complex pages.

**Invariant**: direct clip is allowed as a narrow convenience path, but multi-element discovery must remain inspector-first.

---

## 4. Read-Only Extraction Contract

Graphshell performs read-only DOM extraction against the active page.

Current extraction classes:

1. **Single-hit extraction**: resolve the element under the interaction point and return one capture payload.
2. **Inspector candidate discovery**: score and return a bounded set of likely useful page elements.
3. **Pointer-stack inspection**: return stacked/nested element candidates for in-situ stepping when available.

Current capture payload is viewer-owned clip metadata, not a general analysis schema. It includes enough data for:

- clip rendering,
- clip title resolution,
- source provenance,
- local display and re-open,
- enrichment handoff.

Typical fields include:

- source URL,
- page title,
- clip title,
- outer HTML,
- text excerpt,
- tag name,
- link/image hints,
- DOM path or equivalent local locator.

**Invariant**: extraction scripts are read-only and must not mutate page DOM, initiate navigation, or read credential-bearing storage surfaces.

**Invariant**: viewer-owned clip capture payload is intentionally narrower than a general document-analysis schema.

---

## 5. Inspector-First Interaction Contract

Before multi-element clip creation, Graphshell may open an inspector surface over the extracted candidate set.

Current runtime capabilities include:

- candidate list/filtering,
- search query,
- explicit "Clip Selected" and "Clip Filtered" actions,
- pointer-stack stepping over nested elements,
- in-situ highlight support.

Deferred but still aligned viewer-lane follow-ons include:

- stronger ancestor/descendant stepping,
- richer panel ergonomics,
- tighter contextual-palette parity.

**Invariant**: inspector mode is temporary inspection state, not durable graph mutation by default.

**Invariant**: opening inspector mode must not materialize the page element tree into the graph unless the user explicitly clips something.

---

## 6. Exploded Inspector Boundary

The exploded inspector direction remains valid, but its current architectural meaning must stay narrow.

At this stage:

- it is a temporary viewer-owned inspection representation,
- it may expose structural relationships and grouping cues,
- it does not automatically become durable graph truth.

If the exploded inspector grows into a richer temporary element-tree or element-graph local world, that becomes a **Projection** follow-on under `../aspect_projection/ASPECT_PROJECTION.md`.

**Invariant**: the viewer clipping spec may describe temporary inspection structure, but it does not authorize automatic document-to-graph projection.

---

## 7. Clip Materialization Contract

### 7.1 What materialization does

When the user explicitly clips one or more extracted elements, Graphshell creates ordinary graph nodes representing those clips and links them back to the source node.

Current runtime shape includes:

- single clip creation,
- multi-clip batch materialization,
- source linkage via a labeled semantic edge,
- clip-local metadata sufficient for display and re-open,
- enrichment handoff for inherited source classifications.

### 7.2 Link to source

When a clip node is created from a source node, Graphshell creates a labeled source-link edge using the current `clip-source` label on the semantic grouped relation.

**Invariant**: clip creation and source linkage are one materialization action. A materialized clip without its source linkage is invalid.

### 7.3 Enrichment handoff

Clip materialization may carry inherited source classifications into the new clip node.

Current rule:

- inherited classifications must be marked with inherited provenance,
- inherited classifications must not be silently auto-accepted as if the user authored them on the clip.

This is a handoff into the enrichment lane, not proof that clipping owns enrichment architecture.

---

## 8. Clip Node And Address Contract

### 8.1 Current bridge contract

Clip nodes currently surface through:

- `Address::Clip(...)`,
- `AddressKind::GraphshellClip`,
- the `#clip` reserved tag as the current bridge carrier.

**Invariant**: both `verso://clip/...` and legacy `graphshell://clip/...` are accepted during the bridge period.

### 8.2 `#clip` bridge note

`#clip` is still acting like a content/type facet while being modeled as a tag. This remains an active architectural inconsistency, not a settled long-term choice.

**Invariant**: this spec may describe `#clip` as the current bridge carrier, but must not treat that bridge as final architecture.

**Recommended resolution**:

- keep clip as an explicit content facet on an ordinary node,
- avoid introducing a broad top-level node-type hierarchy just to resolve clip semantics,
- derive `#clip`, clip badge state, and clip query aliases from that explicit facet once it lands.

Recommended shape:

```rust
NodeContentFacet::Clip(ClipFacetData)
```

### 8.3 What is deferred

The following are deferred and not current active contract:

- explicit clip content facet replacing `#clip` as canonical truth while leaving `#clip` as a compatibility projection,
- reusable selector-recipe APIs,
- automatic link-harvest graph mutation,
- distillery-style typed extraction artifacts from page analysis.

---

## 9. Ownership Boundaries

### Viewer lane owns

- context entry into clip/inspect,
- read-only extraction for clip capture,
- inspector-first selection,
- explicit clip materialization,
- clip-local metadata capture required for provenance, display, re-open, and enrichment handoff.

### Projection follow-on owns

- richer temporary element-tree or element-graph local worlds beyond the current inspector panel/state.

### Enrichment follow-on owns

- durable classification/content-kind policy,
- explanation/filter surfaces for inherited or derived metadata,
- provenance/confidence/status semantics after handoff.

### Analysis follow-on owns

- outbound-link extraction as graph mutation,
- selector-driven extraction recipes,
- broader document-analysis batches or reusable extraction workflows.

### Distillery follow-on owns

- typed extracted artifacts from page or clip content under privacy-boundary policy.

---

## 10. Acceptance Criteria

| Criterion | Verification |
| --- | --- |
| Context meaning is bridged into Graphshell-owned clip/inspect actions | invoke clipping from web content and verify Graphshell-owned surface handles the flow |
| Direct clip remains available for simple cases | invoke one-hit clip and verify a single clip node is materialized |
| Inspector remains the canonical multi-element path | invoke inspector candidate discovery and verify no clip nodes are created until explicit action |
| Inspector state is temporary by default | open inspector and verify page structure is not materialized into graph truth automatically |
| Clip nodes surface through typed clip address behavior | materialized clip resolves through `Address::Clip(...)` / `AddressKind::GraphshellClip` bridge behavior |
| Both clip route families remain accepted during the bridge period | verify `verso://clip/...` and legacy `graphshell://clip/...` both resolve as clip addresses |
| Clip creation links back to the source node | create a clip and verify the labeled source link exists |
| Inherited source classifications remain inherited, not silently accepted | create a clip from a classified source node and verify inherited provenance / non-accepted status |
| Broader document-analysis behavior is not implied by this spec | reader can distinguish clip capture from analysis follow-ons without conflating their contracts |

---

## 11. Reading Rule

When this spec and related plans mention broader ideas such as exploded element graphs, link extraction, selector recipes, or typed extraction artifacts, read them as **adjacent follow-ons** unless they are explicitly defined here as active viewer-lane contract.

The clipping viewer lane is intentionally narrower:

- inspect,
- select,
- clip,
- hand off.
