<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Domain Projection Matrix

**Date**: 2026-04-21
**Status**: Canonical / cross-reference catalog
**Scope**: Enumerate the named projections across Graphshell domain pairs.
Reveals which domain-pair bridges are formally named, which are implicit,
and where new projections (e.g., Graph Cartography) slot in.

**Related**:

- [../../TERMINOLOGY.md#projection-concepts](../../TERMINOLOGY.md) — umbrella definition, three-form convention, mechanism terms
- [unified_view_model.md](unified_view_model.md) — five-domain model (Shell / Graph / Navigator / Workbench / Viewer)
- [graph_tree_spec.md](graph_tree_spec.md) — `ProjectionLens` as Shape-stage mechanism for tree-family projections
- [../implementation_strategy/navigator/navigator_projection_spec.md](../implementation_strategy/navigator/navigator_projection_spec.md) — Navigator projection pipeline
- [../implementation_strategy/subsystem_history/2026-04-21_graph_runtime_projection_layer_plan.md](../implementation_strategy/subsystem_history/2026-04-21_graph_runtime_projection_layer_plan.md) — Graph Cartography layer
- [../implementation_strategy/subsystem_history/2026-04-17_graph_memory_architecture_note.md](../implementation_strategy/subsystem_history/2026-04-17_graph_memory_architecture_note.md) — substrate and downstream projection layers

---

## What This Catalog Is

A projection bridges a **source** (a domain's truth or substrate) with a
**target** (a representation consumed by a different domain or surface). The
three-form convention from TERMINOLOGY.md applies: a *projection* is the
pattern, a *projected view* is the outcome, *projecting* is the process.

This document enumerates projections that exist or are planned, flags
structurally-projections-but-not-currently-named ones, and lists open
gaps where a domain pair has no contract yet.

Scope note: routing and mutation paths (e.g., Navigator → Graph via intents)
are not projections — a projection is a read/derivation, not a mutation
route. Routing contracts are cataloged elsewhere.

## Matrix

### Graph (truth) → Other Domains

| Target | Projection | Canonical doc | Status |
|---|---|---|---|
| Navigator | **Navigator projection** (all Navigator sections and graphlet derivations — pattern) | NAVIGATOR.md, navigator_projection_spec.md, archived navigator producing plan | Live; canonical pipeline spec landed |
| Workbench | **Projection Rule** (nodes→tiles, graphlets→tile groups, frames→frames — correspondence) | TERMINOLOGY.md Tile Tree Architecture §Projection Rule; workbench/2026-03-20_arrangement_graph_projection_plan.md | Live; graph-tree migration active |
| Viewer | **Viewer resolution** (AddressKind + ContentKind + mime_hint → viewer backend — backend selection) | viewer/viewer_presentation_and_fallback_spec.md; ViewerRegistry contract | Live; not previously named as a projection — flagged here for consistency |
| Graph canvas (rendered surface) | **Canvas rendering** (graph truth → spatial/visual rendering at current LOD) | graph/graph_canvas_spec.md; graph/graph_node_edge_interaction_spec.md | Live; not previously named as a projection |
| Workbench (secondary) | **ArrangementRelation projection** (arrangement edges → tile group / frame membership) | workbench/2026-03-20_arrangement_graph_projection_plan.md | Active migration |

### Graph Memory (substrate) → Other Targets

| Target | Projection | Canonical doc | Status |
|---|---|---|---|
| Graph-memory consumers | **Branch projection** (substrate → owner-scoped current path + alternate children) | subsystem_history/2026-04-17_graph_memory_architecture_note.md §3.5 | Live (`branch_projection()`) |
| Graph-memory consumers | **AggregatedEntryEdgeView** (substrate visit parentage → entry-level multi-graph) | subsystem_history/2026-04-17_graph_memory_architecture_note.md §3.5 | Live |
| Graph-memory consumers | **Semantic summary** (substrate → current URL, last-visit time, visit count) | subsystem_history/2026-04-17_graph_memory_architecture_note.md §6 | Live (`semantic_summary()`) |
| Linear-history consumers | **Linear projection** (substrate → flat linear navigation history) | subsystem_history/2026-04-17_graph_memory_architecture_note.md §6 | Live (`projection()`) |
| VGCP wire format | **Contribution projection** (live memory → canonicalized signed shareable artifacts) | subsystem_history/2026-04-17_graph_memory_architecture_note.md §8.2; verse_docs/technical_architecture/2026-04-17_verse_graph_contribution_protocol_v0_1.md | Planned downstream layer |

### Graph + Substrate + WAL → Navigator/Canvas Consumers

| Target | Projection | Canonical doc | Status |
|---|---|---|---|
| Navigator pipeline scorer / annotation / parent-picker slots | **Graph Cartography projection** (aggregates: co-visit, co-activation, cluster stability, frame-reformation, bridge emphasis) | subsystem_history/2026-04-21_graph_runtime_projection_layer_plan.md; navigator/navigator_projection_spec.md | Plan; fills declared pipeline slots |
| Canvas ambient effects | **Cartographic overlays** (hotspot halos, heat, tidal influence, edge tension — consuming GC aggregates) | research/2026-03-27_ambient_graph_visual_effects.md; graph/2026-04-03_semantic_clustering_follow_on_plan.md | Research; downstream of GC |

### History / WAL → Surfaces

| Target | Projection | Canonical doc | Status |
|---|---|---|---|
| History Manager (mixed timeline tab) | **Mixed timeline projection** (WAL → typed union with filter) | subsystem_history/2026-03-18_mixed_timeline_contract.md | Live |
| Navigator Recent section | **Traversal projection** (WAL → recent-history rows with activity annotation) | subsystem_history/SUBSYSTEM_HISTORY.md §4.1; NAVIGATOR.md §8 | Live (read-only, per SUBSYSTEM_HISTORY §0A.7 shared-projection policy) |
| Node pane history surface | **Node navigation history projection** | subsystem_history/SUBSYSTEM_HISTORY.md §9 | Live |
| Node pane audit surface | **Node audit history projection** | subsystem_history/node_audit_log_spec.md | Live |

### Specialty / Scoped Projections

| Target | Projection | Canonical doc | Status |
|---|---|---|---|
| Thread-shaped Navigator view | **Constellation projection** (anchor + replies/references/frontier → cluster layout) | navigator/2026-04-09_constellation_projection_plan.md | Plan |
| Semantic cluster assignment | **Semantic clustering projection** (embeddings → cluster membership, centroid, label) | graph/2026-04-03_semantic_clustering_follow_on_plan.md | Plan |
| Discovery / constellation frontier | **Discovery pack manifest projection** | navigator/2026-04-09_discovery_pack_manifest_and_install_flow.md | Plan |
| Source subscription health | **Source health projection** | navigator/2026-04-09_source_subscription_manager_and_health.md | Plan |
| Facet navigation surface | **PMEST faceted filter projection** | graph/faceted_filter_surface_spec.md; graph/facet_pane_routing_spec.md | Live/partial |
| Accessibility tree | **UxTree projection** (UI structure → AccessKit-compatible node tree) | subsystem_ux_semantics/ux_tree_and_probe_spec.md; subsystem_ux_semantics/ux_event_dispatch_spec.md | Live |
| Diagnostics pane | **Diagnostic channel projection** (runtime signals → channel schema + analyzers) | subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md | Live |

### Pipeline / Mechanism Inventory

Mechanisms that implement projections (not projections themselves):

| Mechanism | Role | Canonical doc |
|---|---|---|
| Navigator **projection pipeline** (five-stage: Scope → Shape → Annotation → Presentation → Portal) | Navigator projection mechanism | navigator/navigator_projection_spec.md; `../../archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md` |
| `ProjectionLens` (Rust enum in `graph-tree`) | Shape-stage mechanism for tree-family projections; variants parameterize which edge family drives parent-child | technical_architecture/graph_tree_spec.md §6.7 |
| `LayoutMode` (Rust enum in `graph-tree`) | Presentation-stage mechanism for tree-family projections (TreeStyleTabs, FlatTabs, SplitPanes, Hybrid) | technical_architecture/graph_tree_spec.md §6 |
| `NavAction` / `TreeIntent` | Portal-stage mechanism for tree-family projections | technical_architecture/graph_tree_spec.md §6.8 |
| `GraphTreeRenderer` adapters (egui / iced / web) | Presentation-stage framework adapters | technical_architecture/graph_tree_spec.md §4 |
| `CompositorAdapter` | Composition pass mechanism for node viewer pane rendering | aspect_render/frame_assembly_and_compositor_spec.md |
| `LensCompositor` | Composes Layout + Presentation + Knowledge + Filters (the `Lens` concept — distinct from `ProjectionLens`) | system/register/lens_compositor_spec.md |

## Gaps and Observations

- **Viewer resolution and canvas rendering aren't currently framed as projections.** They structurally are — both take graph truth and produce a representation for consumption in another layer. Naming them this way would align language without changing behavior.
- **Non-tree Shape-stage mechanisms are missing.** `ProjectionLens` covers tree/tab/split presentations via `LayoutMode`. Graphlet-as-graph (radial, corridor, atlas), time-axis, and summary/minimap shapes need their own Shape-stage mechanisms wrapped by the same pipeline.
- **The `Lens` concept and `ProjectionLens` are distinct.** `Lens` (per TERMINOLOGY.md Visual System) = Layout + Theme + Physics Profile + Filter — a configurable graph appearance/motion composition. `ProjectionLens` = a Shape-stage enum for tree-family projections. They do not collide architecturally but share a word; the three-form convention helps ("a projection produced under the Recency `ProjectionLens`" vs. "the active `Lens` is Liquid").
- **Contribution projection is named but not yet built.** Graph-memory note §8.2 describes it; VGCP §8, §9.2, §9.5 defines its target shape. A producing plan is outstanding.
- **Cartography aggregates render as annotations, not as edges.** See GC plan Phase 5 — the `EdgeKind` taxonomy stays closed; aggregates surface through the annotation registry (projection pipeline A3).

## Maintenance

- Any new domain-pair projection adds a row here in the same session as its canonical plan/spec (DOC_POLICY §6.1 DOC_README update also applies).
- When a projection pattern is retired, mark the row archived rather than deleting it (fallback historical reference).
- When a structurally-present-but-unnamed projection (e.g., Viewer resolution) gets a canonical name, move its row from the "not previously named" flag to the regular listing.
