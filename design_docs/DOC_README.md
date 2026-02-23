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
5. [graphshell_docs/](graphshell_docs/)
6. [verse_docs/](verse_docs/)
7. [archive_docs/](archive_docs/)

## Working Principles

- Verify claims against the code/docs before repeating them as facts.
- Keep active docs in `graphshell_docs/` or `verse_docs/`; move superseded material to `archive_docs/` checkpoints.
- Prefer updating an existing doc over creating a new one unless the scope clearly requires a new category/resource.
- Do not edit `PROJECT_DESCRIPTION.md` unless explicitly requested.
- Keep this index aligned with folder structure and status in the same session as any doc changes.
- If an AI note/memory adds a durable project principle, record it here under Working Principles.
- If another index conflicts with this file, this file is authoritative and the others should be aligned.
- Migration Strategy: Iterative Replacement
- - Since there are no active users, we prioritize **code cleanliness** over backward compatibility. We will replace subsystems directly rather than maintaining parallel legacy paths.
- When designing a new feature, ask:
- - Is the way you want this system to work consistent with our architectural guarantees (modularity, parallelization, access through intents and not direct state mutation, componentization as opposed to consolidation into monolithic core files, centralization of testing + diagnostic threading to automate testing)?
- - How can we refine the integration to meet our feature goals but respect our architecture?

# Design Docs Index

Last updated: February 21, 2026  
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
- [graphshell_docs/research/STANDALONE_EXTRACTION.md](graphshell_docs/research/STANDALONE_EXTRACTION.md) - Standalone extraction notes.

### Graphshell Technical Architecture

- [graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md](graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md) - Current architecture and component boundaries.
- [graphshell_docs/technical_architecture/ARCHITECTURAL_CONCERNS.md](graphshell_docs/technical_architecture/ARCHITECTURAL_CONCERNS.md) - Known contradictions and architecture risks.
- [graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md](graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) - Browser semantics and behavioral model.
- [graphshell_docs/technical_architecture/CODEBASE_MAP.md](graphshell_docs/technical_architecture/CODEBASE_MAP.md) - Module map and data flow overview.
- [graphshell_docs/technical_architecture/DEVELOPER_GUIDE.md](graphshell_docs/technical_architecture/DEVELOPER_GUIDE.md) - Contributor orientation and workflows.
- [graphshell_docs/technical_architecture/BUILD.md](graphshell_docs/technical_architecture/BUILD.md) - Build instructions and dependency notes.
- [graphshell_docs/technical_architecture/QUICKSTART.md](graphshell_docs/technical_architecture/QUICKSTART.md) - Fast-start command reference.
- [graphshell_docs/technical_architecture/2026-02-18_universal_node_content_model.md](graphshell_docs/technical_architecture/2026-02-18_universal_node_content_model.md) - Universal node content model vision.

### Graphshell Implementation Strategy

- [graphshell_docs/implementation_strategy/IMPLEMENTATION_ROADMAP.md](graphshell_docs/implementation_strategy/IMPLEMENTATION_ROADMAP.md) - Feature targets, dependencies, validation order.
- [graphshell_docs/implementation_strategy/2026-02-11_bookmarks_history_import_plan.md](graphshell_docs/implementation_strategy/2026-02-11_bookmarks_history_import_plan.md) - Bookmark/history import plan.
- [graphshell_docs/implementation_strategy/2026-02-11_clipping_dom_extraction_plan.md](graphshell_docs/implementation_strategy/2026-02-11_clipping_dom_extraction_plan.md) - DOM clipping plan.
- [graphshell_docs/implementation_strategy/2026-02-11_diagnostic_inspector_plan.md](graphshell_docs/implementation_strategy/2026-02-11_diagnostic_inspector_plan.md) - Diagnostic inspector plan.
- [graphshell_docs/implementation_strategy/2026-02-11_p2p_collaboration_plan.md](graphshell_docs/implementation_strategy/2026-02-11_p2p_collaboration_plan.md) - P2P collaboration implementation plan.
- [graphshell_docs/implementation_strategy/2026-02-11_performance_optimization_plan.md](graphshell_docs/implementation_strategy/2026-02-11_performance_optimization_plan.md) - Performance optimization plan.
- [graphshell_docs/implementation_strategy/2026-02-16_architecture_and_navigation_plan.md](graphshell_docs/implementation_strategy/2026-02-16_architecture_and_navigation_plan.md) - Architecture/navigation consolidation.
- [graphshell_docs/implementation_strategy/2026-02-17_egl_embedder_extension_plan.md](graphshell_docs/implementation_strategy/2026-02-17_egl_embedder_extension_plan.md) - EGL embedder extension plan (deferred/archival note retained).
- [graphshell_docs/implementation_strategy/2026-02-18_edge_operations_and_cmd_palette_plan.md](graphshell_docs/implementation_strategy/2026-02-18_edge_operations_and_cmd_palette_plan.md) - Edge operations and command palette plan.
- [graphshell_docs/implementation_strategy/2026-02-18_single_window_active_obviation_plan.md](graphshell_docs/implementation_strategy/2026-02-18_single_window_active_obviation_plan.md) - Single-window obviation plan (deferred).
- [graphshell_docs/implementation_strategy/2026-02-19_graph_ux_polish_plan.md](graphshell_docs/implementation_strategy/2026-02-19_graph_ux_polish_plan.md) - UX polish plan.
- [graphshell_docs/implementation_strategy/2026-02-19_ios_port_plan.md](graphshell_docs/implementation_strategy/2026-02-19_ios_port_plan.md) - iOS port plan (deferred).
- [graphshell_docs/implementation_strategy/2026-02-19_layout_advanced_plan.md](graphshell_docs/implementation_strategy/2026-02-19_layout_advanced_plan.md) - Advanced layout/physics plan.
- [graphshell_docs/implementation_strategy/2026-02-19_persistence_hub_plan.md](graphshell_docs/implementation_strategy/2026-02-19_persistence_hub_plan.md) - Persistence hub plan.
- [graphshell_docs/implementation_strategy/2026-02-19_undo_redo_plan.md](graphshell_docs/implementation_strategy/2026-02-19_undo_redo_plan.md) - Undo/redo plan.
- [graphshell_docs/implementation_strategy/2026-02-19_workspace_routing_and_membership_plan.md](graphshell_docs/implementation_strategy/2026-02-19_workspace_routing_and_membership_plan.md) - Workspace routing/membership plan.
- [graphshell_docs/implementation_strategy/2026-02-20_cross_platform_sync_and_extension_plan.md](graphshell_docs/implementation_strategy/2026-02-20_cross_platform_sync_and_extension_plan.md) - Sync clients and extension plan.
- [graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md](graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md) - Edge traversal migration plan.
- [graphshell_docs/implementation_strategy/2026-02-20_embedder_decomposition_plan.md](graphshell_docs/implementation_strategy/2026-02-20_embedder_decomposition_plan.md) - Embedder decomposition plan.
- [graphshell_docs/implementation_strategy/2026-02-20_node_badge_and_tagging_plan.md](graphshell_docs/implementation_strategy/2026-02-20_node_badge_and_tagging_plan.md) - Badge/tagging plan.
- [graphshell_docs/implementation_strategy/2026-02-20_settings_architecture_plan.md](graphshell_docs/implementation_strategy/2026-02-20_settings_architecture_plan.md) - Settings architecture plan.
- [graphshell_docs/implementation_strategy/2026-02-21_control_plane_async_scaling.md](graphshell_docs/implementation_strategy/2026-02-21_control_plane_async_scaling.md) - Async control-plane scaling plan.
- [graphshell_docs/implementation_strategy/2026-02-21_lifecycle_intent_model.md](graphshell_docs/implementation_strategy/2026-02-21_lifecycle_intent_model.md) - Lifecycle intent model.
- [graphshell_docs/implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md](graphshell_docs/implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md) - UDC semantic tagging and layout plan.


### Graphshell Design

- [graphshell_docs/design/KEYBINDINGS.md](graphshell_docs/design/KEYBINDINGS.md) - Keyboard interaction reference.

### Graphshell Testing

- [graphshell_docs/testing/VALIDATION_TESTING.md](graphshell_docs/testing/VALIDATION_TESTING.md) - Manual validation tests and gap tracking.
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

## Verse Active Docs

- [verse_docs/research/VERSE.md](verse_docs/research/VERSE.md) - Verse architecture and tokenization overview.
- [verse_docs/research/SEARCH_FINDINGS_SUMMARY.md](verse_docs/research/SEARCH_FINDINGS_SUMMARY.md) - Research and source synthesis.
- [verse_docs/technical_architecture/GRAPHSHELL_P2P_COLLABORATION.md](verse_docs/technical_architecture/GRAPHSHELL_P2P_COLLABORATION.md) - P2P collaboration architecture and integration model.

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
