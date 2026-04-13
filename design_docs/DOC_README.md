# AI Assistant Context

All project documentation lives in `design_docs/`.

## DOC_README Authority

`DOC_README.md` is the authoritative first-reference document for documentation state.

It serves three goals:

1. Capture AI-agent insights and convert durable notes into **Working Principles**.
2. Provide the sole canonical index for active `design_docs` documentation and current project guidance.
3. Stay synchronized with current `design_docs/` contents and reflect the latest active documentation state.

## Required Reading Order

1. [DOC_README.md](DOC_README.md)
2. [DOC_POLICY.md](DOC_POLICY.md)
3. [PROJECT_DESCRIPTION.md](PROJECT_DESCRIPTION.md)
4. [TERMINOLOGY.md](TERMINOLOGY.md)
5. [graphshell_docs/](graphshell_docs/) — core app
6. [verso_docs/](verso_docs/) — Verso mod (web rendering + bilateral sync)
7. [verse_docs/](verse_docs/) — Verse mod (Tier 2 community network)
8. [nostr_docs/](nostr_docs/) — NostrCore mod
9. [matrix_docs/](matrix_docs/) — MatrixCore mod
10. [graphshell_docs/comms/](graphshell_docs/comms/) — optional hosted communication surfaces inside Graphshell hosting
11. [archive_docs/](archive_docs/)

## Working Principles

- Verify claims against the code/docs before repeating them as facts.
- Keep active docs in the appropriate mod directory (`graphshell_docs/`, `verso_docs/`, `verse_docs/`, `nostr_docs/`, `matrix_docs/`), with hosted communication-surface docs under `graphshell_docs/comms/`; move superseded material to `archive_docs/` checkpoints.
- Prefer updating an existing doc over creating a new one unless the scope clearly requires a new category/resource.
- Do not edit `PROJECT_DESCRIPTION.md` unless explicitly requested.
- Keep this index aligned with folder structure and status in the same session as any doc changes.
- If an AI note/memory adds a durable project principle, record it here under Working Principles.
- If another index conflicts with this file, this file is authoritative and the others should be aligned.
- Keep scaffold markers (`[SCAFFOLD:<id>]`) synchronized with the scaffold registry when integration status changes.
- Migration Strategy: Iterative Replacement
- - Since there are no active users, we prioritize **code cleanliness** over backward compatibility. We will replace subsystems directly rather than maintaining parallel legacy paths.
- When designing a new feature, ask:
- - Is the way you want this system to work consistent with our architectural guarantees (modularity, parallelization, access through intents and not direct state mutation, componentization as opposed to consolidation into monolithic core files, centralization of testing + diagnostic threading to automate testing)?
- - How can we refine the integration to meet our feature goals but respect our architecture?
- **Implementation feedback loop** (DOC_POLICY §13): every implementation is also a design probe. After each implementation pass, disseminate structural learnings to the relevant plans/docs in the same session. If a carrier model, API surface, or data shape changes, check which downstream plans depend on the old shape and add dependency notes or blocking guards before the next dependent step proceeds. It is acceptable for a plan to describe something not yet fully implemented — it is not acceptable for a plan to be silent about an architectural problem visible in code.
- For smolweb and middlenet content, prefer faithful protocol rendering plus optional assistive enrichment. Do not silently erase source protocol semantics in the name of convenience or polish.
- For smolweb expansion, prioritize browser-maturity capabilities such as trust UX, subscription/source health, source/page tools, retention boundaries, and wayfinding before widening protocol surface area for its own sake.

## Design Docs Index

Last updated: April 9, 2026
Project status source: [../README.md](../README.md)

## Root Documents

- [DOC_POLICY.md](DOC_POLICY.md) - Documentation policy and lifecycle rules.
- [PROJECT_DESCRIPTION.md](PROJECT_DESCRIPTION.md) - Maintainer-owned product vision and long-term scope.
- [DOC_README.md](DOC_README.md) - AI context and canonical documentation index.
- [TERMINOLOGY.md](TERMINOLOGY.md) - Canonical project terminology and definitions.

## Graphshell Active Docs

### Graphshell Research

- [graphshell_docs/research/2026-02-18_graph_ux_research_report.md](graphshell_docs/research/2026-02-18_graph_ux_research_report.md) - Graph UI/UX research and synthesis.
- [graphshell_docs/research/2026-02-20_edge_traversal_model_research.md](graphshell_docs/research/2026-02-20_edge_traversal_model_research.md) - Edge traversal model research.
- [graphshell_docs/research/2026-02-24_visual_tombstones_research.md](graphshell_docs/research/2026-02-24_visual_tombstones_research.md) - Research backlog for Ghost Nodes (deleted-but-preserved nodes; formerly "visual tombstones").
- [graphshell_docs/research/2026-02-24_interaction_and_semantic_design_schemes.md](graphshell_docs/research/2026-02-24_interaction_and_semantic_design_schemes.md) - Research on interaction patterns, physics-as-semantics, and lens-based UX.
- [graphshell_docs/research/2026-02-24_diagnostics_research.md](graphshell_docs/research/2026-02-24_diagnostics_research.md) - Diagnostics system research: three-registry model (ChannelRegistry/AnalyzerRegistry/TestRegistry), probe vs. analyzer vs. test classification, current gaps, pane improvements, and priority order.
- [graphshell_docs/research/2026-02-27_viewer_state_matrix.md](graphshell_docs/research/2026-02-27_viewer_state_matrix.md) - Declared vs runtime-wired vs actually-rendered viewer matrix for migration planning.
- [graphshell_docs/research/2026-02-27_all_docs_context_bootstrap.md](graphshell_docs/research/2026-02-27_all_docs_context_bootstrap.md) - High-signal AI bootstrap summary of active+archive documentation context, priorities, invariants, and execution guardrails.
- [graphshell_docs/research/STANDALONE_EXTRACTION.md](graphshell_docs/research/STANDALONE_EXTRACTION.md) - Standalone extraction notes.
- [graphshell_docs/research/2026-03-01_webrender_wgpu_renderer_research.md](graphshell_docs/research/2026-03-01_webrender_wgpu_renderer_research.md) - WebRender wgpu renderer: spec, feasibility, QA strategy, upstreaming model, and upstream community state (byo-renderer, Mozilla wgpu-hal direction, GLSL→SPIR-V recommendation). Closes the technical definition of `#180` and feeds `#183` readiness gates.
- [graphshell_docs/research/2026-03-01_servo_script_engine_alternatives.md](graphshell_docs/research/2026-03-01_servo_script_engine_alternatives.md) - Long-horizon research: Nova (Rust JS engine) + Cranelift JIT as a mozjs/SpiderMonkey replacement in Servo. Covers mozjs architecture, Nova DOD design, JIT options, ohim/Wasm plugin track, Servo AI policy, and scope comparison vs. WebRender wgpu work.
- [graphshell_docs/research/2026-03-02_ux_integration_research.md](graphshell_docs/research/2026-03-02_ux_integration_research.md) - UX integration research: file tree + tile tree + UX tree. Seven-area gap analysis (IA, interaction semantics, predictability, discoverability, feedback/recovery, accessibility, density/overflow), priority matrix, five deliverable specs (Command Semantics Matrix, Interaction Contract, Surface Behavior Spec, Accessibility Baseline Checklist, UX Telemetry Plan).
- [graphshell_docs/research/2026-03-04_standards_alignment_report.md](graphshell_docs/research/2026-03-04_standards_alignment_report.md) - **Canonical standards adoption register.** Maps adopted vs. referenced-only external standards to every Graphshell domain. Resolves contradictions (ActivityPub vs. W3C VC/DID, RFC 6902 vs. CRDTs, rkyv vs. dag-cbor for Verse wire format, UUID v4/v7 namespace split, WCAG 2.2 vs. EN 301 549). All subsystem specs cite their adopted standards from this document. Read this before designing any new subsystem or Verse protocol.
- [graphshell_docs/research/2026-03-27_ambient_graph_visual_effects.md](graphshell_docs/research/2026-03-27_ambient_graph_visual_effects.md) - Ambient canvas visual effects: temporal decay, graphlet halos, rhythm/pulse, warm-node particle emission, tidal influence, edge tension arcs. Default-on/off split, configurability model, and open design questions.
- [graphshell_docs/research/2026-03-29_graph_interaction_brainstorm.md](graphshell_docs/research/2026-03-29_graph_interaction_brainstorm.md) - Validated graph interaction ideas: gravity wells, reading river, knowledge decay, constellation templates, merge detection, breadcrumb trails, orbital ego graphlets, portal nodes, filesystem projection, git-like branching, variable node size, citation overlap, sonification.
- [graphshell_docs/research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md](graphshell_docs/research/2026-04-09_smolweb_graph_enrichment_and_accessibility_note.md) - Smolweb opportunity note covering Bubble, CAPCOM, Antenna, Cosmos, Spacewalk, GUS, IRC, Wander, `history-tree`, irchiver, HTML vs Markdown, and the "faithful render plus optional assistive enrichment" recommendation.
- [graphshell_docs/research/2026-04-09_smolweb_browser_capability_gaps.md](graphshell_docs/research/2026-04-09_smolweb_browser_capability_gaps.md) - Capability-gap note for maturing Graphshell as a smolweb browser: trust/certificate UX, subscription and source health, source/page tools, discovery separation, wayfinding surfaces, retention boundaries, hosted comms, mutation/publication loops, explainable routing, and host-aware degradation.
- [graphshell_docs/research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md](graphshell_docs/research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md) - UI framework comparison (Makepad, iced, xilem, gpui, slint, custom), wgpu-gui-bridge demo findings, crate recommendations (vello as bridge), and discovery of GraphTree as graphlet-native tile tree replacement.

### Graphshell Technical Architecture

- [graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md](graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md) - Current architecture and component boundaries.
- [graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md](graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) - Browser semantics and behavioral model; universal content viewer (MIME detection, ViewerRegistry selection, non-web renderers, tags/badges, UDC semantic physics).
- [verso_docs/technical_architecture/VERSO_AS_PEER.md](verso_docs/technical_architecture/VERSO_AS_PEER.md) - Verso mod: web capability (Servo + wry viewers, protocol handlers) and Verso peer agent (Ed25519 identity, SyncWorker, pairing, graph/workbench context sharing).
- [graphshell_docs/technical_architecture/codebase_guide.md](graphshell_docs/technical_architecture/codebase_guide.md) - Active module-orientation guide and debugging entry points for reducer/workbench/render boundaries.
- [graphshell_docs/technical_architecture/BUILD.md](graphshell_docs/technical_architecture/BUILD.md) - Build instructions and dependency notes.
- [graphshell_docs/technical_architecture/QUICKSTART.md](graphshell_docs/technical_architecture/QUICKSTART.md) - Fast-start command reference.
- [graphshell_docs/technical_architecture/unified_view_model.md](graphshell_docs/technical_architecture/unified_view_model.md) - Canonical five-domain architecture model: Shell host, Graph truth, Navigator projection, Workbench arrangement, Viewer realization, and graph-bearing surfaces.
- [graphshell_docs/technical_architecture/graphlet_model.md](graphshell_docs/technical_architecture/graphlet_model.md) - Canonical graphlet semantics: derived vs pinned graphlets, graphlet shapes, ownership split, and UI expressions.
- [graphshell_docs/technical_architecture/graph_tree_spec.md](graphshell_docs/technical_architecture/graph_tree_spec.md) - GraphTree crate API design: framework-agnostic graphlet-native tile tree replacing egui_tiles, collapsing Navigator/Workbench projection gap via ProjectionLens, taffy layout, UxTree integration.
- [graphshell_docs/technical_architecture/domain_interaction_scenarios.md](graphshell_docs/technical_architecture/domain_interaction_scenarios.md) - End-to-end scenarios showing how the five domains collaborate in concrete user flows.
- [graphshell_docs/technical_architecture/2026-02-18_universal_node_content_model.md](graphshell_docs/technical_architecture/2026-02-18_universal_node_content_model.md) - Universal node content model vision.
- [graphshell_docs/technical_architecture/2026-02-27_presentation_provider_and_ai_orchestration.md](graphshell_docs/technical_architecture/2026-02-27_presentation_provider_and_ai_orchestration.md) - Provider capability contract, node facet taxonomy, and tiered AI orchestration (tiny local model + retrieval + optional large-model escalation).
- [graphshell_docs/technical_architecture/2026-03-01_dependency_inventory.md](graphshell_docs/technical_architecture/2026-03-01_dependency_inventory.md) - Full direct-dependency inventory: active, transitional (wgpu migration drops), pre-staged (15 unused reserved deps), build-only, and platform-specific. Includes pre-staged→planned-feature mapping and wgpu migration group summary.

### Graphshell Implementation Strategy

- [graphshell_docs/implementation_strategy/PLANNING_REGISTER.md](graphshell_docs/implementation_strategy/PLANNING_REGISTER.md) - **Canonical execution register**: active lane sequencing, stabilization bug register, issue-seeding guidance, and subsystem/lane prioritization.
- [graphshell_docs/implementation_strategy/domain_interaction_acceptance_matrix.md](graphshell_docs/implementation_strategy/domain_interaction_acceptance_matrix.md) - Compact PR-review acceptance matrix for cross-domain scenario IDs and evidence expectations.
- [graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md](graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md) - Consolidated UX execution control-plane: baseline done-gate, current milestone checklist, and issue-domain map.
- [graphshell_docs/implementation_strategy/2026-02-28_ux_contract_register.md](graphshell_docs/implementation_strategy/2026-02-28_ux_contract_register.md) - Cross-spec UX ownership register and contract map.
- [graphshell_docs/implementation_strategy/2026-03-01_ux_migration_lifecycle_audit_register.md](graphshell_docs/implementation_strategy/2026-03-01_ux_migration_lifecycle_audit_register.md) - UX migration lifecycle register: current/planned/speculative audit with pre/post renderer/WGPU and networking timing gates plus UxTree automation readiness.
- [graphshell_docs/implementation_strategy/2026-03-01_complete_feature_inventory.md](graphshell_docs/implementation_strategy/2026-03-01_complete_feature_inventory.md) - Complete cross-doc feature inventory with implemented/planned/speculative status and WGPU migration issue categorization.
- [graphshell_docs/implementation_strategy/2026-03-02_scaffold_registry.md](graphshell_docs/implementation_strategy/2026-03-02_scaffold_registry.md) - Canonical machine-readable scaffold inventory (`[SCAFFOLD:<id>]`) and closure criteria.
- [graphshell_docs/implementation_strategy/subsystem_storage/2026-03-11_graphstore_vs_client_storage_manager_note.md](graphshell_docs/implementation_strategy/subsystem_storage/2026-03-11_graphstore_vs_client_storage_manager_note.md) - Short architecture note separating Graphshell app durability (`GraphStore`) from future WHATWG-style browser-origin storage coordination (`ClientStorageManager`), with a Rust trait sketch for pluggable backends.
- [graphshell_docs/implementation_strategy/subsystem_storage/2026-03-11_client_storage_manager_implementation_plan.md](graphshell_docs/implementation_strategy/subsystem_storage/2026-03-11_client_storage_manager_implementation_plan.md) - Phased P0–P5 execution plan for a future Servo-compatible `ClientStorageManager`: runtime seam, metadata loading, bucket lifecycle, endpoint registration, async deletion, and session/private-scope closure.
- [graphshell_docs/implementation_strategy/viewer/2026-03-02_filesystem_ingest_graph_mapping_plan.md](graphshell_docs/implementation_strategy/viewer/2026-03-02_filesystem_ingest_graph_mapping_plan.md) - Filesystem ingest feature plan with viewer-readiness gate, files→nodes / folders→frames mapping, and phased acceptance criteria.
- [graphshell_docs/implementation_strategy/viewer/2026-04-03_clipping_viewer_follow_on_plan.md](graphshell_docs/implementation_strategy/viewer/2026-04-03_clipping_viewer_follow_on_plan.md) - Active follow-on viewer plan for remaining clipping work after the landed 2026-02-11 clipping slice was archived.
- [graphshell_docs/implementation_strategy/viewer/2026-03-08_servo_text_editor_architecture_plan.md](graphshell_docs/implementation_strategy/viewer/2026-03-08_servo_text_editor_architecture_plan.md) - Servo-backed text editor architecture: `editor-core` (WASM-clean) + Servo surface split, IME composition contract, crate layout, phases, and acceptance criteria.
- [graphshell_docs/implementation_strategy/viewer/2026-03-08_simple_document_engine_target_spec.md](graphshell_docs/implementation_strategy/viewer/2026-03-08_simple_document_engine_target_spec.md) - Canonical `SimpleDocument` / `EngineTarget` / `RenderPolicy` spec for the Servo-first content adaptation pipeline (UCM Steps 11–12). Prerequisite for Gemini resolver, Reader Mode, and markdown pipeline.
- [graphshell_docs/implementation_strategy/viewer/universal_content_model_spec.md](graphshell_docs/implementation_strategy/viewer/universal_content_model_spec.md) - Canonical UCM interaction contract: viewer trait, selection policy, MIME detection, non-web viewer types, FilePermissionGuard, core/host split.
- [graphshell_docs/implementation_strategy/viewer/2026-03-02_unified_source_directory_mapping_plan.md](graphshell_docs/implementation_strategy/viewer/2026-03-02_unified_source_directory_mapping_plan.md) - Unified local/network/web directory-domain auto-mapping plan, gated by filesystem-ingest readiness.
- [graphshell_docs/implementation_strategy/2026-03-11_boa_scripting_engine_plan.md](graphshell_docs/implementation_strategy/2026-03-11_boa_scripting_engine_plan.md) - Boa JS engine as a Graphshell-native scripting layer: graph queries, reactive automation, custom action registration, and JS mod tier. Four-phase plan (read-only queries → event hooks → ES module mods → async runtime).
- [graphshell_docs/implementation_strategy/aspect_render/2026-03-12_compositor_expansion_plan.md](graphshell_docs/implementation_strategy/aspect_render/2026-03-12_compositor_expansion_plan.md) - GL parent-render compositor expansion: content signature enrichment, lifecycle → overlay affordance, lens-driven Pass 3 descriptor, tile activity diagnostics channel, focus delta latching, EmbeddedEgui z-order fix, and generic viewer callback path.
- [graphshell_docs/implementation_strategy/aspect_render/2026-03-01_webrender_wgpu_renderer_implementation_plan.md](graphshell_docs/implementation_strategy/aspect_render/2026-03-01_webrender_wgpu_renderer_implementation_plan.md) - WebRender wgpu renderer implementation plan (P0–P12). The egui-wgpu UI cut has landed; the remaining plan concerns deeper WebRender/runtime bridge convergence.
- [graphshell_docs/implementation_strategy/subsystem_ux_semantics/ux_event_dispatch_spec.md](graphshell_docs/implementation_strategy/subsystem_ux_semantics/ux_event_dispatch_spec.md) - Canonical UxTree event dispatch contract (capture/target/bubble/default, modal isolation, authority routing, diagnostics/test gates).
- [graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md](graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) - Active companion closure lane for Shell command-surface provenance diagnostics, UxTree/probe/scenario modeling, and Shell command-bar / omnibar AT validation.
- [graphshell_docs/implementation_strategy/aspect_command/radial_menu_geometry_and_overflow_spec.md](graphshell_docs/implementation_strategy/aspect_command/radial_menu_geometry_and_overflow_spec.md) - Canonical radial geometry/overflow/readability contract with deterministic ring assignment and CI test expectations.
- [graphshell_docs/implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md](graphshell_docs/implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md) - Canonical interaction contract for the workbench/frame/tile model.
- [graphshell_docs/implementation_strategy/workbench/WORKBENCH.md](graphshell_docs/implementation_strategy/workbench/WORKBENCH.md) - Workbench domain spec: arrangement and activation authority within the five-domain model.
- [graphshell_docs/implementation_strategy/workbench/workbench_backlog_pack.md](graphshell_docs/implementation_strategy/workbench/workbench_backlog_pack.md) - Workbench execution backlog including cross-domain scenario-track IDs.
- [graphshell_docs/implementation_strategy/workbench/graphlet_projection_binding_spec.md](graphshell_docs/implementation_strategy/workbench/graphlet_projection_binding_spec.md) - Workbench-specific binding model for linked vs detached graphlet arrangements.
- [graphshell_docs/implementation_strategy/workbench/2026-04-10_graph_tree_implementation_plan.md](graphshell_docs/implementation_strategy/workbench/2026-04-10_graph_tree_implementation_plan.md) - GraphTree implementation plan: phased migration from egui_tiles, Navigator projection collapse, arrangement-edge consumption, 7-phase execution with code impact map and rollup of absorbed Navigator backlog items.
- [graphshell_docs/implementation_strategy/workbench/2026-03-20_arrangement_graph_projection_plan.md](graphshell_docs/implementation_strategy/workbench/2026-03-20_arrangement_graph_projection_plan.md) - Plan to make tile tree a projection of arrangement graph truth: HostedSurface bridge object, ArrangementRelation sub-kinds (tab-order-next, group-member, split-child, etc.), Navigator faithfulness contract, workbench invocation via arrangement mutations, and 5-phase migration plan.
- [graphshell_docs/implementation_strategy/workbench/pane_presentation_and_locking_spec.md](graphshell_docs/implementation_strategy/workbench/pane_presentation_and_locking_spec.md) - Canonical contract for tiled/docked presentation and `PaneLock` behavior.
- [graphshell_docs/implementation_strategy/workbench/workbench_layout_policy_spec.md](graphshell_docs/implementation_strategy/workbench/workbench_layout_policy_spec.md) - Semantic layout policy: `WorkbenchLayoutConstraint` (role-keyed anchor splits), `UxConfigMode` (per-surface unlock/configure/lock flow), `SurfaceFirstUsePolicy` (first-use preference prompts), and `WorkbenchLayoutPolicyEvaluator` (pure `(UxTreeSnapshot, WorkbenchProfile) → Vec<WorkbenchIntent>`).
- [graphshell_docs/implementation_strategy/navigator/NAVIGATOR.md](graphshell_docs/implementation_strategy/navigator/NAVIGATOR.md) - Navigator domain spec: graphlet derivation, scoped search, specialty navigation layouts, and projection authority.
- [graphshell_docs/implementation_strategy/navigator/navigator_backlog_pack.md](graphshell_docs/implementation_strategy/navigator/navigator_backlog_pack.md) - Navigator execution backlog including cross-domain scenario-track IDs.
- [graphshell_docs/implementation_strategy/navigator/navigator_interaction_contract.md](graphshell_docs/implementation_strategy/navigator/navigator_interaction_contract.md) - Canonical Navigator click grammar and node-vs-structural row interaction rules.
- [graphshell_docs/implementation_strategy/shell/SHELL.md](graphshell_docs/implementation_strategy/shell/SHELL.md) - Shell domain spec: Graphshell's only host and app-level orchestration boundary.
- [graphshell_docs/implementation_strategy/shell/2026-04-03_shell_command_bar_execution_plan.md](graphshell_docs/implementation_strategy/shell/2026-04-03_shell_command_bar_execution_plan.md) - Active Workstream A closure lane for Shell command-bar authority, omnibar session/mailbox state, focused-target routing, and legacy command-path cleanup.
- [graphshell_docs/implementation_strategy/shell/shell_backlog_pack.md](graphshell_docs/implementation_strategy/shell/shell_backlog_pack.md) - Shell execution backlog including overview, routing, and interruption scenario-track IDs.
- [graphshell_docs/implementation_strategy/shell/shell_overview_surface_spec.md](graphshell_docs/implementation_strategy/shell/shell_overview_surface_spec.md) - Concrete Shell overview surface for graph/workbench/runtime summary and cross-domain routing.
- [graphshell_docs/implementation_strategy/graph/GRAPH.md](graphshell_docs/implementation_strategy/graph/GRAPH.md) - Graph domain spec; the canvas is its primary rendered surface, not the domain name.
- [graphshell_docs/implementation_strategy/graph/node_badge_and_tagging_spec.md](graphshell_docs/implementation_strategy/graph/node_badge_and_tagging_spec.md) - Canonical interaction contract for node badges, tag assignment surface behavior, icon resources, and Knowledge Registry integration.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_node_glyph_spec.md](graphshell_docs/implementation_strategy/graph/2026-04-03_node_glyph_spec.md) - Canonical node glyph spec: visual form of a node on the canvas, resolved via rule-matching pipeline from node data through theme application. Owns body, content imagery, LOD presentation, and user-authored glyph rules. Orthogonal to PMEST facets.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_physics_preferences_surface_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_physics_preferences_surface_plan.md) - Follow-on execution lane for the page-backed physics settings surface: scope-aware controls, preset portability, advanced overrides, and dependency triage for graph/view-local physics preferences.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_physics_region_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_physics_region_plan.md) - Follow-on execution lane for authored physics regions: spatial-rule objects distinct from frame-affinity, with explicit creation, overlap, scope, persistence, and divergent-view commit semantics.
- [graphshell_docs/implementation_strategy/graph/graph_node_edge_interaction_spec.md](graphshell_docs/implementation_strategy/graph/graph_node_edge_interaction_spec.md) - Canonical interaction contract for graph, node, edge, and camera semantics on graph-bearing surfaces including the canvas.
- [graphshell_docs/implementation_strategy/graph/2026-03-21_edge_family_and_provenance_expansion_plan.md](graphshell_docs/implementation_strategy/graph/2026-03-21_edge_family_and_provenance_expansion_plan.md) - Edge vocabulary expansion plan: keeps the current five relation families, adds a dedicated Provenance family, and proposes broader semantic/traversal/containment/arrangement/imported sub-kind vocabularies for prototype-era knowledge capture.
- [graphshell_docs/implementation_strategy/graph/2026-03-21_edge_payload_type_sketch.md](graphshell_docs/implementation_strategy/graph/2026-03-21_edge_payload_type_sketch.md) - Rust-facing graph-model sketch for the next `EdgePayload`: split family from sub-kind, move traversal onto an explicit event carrier, and add typed Imported/Provenance sidecars.
- [graphshell_docs/implementation_strategy/graph/2026-03-11_graph_enrichment_plan.md](graphshell_docs/implementation_strategy/graph/2026-03-11_graph_enrichment_plan.md) - Umbrella graph-enrichment plan unifying tags, badges, UDC classification, import/clip enrichment, provenance, and visible graph effects under the knowledge-capture lane.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_layout_backend_state_ownership_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_layout_backend_state_ownership_plan.md) - Follow-on execution lane for widening the persisted layout carrier, stabilizing external layout IDs, and defining when Graphshell should move past the current upstream layout-state seam.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_damping_profile_follow_on_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_damping_profile_follow_on_plan.md) - Follow-on execution lane for damping profiles: named curve identities, deterministic registry lookup, explicit settle-shape policy, and profile-facing diagnostics.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_edge_routing_follow_on_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_edge_routing_follow_on_plan.md) - Follow-on execution lane for edge routing: post-layout path policy, readability-driven suggestions, low-cost first-slice routing, and bounded bundling strategy.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_semantic_clustering_follow_on_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_semantic_clustering_follow_on_plan.md) - Follow-on execution lane for semantic clustering: attributable semantic inputs, out-of-band cluster computation, explicit layout consumption, and user-visible explanation.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_layout_transition_and_history_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_layout_transition_and_history_plan.md) - Follow-on execution lane for layout morphing, bounded position snapshots, and view-owned spatial undo/redo that stays separate from graph-topology history.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_layout_variant_follow_on_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_layout_variant_follow_on_plan.md) - Follow-on execution lane for built-in layout variants beyond FR/Barnes-Hut: widened state carrier, analytic first wave, portfolio integration, and second-wave admission bar.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_wasm_layout_runtime_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_wasm_layout_runtime_plan.md) - Follow-on execution lane for sandboxed runtime-loaded layouts: host adapter, guest ABI, fallback behavior, and snapshot-safe degradation.
- [graphshell_docs/implementation_strategy/graph/2026-04-03_twod_twopointfive_isometric_plan.md](graphshell_docs/implementation_strategy/graph/2026-04-03_twod_twopointfive_isometric_plan.md) - Narrowed execution lane for `TwoD`, `TwoPointFive`, and `Isometric` projection modes that preserve 2D layout truth without assuming full free-camera 3D.
- [graphshell_docs/implementation_strategy/graph/faceted_filter_surface_spec.md](graphshell_docs/implementation_strategy/graph/faceted_filter_surface_spec.md) - Canonical faceted-filter contract: PMEST schema, operator semantics, Lens integration, omnibar/palette parity, and diagnostics/test gates.
- [graphshell_docs/implementation_strategy/graph/facet_pane_routing_spec.md](graphshell_docs/implementation_strategy/graph/facet_pane_routing_spec.md) - Canonical facet-rail routing contract: single-node facet navigation, Enter-to-pane destination resolution, focus return, and UxTree exposure.
- [graphshell_docs/implementation_strategy/aspect_command/command_surface_interaction_spec.md](graphshell_docs/implementation_strategy/aspect_command/command_surface_interaction_spec.md) - Canonical interaction contract for command surfaces and unified action invocation.
- [graphshell_docs/implementation_strategy/aspect_input/input_interaction_spec.md](graphshell_docs/implementation_strategy/aspect_input/input_interaction_spec.md) - Canonical interaction contract for hardware input routing: context stack, binding resolution, chord/sequence recognition, remapping, cross-surface routing, and gamepad input.
- [graphshell_docs/implementation_strategy/aspect_control/2026-03-02_graphshell_profile_registry_spec.md](graphshell_docs/implementation_strategy/aspect_control/2026-03-02_graphshell_profile_registry_spec.md) - General persisted `GraphshellProfile` contract (default profile, user-created profiles, persistence/migration/diagnostics).
- [graphshell_docs/implementation_strategy/aspect_render/render_backend_contract_spec.md](graphshell_docs/implementation_strategy/aspect_render/render_backend_contract_spec.md) - Canonical backend abstraction contract: bridge mode enum, capability probe, Glow GL state isolation, wgpu texture handoff, fallback routing, Glow retirement conditions, and feature guardrails.
- [graphshell_docs/implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md](graphshell_docs/implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md) - Canonical focus authority and region navigation contract.
- [graphshell_docs/implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md](graphshell_docs/implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md) - Canonical viewer presentation, fallback, and degraded-state contract.
- [graphshell_docs/implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md](graphshell_docs/implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md) - Canonical lifecycle/reconcile contract for `Active/Warm/Cold/Tombstone` and `RuntimeBlocked` behavior.
- [graphshell_docs/implementation_strategy/viewer/webview_lifecycle_and_crash_recovery_spec.md](graphshell_docs/implementation_strategy/viewer/webview_lifecycle_and_crash_recovery_spec.md) - Canonical runtime contract for webview attach/detach/crash/recovery behavior.
- [graphshell_docs/implementation_strategy/aspect_control/settings_and_control_surfaces_spec.md](graphshell_docs/implementation_strategy/aspect_control/settings_and_control_surfaces_spec.md) - Canonical settings/history/control-surface contract.
- [graphshell_docs/implementation_strategy/system/system_architecture_spec.md](graphshell_docs/implementation_strategy/system/system_architecture_spec.md) - Top-level system architecture and layer decomposition.
- [graphshell_docs/implementation_strategy/system/register_layer_spec.md](graphshell_docs/implementation_strategy/system/register_layer_spec.md) - Register layer as a system component.
- [graphshell_docs/implementation_strategy/system/registry_runtime_spec.md](graphshell_docs/implementation_strategy/system/registry_runtime_spec.md) - `RegistryRuntime` composition-root contract.
- [graphshell_docs/implementation_strategy/system/control_panel_spec.md](graphshell_docs/implementation_strategy/system/control_panel_spec.md) - `ControlPanel` async coordination contract.
- [graphshell_docs/implementation_strategy/system/signal_bus_spec.md](graphshell_docs/implementation_strategy/system/signal_bus_spec.md) - `SignalBus` / signal-routing contract.
- [graphshell_docs/implementation_strategy/system/register/SYSTEM_REGISTER.md](graphshell_docs/implementation_strategy/system/register/SYSTEM_REGISTER.md) - Register hub/index and historical implementation guide.
- [graphshell_docs/implementation_strategy/system/2026-03-05_cp4_p2p_sync_plan.md](graphshell_docs/implementation_strategy/system/2026-03-05_cp4_p2p_sync_plan.md) - CP4 ControlPanel integration plan: `p2p_sync_worker` supervision, `ApplyRemoteDelta`/`MarkPeerOffline` intent variants, version vector persistence, reducer handling, and done gates.
- [graphshell_docs/implementation_strategy/system/2026-03-05_network_architecture.md](graphshell_docs/implementation_strategy/system/2026-03-05_network_architecture.md) - Network protocol layer assignments: iroh (Coop/Device Sync transport), libp2p (Verse swarm), Nostr (identity/social/event bus). Covers public profile/follows, DMs, relay posture, Blossom, NIP-72/29 as Verse primitives, Nostr mod plugin surface, and iroh/libp2p/Nostr interoperability notes.
- [graphshell_docs/implementation_strategy/system/2026-03-06_foundational_reset_implementation_plan.md](graphshell_docs/implementation_strategy/system/2026-03-06_foundational_reset_implementation_plan.md) - Active foundational reset execution plan. Tracks landed CLAT progress, remaining state-layer work, command/planner follow-ons, and verification gates.
- [graphshell_docs/implementation_strategy/system/2026-03-06_reducer_only_mutation_enforcement_plan.md](graphshell_docs/implementation_strategy/system/2026-03-06_reducer_only_mutation_enforcement_plan.md) - Migration plan from trusted-writer boundary to compiler-enforced reducer-only graph mutation, including `GraphMutation` staging, replay alignment, side-effect isolation, and acceptance gates.
- [graphshell_docs/implementation_strategy/system/register/protocol_registry_spec.md](graphshell_docs/implementation_strategy/system/register/protocol_registry_spec.md) - Registry spec family starts here; protocol resolution and handler floor.
- [graphshell_docs/implementation_strategy/subsystem_mods/SUBSYSTEM_MODS.md](graphshell_docs/implementation_strategy/subsystem_mods/SUBSYSTEM_MODS.md) - Canonical Mods subsystem policy authority: built-in/native/WASM split, capability boundaries, and lifecycle ownership.
- [graphshell_docs/implementation_strategy/subsystem_mods/mod_lifecycle_integrity_spec.md](graphshell_docs/implementation_strategy/subsystem_mods/mod_lifecycle_integrity_spec.md) - Canonical lifecycle-integrity contract for mod admission, activation ordering, rollback/quarantine, unload, and diagnostics obligations across native and future WASM mods.
- [nostr_docs/implementation_strategy/nostr_core_registry_spec.md](nostr_docs/implementation_strategy/nostr_core_registry_spec.md) - Canonical `NostrCore` provider profile: capability IDs, diagnostics channel descriptors, and initial native `ModManifest` shape. *(moved to nostr_docs)*
- [nostr_docs/implementation_strategy/2026-03-10_nostr_nip_completion_plan.md](nostr_docs/implementation_strategy/2026-03-10_nostr_nip_completion_plan.md) - Phased plan for NIP-19/21/02/05/11/25/51/65/17 coverage; dependency order and crate strategy for completing the Nostr client layer. *(moved to nostr_docs)*
- [graphshell_docs/implementation_strategy/workbench/](graphshell_docs/implementation_strategy/workbench/) - Workbench specs and workbench-specific plans.
- [graphshell_docs/implementation_strategy/graph/](graphshell_docs/implementation_strategy/graph/) - Graph domain specs and graph-specific plans; use `canvas` for the rendered surface terminology within those docs.
- [graphshell_docs/implementation_strategy/viewer/](graphshell_docs/implementation_strategy/viewer/) - Viewer specs and viewer/backend plans.
- [graphshell_docs/implementation_strategy/system/](graphshell_docs/implementation_strategy/system/) - System-level specs, registry architecture, and register component docs.
- [graphshell_docs/implementation_strategy/subsystem_history/](graphshell_docs/implementation_strategy/subsystem_history/) - History subsystem specs: traversal, node navigation, node audit, temporal replay, unified architecture plan, and mixed-timeline contract.
- [graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_bookmarks_import_plan.md](graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_bookmarks_import_plan.md) - Current bookmark-import plan: imported provenance/import-record integration, bookmark-folder semantics, and ActionRegistry-backed command entry.
- [graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_browser_history_import_plan.md](graphshell_docs/implementation_strategy/subsystem_history/2026-04-02_browser_history_import_plan.md) - Current browser-history import plan: imported-data seeding without violating live traversal/history authority.
- [graphshell_docs/implementation_strategy/social/2026-04-12_host_abstractions_plan.md](graphshell_docs/implementation_strategy/social/2026-04-12_host_abstractions_plan.md) - Protocol-Agnostic P2P Graph Views & Host Abstractions Plan: transition from unified semantic protocol to host-first browser (Identity Ring, Git-Like Persistence Facade, Verso canvas sync).

### Graphshell Design

- [graphshell_docs/design/KEYBINDINGS.md](graphshell_docs/design/KEYBINDINGS.md) - Keyboard interaction reference.
- [graphshell_docs/design/command_semantics_matrix.md](graphshell_docs/design/command_semantics_matrix.md) - Canonical D1 command semantics matrix across keyboard, palette, radial, toolbar, and omnibar surfaces.
- [graphshell_docs/design/2026-03-10_command_keybinding_accounting_plan.md](graphshell_docs/design/2026-03-10_command_keybinding_accounting_plan.md) - Gap analysis and phased plan for aligning command matrix, keybinding docs, and ActionRegistry; establishes C1–C4 usability work sequence post-Sector-H/G.
- [graphshell_docs/design/surface_behavior_spec.md](graphshell_docs/design/surface_behavior_spec.md) - Canonical D3 surface behavior policy for scroll, overflow, resize, empty/loading/error states, and floating lifecycle.
- [graphshell_docs/design/accessibility_baseline_checklist.md](graphshell_docs/design/accessibility_baseline_checklist.md) - Canonical D4 WCAG 2.2 A/AA checklist by surface, with initial screen-reader test matrix.
- [graphshell_docs/design/ux_telemetry_plan.md](graphshell_docs/design/ux_telemetry_plan.md) - Canonical D5 UX telemetry metric register with diagnostics/probe mapping and baseline targets.
- [graphshell_docs/testing/2026-03-02_accessibility_closure_bundle_audit_301.md](graphshell_docs/testing/2026-03-02_accessibility_closure_bundle_audit_301.md) - `#301` closure evidence for reduced-motion guardrails, contrast/target-size audits, and keyboard-trap validation.
- [graphshell_docs/testing/2026-03-02_graph_canvas_keyboard_focus_audit_298.md](graphshell_docs/testing/2026-03-02_graph_canvas_keyboard_focus_audit_298.md) - `#298` closure evidence for deterministic graph keyboard traversal and graph canvas accessibility naming baseline.

### Graphshell Testing

- [graphshell_docs/testing/test_guide.md](graphshell_docs/testing/test_guide.md) - Active testing entry guide covering baseline commands, scope rules, and acceptance checks.
- [graphshell_docs/testing/scenario_back_forward_burst.html](graphshell_docs/testing/scenario_back_forward_burst.html) - Navigation scenario fixture.
- [graphshell_docs/testing/scenario_hash_change.html](graphshell_docs/testing/scenario_hash_change.html) - Hash change scenario fixture.
- [graphshell_docs/testing/scenario_spa_pushstate.html](graphshell_docs/testing/scenario_spa_pushstate.html) - SPA pushState fixture.
- [graphshell_docs/testing/scenario_window_child.html](graphshell_docs/testing/scenario_window_child.html) - Child window scenario fixture.
- [graphshell_docs/testing/scenario_window_open.html](graphshell_docs/testing/scenario_window_open.html) - window.open scenario fixture.
- [graphshell_docs/testing/delegate_trace_back_forward_burst_http.log](graphshell_docs/testing/delegate_trace_back_forward_burst_http.log) - Delegate trace log.
- [graphshell_docs/testing/delegate_trace_hash_change_http.log](graphshell_docs/testing/delegate_trace_hash_change_http.log) - Delegate trace log.
- [graphshell_docs/testing/delegate_trace_hash_change.log](graphshell_docs/testing/delegate_trace_hash_change.log) - Delegate trace log.
- [graphshell_docs/testing/delegate_trace_redirect.log](graphshell_docs/testing/delegate_trace_redirect.log) - Delegate trace log.
- [graphshell_docs/testing/delegate_trace_spa_pushstate_http.log](graphshell_docs/testing/delegate_trace_spa_pushstate_http.log) - Delegate trace log.
- [graphshell_docs/testing/delegate_trace_spa_pushstate.log](graphshell_docs/testing/delegate_trace_spa_pushstate.log) - Delegate trace log.
- [graphshell_docs/testing/delegate_trace_window_open_http.log](graphshell_docs/testing/delegate_trace_window_open_http.log) - Delegate trace log.

## Verso Active Docs

Verso mod: web rendering (Servo + Wry) and bilateral P2P sync (Tier 1). See `DOC_POLICY.md` for boundary definition.

### Verso Technical Architecture

- [verso_docs/technical_architecture/VERSO_AS_PEER.md](verso_docs/technical_architecture/VERSO_AS_PEER.md) - Verso mod: web capability (Servo + wry viewers, protocol handlers) and Verso peer agent (Ed25519 identity, SyncWorker, pairing, graph/workbench context sharing).
- [verso_docs/technical_architecture/VERSO_SERVO_ARCHITECTURE.md](verso_docs/technical_architecture/VERSO_SERVO_ARCHITECTURE.md) - Verso/Servo integration architecture.

### Verso Implementation Strategy

- [verso_docs/implementation_strategy/2026-02-22_verse_implementation_strategy.md](verso_docs/implementation_strategy/2026-02-22_verse_implementation_strategy.md) - Verso/Verse implementation strategy and Tier 1 / Tier 2 split framing.
- [verso_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](verso_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) - **Canonical Verso Tier 1 sync plan** (iroh transport, sync units, pairing/sync phases, deterministic sync-logic simulator matrix).
- [verso_docs/implementation_strategy/2026-02-25_verse_presence_plan.md](verso_docs/implementation_strategy/2026-02-25_verse_presence_plan.md) - Post-Phase-5 collaborative presence plan: ghost cursors, remote selection, follow mode, and presence stream policy.
- [verso_docs/implementation_strategy/coop_session_spec.md](verso_docs/implementation_strategy/coop_session_spec.md) - Canonical co-op session authority: host-led co-presence, roles, sharing, approval workflow, snapshot, session UI, intent surface, flock model (§14), Nostr identity (§15), and wallet integration (§16).
- [verso_docs/implementation_strategy/2026-03-27_session_capsule_ledger_plan.md](verso_docs/implementation_strategy/2026-03-27_session_capsule_ledger_plan.md) - Session Capsule Ledger: WASM-safe portable session archive format (`SessionCapsule`, `SessionLedger`, `ArchiveReceipt`), CID addressing, AES-256-GCM encryption, UCAN delegation, and Verso bilateral sync integration. 5-slice implementation plan A.1–A.5.
- [verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md](verso_docs/implementation_strategy/2026-03-28_gemini_capsule_server_plan.md) - Small protocol capsule servers (Gemini/Gopher/Finger): serve Graphshell content and personal profiles over TCP. `SimpleDocument` ↔ `text/gemini`/Gophermap/plain-text serializers, content routers, `GraphIntent` wiring for all three protocols.
- [verso_docs/implementation_strategy/2026-03-28_cable_coop_minichat_spec.md](verso_docs/implementation_strategy/2026-03-28_cable_coop_minichat_spec.md) - Cable wire protocol as co-op minichat substrate: identity/cabal-key derivation, post type mapping, moderation integration (host as admin seed + subjective guest layer), in-memory ephemeral store, iroh transport (skip Noise), Comms lane positioning, 4-phase rollout.
- [verso_docs/implementation_strategy/PHASE5_STEP5.1_COMPLETE.md](verso_docs/implementation_strategy/PHASE5_STEP5.1_COMPLETE.md) - Phase 5 Step 5.1 completion record.
- [verso_docs/implementation_strategy/PHASE5_STEP5.2_COMPLETE.md](verso_docs/implementation_strategy/PHASE5_STEP5.2_COMPLETE.md) - Phase 5 Step 5.2 completion record.
- [verso_docs/implementation_strategy/PHASE5_STEP5.3_COMPLETE.md](verso_docs/implementation_strategy/PHASE5_STEP5.3_COMPLETE.md) - Phase 5 Step 5.3 completion record.

### Verso Research

- [verso_docs/research/2026-03-28_permacomputing_alignment.md](verso_docs/research/2026-03-28_permacomputing_alignment.md) - Permacomputing alignment: 10-principle audit, gaps (resource awareness, intentional forgetting, constrained hardware, small-web publishing, content portability), curated project index (Cable, Uxn, Coalescent Computer, Solar Protocol, snac, Cerca), and design posture summary.
- [verso_docs/research/2026-03-28_smolnet_follow_on_audit.md](verso_docs/research/2026-03-28_smolnet_follow_on_audit.md) - Suitability audit for post-Gemini/Gopher/Finger smallnet follow-ons: admission bar for native Verso support, capability-family split (`SimpleDocument` bridge boundary, discovery vs messaging vs document lanes), and recommendations for Titan, Spartan, Misfin, Nex, and Guppy.
- [verso_docs/research/2026-03-28_smolnet_dependency_health_audit.md](verso_docs/research/2026-03-28_smolnet_dependency_health_audit.md) - Dependency-health rubric for follow-on smallnet protocol crates: when to prefer local implementations, when external Rust crates may be justified, and what still requires external ecosystem validation for Titan, Spartan, Misfin, Nex, and Guppy.

---

## Verse Active Docs

Verse mod: public decentralized community network (Tier 2, long-horizon research). Not a Phase 5 dependency.

### Verse Technical Architecture

- [verse_docs/technical_architecture/VERSE_AS_NETWORK.md](verse_docs/technical_architecture/VERSE_AS_NETWORK.md) - Verse as the optional community-scale network layer, with explicit boundary against Verso bilateral sync/co-op and Comms hosted surfaces.
- [verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md](verse_docs/technical_architecture/2026-02-23_verse_tier2_architecture.md) - Long-horizon Tier 2 architecture: dual transport, VerseBlob, FLora, Proof of Access, crawler economy, and open research questions.
- [verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md](verse_docs/technical_architecture/2026-03-05_verse_nostr_dvm_integration.md) - How Nostr (NIP-72 community surface, NIP-90 DVMs), FLora checkpoints, distributed indices, and Proof of Access economics compose: feed curation, context-aware traversal suggestions, graph node summarisation, Lightning/receipt tokenomics, and Tier 2 rollout sequence.
- [verse_docs/technical_architecture/2026-03-05_verse_economic_model.md](verse_docs/technical_architecture/2026-03-05_verse_economic_model.md) - Coherent economic model: no native Verse token (sats for compute, FIL for storage, reputation for governance); storage staking/bonds; sats operational budget; FIL treasury; full browsing→review→hosting→compute→settlement value loop; contributor/reviewer/bootstrap staking types; anti-plutocracy guarantees; open problems.

### Verse Implementation Strategy

- [verse_docs/implementation_strategy/self_hosted_model_spec.md](verse_docs/implementation_strategy/self_hosted_model_spec.md) - Self-hosted model spec: capability contracts, model/engram classification, cooperative multi-model execution, mini-adapter flow, and UI-facing behavior contracts.
- [verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md](verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md) - STM/LTM, MemoryExtractor/MemoryIngestor, engram storage, and intelligence memory plumbing.
- [verse_docs/implementation_strategy/engram_spec.md](verse_docs/implementation_strategy/engram_spec.md) - Canonical `Engram` / `TransferProfile` schema: envelope, memory classes, validation classes, redaction, trust, and FLora submission rules.
- [verse_docs/implementation_strategy/verseblob_content_addressing_spec.md](verse_docs/implementation_strategy/verseblob_content_addressing_spec.md) - Canonical `VerseBlob` schema and content-addressing policy: CID defaults, attachment model, retrieval rules, and safety limits.
- [verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md](verse_docs/implementation_strategy/flora_submission_checkpoint_spec.md) - Canonical FLora flow: engram submission manifests, review, checkpoints, reward hooks, and anti-abuse policy.
- [verse_docs/implementation_strategy/proof_of_access_ledger_spec.md](verse_docs/implementation_strategy/proof_of_access_ledger_spec.md) - Canonical receipt and accounting model: off-chain ledger, reputation, epoch settlement, and optional payout channels.
- [verse_docs/implementation_strategy/community_governance_spec.md](verse_docs/implementation_strategy/community_governance_spec.md) - Canonical community policy model: roles, quorum, treasury controls, moderation, and appeals.
- [verse_docs/implementation_strategy/self_hosted_verse_node_spec.md](verse_docs/implementation_strategy/self_hosted_verse_node_spec.md) - Canonical private-by-default Verse node model: service surfaces, transport boundaries, quotas, and budget controls.
- [verse_docs/research/VERSE.md](verse_docs/research/VERSE.md) - Original tokenization and peer-role vision (speculative research).
- [verse_docs/research/SEARCH_FINDINGS_SUMMARY.md](verse_docs/research/SEARCH_FINDINGS_SUMMARY.md) - Research and source synthesis.
- [verse_docs/research/2026-02-22_aspirational_protocols_and_tools.md](verse_docs/research/2026-02-22_aspirational_protocols_and_tools.md) - Protocol ecosystem survey (IPFS, ActivityPub, Nostr, Gemini, Matrix) and crate index. Reference for Tier 2 and future protocol mod work.
- [verse_docs/research/2026-02-23_storage_economy_and_indices.md](verse_docs/research/2026-02-23_storage_economy_and_indices.md) - Speculative research on Proof of Access economy and composable Index Artifacts (Tier 2 research input).
- [verse_docs/research/2026-04-13_storage_system_comparison_for_verse.md](verse_docs/research/2026-04-13_storage_system_comparison_for_verse.md) - Comparative analysis of Syncthing, Tahoe-LAFS, Storj, and Filecoin against Verse's storage-bank direction; recommends a hybrid model: Syncthing-like bilateral storage, Tahoe-like opaque encrypted hosting, Storj-like audit/accounting, and a much lighter-than-Filecoin first implementation.
- [verse_docs/research/2026-02-23_modern_yacy_gap_analysis.md](verse_docs/research/2026-02-23_modern_yacy_gap_analysis.md) - Gap analysis for decentralized search: Index Artifact format (tantivy segments), local vs. remote query, crawler bounty economy (Tier 2 research input).
- [verse_docs/research/2026-03-28_libp2p_nostr_synergy_for_verse.md](verse_docs/research/2026-03-28_libp2p_nostr_synergy_for_verse.md) - Control-plane / data-plane analysis: Nostr as discovery/governance/social layer, libp2p as bulk content transfer. Covers community bootstrap, GossipSub trust scoring, NIP-29 swarm gating, dual-rail publication, and embedded relay as unified service endpoint.
- [verso_docs/research/2026-03-28_rss_feed_graph_model.md](verso_docs/research/2026-03-28_rss_feed_graph_model.md) - RSS/Atom as consumption + publication capability: feed graphlet chain topology, capacity eviction, post harvesting with ghost node proxies, workbench opening semantics, Atom capsule server, `feed-rs`/`atom_syndication` crates, feed poll worker.

---

## NostrCore Active Docs

NostrCore mod: Nostr protocol integration.

### NostrCore Technical Architecture

- [nostr_docs/technical_architecture/nostr_relay_spec.md](nostr_docs/technical_architecture/nostr_relay_spec.md) - Embedded Nostr relay server: three operating modes (Personal/Flock/Community), fjall event store schema, NIP-01/09/11/42/29 coverage, NostrCore ownership model, GraphIntent wiring, and rollout plan.

### NostrCore Implementation Strategy

- [nostr_docs/implementation_strategy/2026-03-05_nostr_mod_system.md](nostr_docs/implementation_strategy/2026-03-05_nostr_mod_system.md) - NostrCore native mod system: capability surface, relay infrastructure, event routing, and NIP baseline.
- [nostr_docs/implementation_strategy/2026-03-10_nostr_nip_completion_plan.md](nostr_docs/implementation_strategy/2026-03-10_nostr_nip_completion_plan.md) - Phased plan for NIP-19/21/02/05/11/25/51/65/17 coverage; dependency order and crate strategy for completing the Nostr client layer.
- [nostr_docs/implementation_strategy/nostr_core_registry_spec.md](nostr_docs/implementation_strategy/nostr_core_registry_spec.md) - Canonical `NostrCore` provider profile: capability IDs, diagnostics channel descriptors, and initial native `ModManifest` shape.
- [nostr_docs/implementation_strategy/nostr_runtime_behavior_spec.md](nostr_docs/implementation_strategy/nostr_runtime_behavior_spec.md) - Nostr runtime behavior contract: relay lifecycle, event dispatch, signing boundary, and NIP-44 DM handling.

---

## MatrixCore Active Docs

MatrixCore mod: Matrix room protocol for durable room membership and shared-space context.

### MatrixCore Implementation Strategy

- [matrix_docs/implementation_strategy/2026-03-17_matrix_core_adoption_plan.md](matrix_docs/implementation_strategy/2026-03-17_matrix_core_adoption_plan.md) - Phase-by-phase execution plan for `MatrixCore`: session lifecycle, room projection, allowlisted graph events, and optional Nostr bridge affordances.
- [matrix_docs/implementation_strategy/2026-03-17_matrix_event_schema.md](matrix_docs/implementation_strategy/2026-03-17_matrix_event_schema.md) - Concrete `graphshell.room.*` event schema for Matrix-backed rooms: payload families, validation rules, and reducer/workbench routing boundaries.
- [matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md](matrix_docs/implementation_strategy/2026-03-17_matrix_layer_positioning.md) - Places Matrix as the durable room contextual substrate within the three-context + two-fabric network model; defines room hosting gradient, cross-carrying rules, and concept resurfacing.
- [matrix_docs/implementation_strategy/2026-03-17_matrix_core_type_sketch.md](matrix_docs/implementation_strategy/2026-03-17_matrix_core_type_sketch.md) - Rust-facing type sketch for `MatrixCoreRegistry`, supervised worker commands, normalized Matrix events, and bounded proposal routing.
- [matrix_docs/implementation_strategy/matrix_core_registry_spec.md](matrix_docs/implementation_strategy/matrix_core_registry_spec.md) - Canonical `MatrixCore` provider profile: capability IDs, room model, and diagnostics channel descriptors.

---

## Graphshell Social Domain Docs

Graphshell social-domain docs cover hosted communication surfaces and related shell-side coordination rules without absorbing protocol authority from Verso, MatrixCore, NostrCore, or Verse.

### Graphshell Social Implementation Strategy

- [graphshell_docs/implementation_strategy/social/COMMS_AS_APPLETS.md](graphshell_docs/implementation_strategy/social/COMMS_AS_APPLETS.md) - Canonical positioning note for Comms as a Graphshell-hosted applet/surface family spanning Matrix rooms, Nostr social lanes, and Verso bilateral co-op/chat surfaces.
- [graphshell_docs/implementation_strategy/social/comms/2026-04-09_irc_public_comms_lane_positioning.md](graphshell_docs/implementation_strategy/social/comms/2026-04-09_irc_public_comms_lane_positioning.md) - Positions IRC as a public Comms lane for smolweb/community spaces: narrow first slice, hosted applet boundary, link capture into Graphshell, and opt-in retention only.
- [graphshell_docs/implementation_strategy/social/profile/PROFILE.md](graphshell_docs/implementation_strategy/social/profile/PROFILE.md) - Canonical social profile surface spec: host-side profile composition, publication lanes, and boundary between public identity profile vs `GraphshellProfile` app/workflow configuration.
- [graphshell_docs/implementation_strategy/social/profile/CAPSULE_PROFILE.md](graphshell_docs/implementation_strategy/social/profile/CAPSULE_PROFILE.md) - Canonical publication mapping spec from social profile cards into concrete Nostr kind 0, Finger text, and Gemini/Gopher capsule profile documents.
- [graphshell_docs/implementation_strategy/social/profile/2026-03-28_social_profile_type_sketch.md](graphshell_docs/implementation_strategy/social/profile/2026-03-28_social_profile_type_sketch.md) - Rust-facing type sketch for `SocialProfileCard`, `CapsuleProfile`, disclosure policy carriers, `GraphshellProfile` associations, and secret-provider references.
- [graphshell_docs/implementation_strategy/social/profile/serve_profile_on_all_protocols_spec.md](graphshell_docs/implementation_strategy/social/profile/serve_profile_on_all_protocols_spec.md) - Execution contract for `ServeProfileOnAllProtocols`: lane fanout, reducer/workbench/runtime boundary split, per-lane receipts, and minimum guard tests.
- [graphshell_docs/implementation_strategy/social/contacts/CONTACTS.md](graphshell_docs/implementation_strategy/social/contacts/CONTACTS.md) - Canonical contacts (flock) spec: `ContactEntry`/`ContactTag`/`UserIdentity` data model, opt-in suggestion flow from co-op sessions and Cable cabals, fjall schema, cross-feature usage (co-op approved-guest list, Nostr relay Flock allow-list, Cable cabal invites), and 4-phase rollout.

## Archive Checkpoints

- [archive_docs/checkpoint_2026-01-29/](archive_docs/checkpoint_2026-01-29/)
- [archive_docs/checkpoint_2026-02-01/](archive_docs/checkpoint_2026-02-01/)
- [archive_docs/checkpoint_2026-02-04/](archive_docs/checkpoint_2026-02-04/)
- [archive_docs/checkpoint_2026-02-09/](archive_docs/checkpoint_2026-02-09/)
- [archive_docs/checkpoint_2026-02-10/](archive_docs/checkpoint_2026-02-10/)
- [archive_docs/checkpoint_2026-02-11/](archive_docs/checkpoint_2026-02-11/)
- [archive_docs/checkpoint_2026-02-12/](archive_docs/checkpoint_2026-02-12/)
- [archive_docs/checkpoint_2026-02-14_no_legacy_cleanup/](archive_docs/checkpoint_2026-02-14_no_legacy_cleanup/)
- [archive_docs/checkpoint_2026-02-16/](archive_docs/checkpoint_2026-02-16/)
- [archive_docs/checkpoint_2026-02-17/](archive_docs/checkpoint_2026-02-17/)
- [archive_docs/checkpoint_2026-02-19/](archive_docs/checkpoint_2026-02-19/)
- [archive_docs/checkpoint_2026-02-20/](archive_docs/checkpoint_2026-02-20/)
- [archive_docs/checkpoint_2026-02-23/](archive_docs/checkpoint_2026-02-23/) — `registry_migration_plan.md`, `2026-02-23_registry_architecture_critique.md` (consolidated into `2026-02-22_registry_layer_plan.md`)
- [archive_docs/checkpoint_2026-02-24/](archive_docs/checkpoint_2026-02-24/) — consolidated-plan redirects: `2026-02-24_input_surface_polish_plan.md`, `2026-02-24_workspace_routing_polish_plan.md`, `2026-02-24_sync_logic_validation_plan.md`; `GRAPHSHELL_P2P_COLLABORATION.md` (pre-intent-model P2P design, superseded by `verso_docs/technical_architecture/VERSE_AS_NETWORK.md` and the Tier 1 sync plan)
- [archive_docs/checkpoint_2026-02-27/](archive_docs/checkpoint_2026-02-27/) — archived stale active docs: `technical_architecture/DEVELOPER_GUIDE.md`, `technical_architecture/CODEBASE_MAP.md`, `testing/VALIDATION_TESTING.md`; superseded by active `codebase_guide.md` and `test_guide.md`.
- [archive_docs/checkpoint_2026-03-01/](archive_docs/checkpoint_2026-03-01/) — bridge spike receipts and embedder-debt records for `#180` and `#183`.
- [archive_docs/checkpoint_2026-03-05/](archive_docs/checkpoint_2026-03-05/) — `2026-03-05_camera_navigation_fix_postmortem.md`: root-cause and fix record for longstanding camera pan/zoom bug (dead metadata slot + every-frame fit reset).
- [archive_docs/checkpoint_2026-03-07/](archive_docs/checkpoint_2026-03-07/) — foundational reset receipt consolidation: archived `2026-03-06_foundational_reset_architecture_vision.md`, `2026-03-06_foundational_reset_migration_governance.md`, `2026-03-06_foundational_reset_demolition_plan.md`, and `2026-03-06_clat_domain_state_core_extraction.md` after consolidating active policy/progress into `system_architecture_spec.md`, `2026-03-06_foundational_reset_implementation_plan.md`, and `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`.
- [archive_docs/checkpoint_2026-03-10/](archive_docs/checkpoint_2026-03-10/) — archived `graphshell_docs/implementation_strategy/viewer/2026-02-26_composited_viewer_pass_contract.md` after consolidating active compositor contract authority into `implementation_strategy/PLANNING_REGISTER.md` §0 and `implementation_strategy/aspect_render/frame_assembly_and_compositor_spec.md`; retained Appendix A future-work ideas live in `PLANNING_REGISTER.md` §0.10.
- [archive_docs/checkpoint_2026-03-18/](archive_docs/checkpoint_2026-03-18/) — completed registry/sector plans (`system/2026-02-22_registry_layer_plan.md`, `system/register/` Sectors A/D/F), stabilization progress receipt, C+F backend bridge receipt, foundational-reset `GraphBrowserApp` field ownership snapshot, and superseded wgpu/WebRender deferred strategy docs (`aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`, `aspect_render/2026-03-01_webrender_readiness_gate_feature_guardrails.md`).
- [archive_docs/checkpoint_2026-03-27/](archive_docs/checkpoint_2026-03-27/) — archived completed `graphshell_docs/technical_architecture/ARCHITECTURAL_CONCERNS.md` after its open items were resolved and its historical references were superseded by active specs and overview docs.
- [archive_docs/checkpoint_2026-04-01/](archive_docs/checkpoint_2026-04-01/) — archived completed node-tagging plan history: `graphshell_docs/implementation_strategy/graph/2026-02-20_node_badge_and_tagging_plan.md` and `graphshell_docs/implementation_strategy/graph/2026-03-31_node_badge_and_tagging_follow_on_plan.md`; active authority remains `graphshell_docs/implementation_strategy/graph/node_badge_and_tagging_spec.md`.
- [archive_docs/checkpoint_2026-04-02/](archive_docs/checkpoint_2026-04-02/) — archived split-note compatibility redirects and superseded implementation-plan history, including `graphshell_docs/implementation_strategy/subsystem_history/2026-02-11_bookmarks_history_import_plan.md` after splitting it into the active bookmark-import and browser-history-import plans.
- [archive_docs/checkpoint_2026-04-03/](archive_docs/checkpoint_2026-04-03/) — archived `graphshell_docs/implementation_strategy/viewer/2026-02-11_clipping_dom_extraction_plan.md` as the landed clipping execution-slice record after splitting remaining viewer-lane work into the active `graphshell_docs/implementation_strategy/viewer/2026-04-03_clipping_viewer_follow_on_plan.md`.
- [archive_docs/checkpoint_2026-04-06/](archive_docs/checkpoint_2026-04-06/) — archived `graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-03-23_navigator_host_runtime_naming_plan.md` after the host-oriented runtime naming migration landed in workbench host state, persistence, and immediate desktop UI consumers; active chrome/host semantics remain in `graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` and the Navigator / Shell / Workbench specs.
