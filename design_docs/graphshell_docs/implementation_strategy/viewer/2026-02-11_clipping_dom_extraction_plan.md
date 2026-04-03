# Clipping And DOM Extraction Plan (Refactored 2026-04-02)

**Status**: Active current-state execution plan
**Phase**: Viewer lane, capture-focused
**Architecture**: backend context-menu adapter -> Graphshell-owned inspector surface -> read-only DOM extraction -> explicit clip materialization into graph nodes

**Related**:

- `clipping_and_dom_extraction_spec.md`
- `VIEWER.md`
- `../graph/2026-03-11_graph_enrichment_plan.md`
- `../aspect_projection/ASPECT_PROJECTION.md`
- `../aspect_distillery/ASPECT_DISTILLERY.md`
- `../system/2026-03-12_architectural_inconsistency_register.md`

---

## Summary

This plan is the **viewer-owned clipping plan**, not a general document-analysis roadmap.

Its job is to keep one product lane clear:

- inspect live page structure inside a viewer,
- select the correct element or small set of elements,
- materialize clips explicitly,
- preserve enough local metadata for provenance, display, and re-open.

This document no longer treats clipping as the owner of broader site/document analysis features such as outbound-link harvesting, selector recipes, or typed extraction artifacts. Those remain valid follow-on ideas, but they must live under the correct downstream lanes.

**Posterity note (2026-04-02):** this plan was intentionally rewritten after the viewer/runtime and enrichment seams were clearer in code. The main change was not feature removal; it was boundary cleanup. Clipping was narrowed to inspect/select/materialize/hand-off, broader document analysis was split into follow-on lanes, and the recommended bridge resolution changed from "maybe explicit node type" toward an explicit clip content facet with `#clip` retained as derived compatibility state.

---

## Current Product Shape

The current runtime shape is no longer greenfield. The following are already landed in code:

- clip inspector runtime state and non-modal inspector surface,
- clip materialization helpers for single and batch capture,
- pointer-stack inspection / stacked-element stepping in the inspector,
- inherited source-classification carryover onto clip nodes with explicit inherited provenance,
- clip route handling through `Address::Clip(...)` / `AddressKind::GraphshellClip`,
- bridge acceptance for both `verso://clip/...` and legacy `graphshell://clip/...`.

This plan therefore focuses on:

- tightening the viewer-lane contract,
- recording what is landed versus deferred,
- preventing broader analysis ideas from silently becoming viewer-lane scope.

---

## Scope

### In scope

- context-menu or contextual-surface entry into page inspection,
- single-hit clip capture for simple cases,
- inspector-first candidate discovery for complex pages,
- pointer-stack and candidate inspection over live page content,
- explicit materialization of one or more selected clips,
- clip-local metadata capture needed for provenance, display, re-open, and enrichment handoff.

### Out of scope for this plan

- site-wide link harvesting as graph mutation,
- automatic document-to-graph projection of full extracted structure,
- selector-driven batch extraction recipes that create graph artifacts without a separate analysis contract,
- distillation or intelligence workflows beyond clip-local metadata inheritance,
- treating the entire page element tree as durable graph truth by default.

---

## Architectural Direction

### 1. Viewer-owned inspection first

The clipping lane is owned by the viewer stack. Browser-native context meaning is an adapter seam that feeds Graphshell-owned inspection and clip actions.

The canonical complex-page workflow is:

1. user invokes clip/inspect from page context,
2. Graphshell extracts one hit or a bounded candidate set,
3. user inspects/filter/selects in a Graphshell-owned surface,
4. Graphshell materializes clips explicitly.

Direct clipping outside inspector mode remains allowed for simple one-hit capture. It is a convenience path, not the canonical multi-element discovery path.

### 2. Temporary inspection state, not durable graph mutation

The exploded inspector direction remains valuable, but it must be read as **temporary inspection state** unless and until a later projection contract gives it stronger semantics.

For this plan:

- inspector state is viewer/runtime state,
- clip nodes are durable graph artifacts created only on explicit materialization,
- entering inspector mode must not materialize the page structure into the user graph by default.

### 3. Bridge status of `#clip`

`#clip` remains the current bridge carrier for clip semantics. That is acceptable for the current slice, but it is still an active architectural inconsistency: the system is using a tag like a content/type facet.

This plan must not deepen that ambiguity. Treat `#clip` as a current bridge, not settled long-term authority. See `../system/2026-03-12_architectural_inconsistency_register.md`.

**Recommended resolution**:

- keep nodes as the primary identity model,
- introduce an explicit clip content facet rather than a broad top-level node-type hierarchy,
- treat `#clip`, clip badge state, and `is:clip`-style query affordances as derived compatibility projections from that facet.

Recommended shape:

```rust
NodeContentFacet::Clip(ClipFacetData)
```

Where `ClipFacetData` owns clip-specific truth such as stable clip identity, source provenance, capture metadata, and future storage references.

### 4. Clip route reality

Clip route handling is no longer just a historical `graphshell://clip/<uuid>` idea.

Current effective contract:

- clip addresses are represented as `Address::Clip(...)`,
- they surface as `AddressKind::GraphshellClip`,
- runtime bridge accepts both `verso://clip/...` and legacy `graphshell://clip/...`.

This plan therefore stops presenting `graphshell://clip/...` as the likely runtime direction. The route family remains a bridge period with `verso://clip/...` accepted and the legacy alias retained.

---

## Landed Runtime Shape

### Landed: inspector and materialization seam

- Graphshell has a clip inspector state carrier and inspector panel.
- Graphshell supports single clip creation and multi-clip materialization from extracted captures.
- Pointer-stack stepping exists as the current in-situ inspection affordance.

### Landed: clip metadata and enrichment handoff

- clip capture payload already includes the local metadata needed for clip rendering and provenance handoff,
- clip content now persists through an explicit clip content facet bridge stored in node-owned state,
- runtime viewer display for clip nodes is synthesized from stored clip HTML rather than treating the route URL itself as render authority,
- source classifications can be inherited onto clips with `InheritedFromSource` provenance and non-accepted status,
- clip creation already participates in the enrichment lane as a concrete Stage C producer.

### Landed: route and address typing

- clip route typing exists in the graph model via `Address::Clip(...)` and `AddressKind::GraphshellClip`,
- clip route identity now survives snapshot restore rather than being rewritten back to the source page URL,
- both `verso://clip/...` and `graphshell://clip/...` are accepted during the bridge period.

### Landed: re-open and presentation cleanup

- opening a clip route now resolves to the matching clip node pane rather than only pivoting to History Manager,
- live and historical omnibar/search surfaces now match and label clips by user-visible clip title and source URL rather than leaking internal `verso://clip/...` identity strings,
- user-facing workbench, navigator, accessibility, toolbar, and tag-panel labels now prefer clip-visible metadata over internal route identity.

---

## Deferred Work Inside The Viewer Lane

These remain valid viewer-lane follow-ons:

- stronger inspector ergonomics beyond the current panel and pointer-stack flow,
- richer clip fidelity choices (`Clean`, `Contextual`, `Screenshot Note`, `Offline Slice`),
- more robust extraction coverage for complex page structures,
- dedicated clip content storage/route cleanup after the route bridge settles,
- richer clip presentation and provenance chrome beyond the now-landed visible title/source cleanup.

These are deferred viewer improvements, not separate feature lanes.

---

## Follow-On Lanes Outside This Plan

### Projection follow-on

If the exploded inspector grows into a richer temporary element-tree or element-graph view, that work belongs under the Projection aspect as a derived local world rather than durable graph truth.

### Enrichment follow-on

Clip-derived classifications, content-kind hints, inherited metadata, and explanation/filter surfaces continue under the graph enrichment lane. This plan only hands off clip-local metadata; it does not own the enrichment system.

### Analysis follow-on

Outbound-link extraction, selector-driven extraction recipes, and broader document-analysis batches are intentionally split out from clipping. They require their own contract under graph/projection or graph/enrichment depending on whether the output is temporary derived representation or durable graph metadata.

### Distillery follow-on

Any future workflow that turns page or clip content into typed extracted artifacts must depend on the Distillery aspect and privacy-boundary rules. That work does not belong inside the viewer clipping plan.

### Downstream publication follow-on

Nostr publication is no longer part of the core clipping execution path in this plan. It may remain a downstream integration that consumes clip artifacts, but it should not shape the viewer-lane clipping architecture.

---

## Execution Slices

### Slice 1: Keep the viewer capture contract current

- keep the plan and spec aligned with the real runtime seam,
- document inspector-first as the canonical complex-page workflow,
- document direct one-hit clipping as a narrow convenience path.

### Slice 2: Keep route and bridge language accurate

- describe clips in terms of `Address::Clip(...)` / `AddressKind::GraphshellClip`,
- record `verso://clip/...` plus legacy `graphshell://clip/...` bridge behavior,
- keep route identity distinct from runtime render URL synthesis,
- avoid reasserting `data:` URLs as the long-term clip authority model.

### Slice 3: Keep the analysis boundary explicit

- preserve broader ideas in roadmap language,
- attach them to projection, enrichment, analysis, or distillery follow-ons,
- do not let viewer-lane docs imply ownership of those future systems.

### Slice 4: Keep the `#clip` bridge visible

- continue using `#clip` as the current bridge carrier,
- link explicitly to the architectural inconsistency rather than treating the issue as settled,
- avoid adding new semantics that depend on `#clip` being the final node-type carrier,
- prepare the bridge to collapse into an explicit clip content facet rather than a broad node-type system.

---

## Validation

1. A reader can tell which parts of clipping are already landed versus future work.
2. A reader can tell where viewer-owned clipping ends and broader document/site analysis begins.
3. The plan no longer implies that entering inspector mode materializes graph truth by default.
4. The route/address story reflects current code reality rather than historical `graphshell://clip/...` assumptions.
5. User-facing clip labels and search surfaces no longer depend on internal route identity for display.
6. Broader ideas like link extraction and selector-driven analysis remain on the roadmap, but are attached to explicit downstream lanes rather than hidden inside the viewer plan.

---

## Defaults

- Default workflow: inspector-first for complex pages, direct clipping allowed for simple cases.
- Default ownership: viewer lane owns inspection and explicit clip creation only.
- Default bridge stance: `#clip` remains current bridge carrier, not final content/type authority; recommended destination is an explicit clip content facet with derived tag/badge/query projections.
- Default projection stance: exploded inspector remains temporary derived inspection state until a projection contract says otherwise.
