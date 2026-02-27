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

Last updated: February 27, 2026
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
- [graphshell_docs/research/2026-02-24_visual_tombstones_research.md](graphshell_docs/research/2026-02-24_visual_tombstones_research.md) - Research backlog for visual tombstones/ghost placeholders.
- [graphshell_docs/research/2026-02-24_interaction_and_semantic_design_schemes.md](graphshell_docs/research/2026-02-24_interaction_and_semantic_design_schemes.md) - Research on interaction patterns, physics-as-semantics, and lens-based UX.
- [graphshell_docs/research/2026-02-24_diagnostics_research.md](graphshell_docs/research/2026-02-24_diagnostics_research.md) - Diagnostics system research: three-registry model (ChannelRegistry/AnalyzerRegistry/TestRegistry), probe vs. analyzer vs. test classification, current gaps, pane improvements, and priority order.
- [graphshell_docs/research/2026-02-27_viewer_state_matrix.md](graphshell_docs/research/2026-02-27_viewer_state_matrix.md) - Declared vs runtime-wired vs actually-rendered viewer matrix for migration planning.
- [graphshell_docs/research/2026-02-27_all_docs_context_bootstrap.md](graphshell_docs/research/2026-02-27_all_docs_context_bootstrap.md) - High-signal AI bootstrap summary of active+archive documentation context, priorities, invariants, and execution guardrails.
- [graphshell_docs/research/STANDALONE_EXTRACTION.md](graphshell_docs/research/STANDALONE_EXTRACTION.md) - Standalone extraction notes.

### Graphshell Technical Architecture

- [graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md](graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md) - Current architecture and component boundaries.
- [graphshell_docs/technical_architecture/ARCHITECTURAL_CONCERNS.md](graphshell_docs/technical_architecture/ARCHITECTURAL_CONCERNS.md) - Known contradictions and architecture risks.
- [graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md](graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) - Browser semantics and behavioral model; universal content viewer (MIME detection, ViewerRegistry selection, non-web renderers, tags/badges, UDC semantic physics).
- [graphshell_docs/technical_architecture/VERSO_AS_PEER.md](graphshell_docs/technical_architecture/VERSO_AS_PEER.md) - Verso mod: web capability (Servo + wry viewers, protocol handlers) and Verse peer agent (Ed25519 identity, SyncWorker, pairing, graph/workbench context sharing).
- [graphshell_docs/technical_architecture/codebase_guide.md](graphshell_docs/technical_architecture/codebase_guide.md) - Active module-orientation guide and debugging entry points for reducer/workbench/render boundaries.
- [graphshell_docs/technical_architecture/BUILD.md](graphshell_docs/technical_architecture/BUILD.md) - Build instructions and dependency notes.
- [graphshell_docs/technical_architecture/QUICKSTART.md](graphshell_docs/technical_architecture/QUICKSTART.md) - Fast-start command reference.
- [graphshell_docs/technical_architecture/2026-02-18_universal_node_content_model.md](graphshell_docs/technical_architecture/2026-02-18_universal_node_content_model.md) - Universal node content model vision.
- [graphshell_docs/technical_architecture/2026-02-27_presentation_provider_and_ai_orchestration.md](graphshell_docs/technical_architecture/2026-02-27_presentation_provider_and_ai_orchestration.md) - Provider capability contract, node facet taxonomy, and tiered AI orchestration (tiny local model + retrieval + optional large-model escalation).

### Graphshell Implementation Strategy

- [graphshell_docs/implementation_strategy/PLANNING_REGISTER.md](graphshell_docs/implementation_strategy/PLANNING_REGISTER.md) - **Canonical execution register**: active lane sequencing, stabilization bug register, issue-seeding guidance, and subsystem/lane prioritization.
- [graphshell_docs/implementation_strategy/2026-02-26_composited_viewer_pass_contract.md](graphshell_docs/implementation_strategy/2026-02-26_composited_viewer_pass_contract.md) - **Canonical surface-composition contract** for composited viewers, render-mode policy, and Appendix A foundation/debt analysis.
- [graphshell_docs/implementation_strategy/2026-02-27_ux_baseline_done_definition.md](graphshell_docs/implementation_strategy/2026-02-27_ux_baseline_done_definition.md) - UX baseline definition-of-done gate before AI-priority expansion; includes lane/issue mapping and dependency leverage plan using current Cargo stack.
- [graphshell_docs/implementation_strategy/2026-02-27_workbench_frame_tile_interaction_spec.md](graphshell_docs/implementation_strategy/2026-02-27_workbench_frame_tile_interaction_spec.md) - Canonical interaction contract for the workbench/frame/tile model, node-open routing, dual history streams, and undo-preview semantics.
- [graphshell_docs/implementation_strategy/2026-02-11_bookmarks_history_import_plan.md](graphshell_docs/implementation_strategy/2026-02-11_bookmarks_history_import_plan.md) - Bookmark/history import plan.
- [graphshell_docs/implementation_strategy/2026-02-11_clipping_dom_extraction_plan.md](graphshell_docs/implementation_strategy/2026-02-11_clipping_dom_extraction_plan.md) - DOM clipping plan.
- [graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md](graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md) - Edge traversal migration plan.
- [graphshell_docs/implementation_strategy/2026-02-20_embedder_decomposition_plan.md](graphshell_docs/implementation_strategy/2026-02-20_embedder_decomposition_plan.md) - Embedder decomposition plan.
- [graphshell_docs/implementation_strategy/2026-02-20_node_badge_and_tagging_plan.md](graphshell_docs/implementation_strategy/2026-02-20_node_badge_and_tagging_plan.md) - Badge/tagging plan.
- [graphshell_docs/implementation_strategy/2026-02-20_settings_architecture_plan.md](graphshell_docs/implementation_strategy/2026-02-20_settings_architecture_plan.md) - Settings architecture plan.
- [graphshell_docs/implementation_strategy/2026-02-21_control_plane_async_scaling.md](graphshell_docs/implementation_strategy/2026-02-21_control_plane_async_scaling.md) - Async control-plane scaling plan.
- [graphshell_docs/implementation_strategy/2026-02-21_lifecycle_intent_model.md](graphshell_docs/implementation_strategy/2026-02-21_lifecycle_intent_model.md) - Lifecycle intent model.
- [graphshell_docs/implementation_strategy/2026-02-22_multi_graph_pane_plan.md](graphshell_docs/implementation_strategy/2026-02-22_multi_graph_pane_plan.md) - Multi-graph pane architecture and workflow plan.
- [graphshell_docs/implementation_strategy/2026-02-22_workbench_workspace_manifest_persistence_plan.md](graphshell_docs/implementation_strategy/2026-02-22_workbench_workspace_manifest_persistence_plan.md) - Workbench frame/arrangement manifest persistence plan (legacy filename retained).
- [graphshell_docs/implementation_strategy/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md](graphshell_docs/implementation_strategy/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md) - Workbench tile-selector semantics overlay/promotion plan (legacy filename retained; includes absorbed frame-routing polish addendum).
- [graphshell_docs/implementation_strategy/2026-02-22_test_harness_consolidation_plan.md](graphshell_docs/implementation_strategy/2026-02-22_test_harness_consolidation_plan.md) - Unified testing harness and validation consolidation plan.
- [graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md](graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md) - **Canonical registry architecture**: Two-Pillar design (Graph/Workbench), atomic/domain registry catalog, phase plan (Phases 0–1 complete), mod system, core seed floor, topology target. Supersedes `registry_migration_plan.md` and `2026-02-23_registry_architecture_critique.md`.
- [graphshell_docs/implementation_strategy/2026-02-23_graph_interaction_consistency_plan.md](graphshell_docs/implementation_strategy/2026-02-23_graph_interaction_consistency_plan.md) - Graph interaction consistency and behavior harmonization plan (includes absorbed secondary input-surface polish scope).
- [graphshell_docs/implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md](graphshell_docs/implementation_strategy/2026-02-23_udc_semantic_tagging_plan.md) - UDC semantic tagging and layout plan.
- [graphshell_docs/implementation_strategy/2026-02-23_wry_integration_strategy.md](graphshell_docs/implementation_strategy/2026-02-23_wry_integration_strategy.md) - WRY integration strategy and platform boundary plan.
- [graphshell_docs/implementation_strategy/2026-02-24_layout_behaviors_plan.md](graphshell_docs/implementation_strategy/2026-02-24_layout_behaviors_plan.md) - Behavioral layout/creative physics plan.
- [graphshell_docs/implementation_strategy/2026-02-24_performance_tuning_plan.md](graphshell_docs/implementation_strategy/2026-02-24_performance_tuning_plan.md) - Performance scaling and frame-budget plan.
- [graphshell_docs/implementation_strategy/2026-02-24_universal_content_model_plan.md](graphshell_docs/implementation_strategy/2026-02-24_universal_content_model_plan.md) - Universal node content model implementation plan: non-web renderers (PDF, image, text, audio, SVG, directory), MIME detection pipeline, ViewerRegistry selection policy, security/sandboxing model, crate selection with license analysis.
- [graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md](graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md) - Canonical register-runtime/control-panel/signal-routing architecture and routing rules.

### Graphshell Design

- [graphshell_docs/design/KEYBINDINGS.md](graphshell_docs/design/KEYBINDINGS.md) - Keyboard interaction reference.

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

## Verse Active Docs

### Verse Technical Architecture

- [verse_docs/technical_architecture/VERSE_AS_NETWORK.md](verse_docs/technical_architecture/VERSE_AS_NETWORK.md) - The Verse network: Tier 1 bilateral iroh sync, Tier 2 community swarms (research), knowledge asset pipeline, participation levels, wire format, conflict resolution, network architecture diagrams.

### Verse Implementation Strategy

- [verse_docs/implementation_strategy/2026-02-22_verse_implementation_strategy.md](verse_docs/implementation_strategy/2026-02-22_verse_implementation_strategy.md) - Verse implementation strategy and phase framing.
- [verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) - **Canonical Verse Tier 1 sync plan** (iroh transport, sync units, pairing/sync phases, deterministic sync-logic simulator matrix).
- [verse_docs/implementation_strategy/PHASE5_STEP5.1_COMPLETE.md](verse_docs/implementation_strategy/PHASE5_STEP5.1_COMPLETE.md) - Phase 5 Step 5.1 completion record.
- [verse_docs/implementation_strategy/PHASE5_STEP5.2_COMPLETE.md](verse_docs/implementation_strategy/PHASE5_STEP5.2_COMPLETE.md) - Phase 5 Step 5.2 completion record.
- [verse_docs/implementation_strategy/PHASE5_STEP5.3_COMPLETE.md](verse_docs/implementation_strategy/PHASE5_STEP5.3_COMPLETE.md) - Phase 5 Step 5.3 completion record.
- [verse_docs/research/VERSE.md](verse_docs/research/VERSE.md) - Original tokenization and peer-role vision (speculative research).
- [verse_docs/research/SEARCH_FINDINGS_SUMMARY.md](verse_docs/research/SEARCH_FINDINGS_SUMMARY.md) - Research and source synthesis.
- [verse_docs/research/2026-02-22_aspirational_protocols_and_tools.md](verse_docs/research/2026-02-22_aspirational_protocols_and_tools.md) - Protocol ecosystem survey (IPFS, ActivityPub, Nostr, Gemini, Matrix) and crate index. Reference for Tier 2 and future protocol mod work.
- [verse_docs/research/2026-02-23_storage_economy_and_indices.md](verse_docs/research/2026-02-23_storage_economy_and_indices.md) - Speculative research on Proof of Access economy and composable Index Artifacts (Tier 2 research input).
- [verse_docs/research/2026-02-23_modern_yacy_gap_analysis.md](verse_docs/research/2026-02-23_modern_yacy_gap_analysis.md) - Gap analysis for decentralized search: Index Artifact format (tantivy segments), local vs. remote query, crawler bounty economy (Tier 2 research input).

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
- [archive_docs/checkpoint_2026-02-24/](archive_docs/checkpoint_2026-02-24/) — consolidated-plan redirects: `2026-02-24_input_surface_polish_plan.md`, `2026-02-24_workspace_routing_polish_plan.md`, `2026-02-24_sync_logic_validation_plan.md`; `GRAPHSHELL_P2P_COLLABORATION.md` (pre-intent-model P2P design, superseded by `VERSE_AS_NETWORK.md` and the Tier 1 sync plan)
- [archive_docs/checkpoint_2026-02-27/](archive_docs/checkpoint_2026-02-27/) — archived stale active docs: `technical_architecture/DEVELOPER_GUIDE.md`, `technical_architecture/CODEBASE_MAP.md`, `testing/VALIDATION_TESTING.md`; superseded by active `codebase_guide.md` and `test_guide.md`.
