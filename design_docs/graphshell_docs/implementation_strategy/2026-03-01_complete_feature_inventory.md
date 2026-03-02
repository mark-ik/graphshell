# Complete Feature Inventory

**Date**: 2026-03-01
**Status**: Living document — synthesized from all design docs and archive checkpoints
**Scope**: All features for Graphshell, Verso, and Verse: implemented, planned, and speculative
**Sources**: All files in `design_docs/` including `archive_docs/`, `verse_docs/`, `research/`, `implementation_strategy/`

Status codes:
- **✅ Done** — Implemented and merged
- **🔨 Active** — In progress (current milestone)
- **📋 Planned** — Specified, on roadmap, not yet started
- **🔭 Speculative** — Aspirational, research-backed, no committed schedule
- **🗄️ Archived** — Superseded or deferred indefinitely

---

## GRAPHSHELL — Core Spatial Browser

### Canvas & Graph Data Model

| Feature | Status | Notes / Source |
|---------|--------|----------------|
| Force-directed node graph canvas | ✅ Done | egui_graphs physics simulation |
| Node creation, deletion, repositioning | ✅ Done | GraphIntent reducer |
| User-grouped edges (explicit connections) | ✅ Done | `EdgeKind::UserGrouped` |
| Traversal-derived edges | ✅ Done | `EdgeKind::TraversalDerived`; edges accumulate directed traversal events |
| Multi-edge display with traversal styling | ✅ Done | Dominant direction computed from traversal counts |
| Edge dominant-direction visual arrows | ✅ Done | Behavioral tendency display, not structural type |
| Graph persistence (WAL-based) | ✅ Done | Append-only intent log; AES-256-GCM at rest |
| Graph serialization to JSON | ✅ Done | Frame snapshots |
| Zoom and pan controls | ✅ Done | `canvas/graph_node_edge_interaction_spec.md` |
| Node selection (single) | ✅ Done | `canvas/graph_node_edge_interaction_spec.md` |
| Multi-select | ✅ Done | `UpdateSelection`, `SelectAll` |
| Lasso selection | ✅ Done | `canvas/graph_node_edge_interaction_spec.md` |
| Lasso modifier modes (replace / add / toggle) | 📋 Planned | Spec in `2026-03-01_ux_migration_design_spec.md §5.1` |
| Connected-node selection | 📋 Planned | Select all nodes reachable from selection |
| Ghost Nodes (deleted nodes rendered as faint topology-preserving placeholders) | 📋 Planned | User-facing term; backed by `NodeLifecycle::Tombstone`; faint dashed outline, ghost edges, "Show Deleted" toggle; `2026-02-24_visual_tombstones_research.md` |
| Node pinning (lock position) | 📋 Planned | `L` key; `I`/`U` for multi-pin |
| Node duplicate | 📋 Planned | `Ctrl+D` |
| Group-move (move selection together) | 📋 Planned | Drag selection |
| Node audit log (create/modify/delete history) | 📋 Planned | Per-node event history |
| Edge walk (Tab traversal along edges) | 📋 Planned | `Tab`/`Shift+Tab` for edge-by-edge navigation |
| Bidirectional edge creation | 📋 Planned | `Shift+G` |
| Self-loop edge exclusion from physics | ✅ Done | Regression test coverage |
| Semantic Gravity (tag-based clustering) | 📋 Planned | Related nodes attracted by shared tags |
| Frame-affinity organizational regions (legacy alias: Magnetic Zones) | 🔭 Speculative | Region-based spatial grouping interpreted through graph-first frame semantics |
| Ghost Node restoration | 📋 Planned | Command Palette contextual mode (or right-click contextual invocation) → restore or permanently delete; `viewer/visual_tombstones_spec.md` |
| Undo / redo | 🔭 Speculative | Command-based journaling; `2026-02-19_undo_redo_plan.md` (archived) |
| Lasso zoning (region-based organization) | 📋 Planned | Persistent lasso regions |
| Origin-grouped process organization | 📋 Planned | Auto-grouping by domain/origin |

### Node Lifecycle

| Feature | Status | Notes |
|---------|--------|-------|
| Active → Warm → Cold → Tombstone lifecycle | ✅ Done | Four-state model; `viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| LRU warm-cache eviction under memory pressure | ✅ Done | `lifecycle_reconcile.rs` |
| Memory pressure levels (Normal / Warning / Critical) | ✅ Done | `viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| RuntimeBlocked state with retry scheduling | ✅ Done | `MarkRuntimeBlocked` / `ClearRuntimeBlocked`; `viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| Recovery action affordance for blocked nodes | 🔨 Active | UX contract S5 (`SUBSYSTEM_UX_SEMANTICS.md`) |
| Per-tile GPU memory budget with graceful degradation | 📋 Planned | `2026-02-26_composited_viewer_pass_contract.md` |

### Physics & Layout

| Feature | Status | Notes |
|---------|--------|-------|
| Graphshell-owned physics engine | ✅ Done | Migrated from egui_graphs |
| Physics presets: Compact / Spread / Tight / Star / Frozen | ✅ Done | `PhysicsProfileRegistry` |
| Physics parameter tuning (c_repulse, c_attract, k_scale, max_step, damping) | ✅ Done | `canvas/layout_behaviors_and_physics_spec.md` |
| Physics toggle (T), reheat (R), preset cycle (Shift+T) | 📋 Planned | Keybindings spec |
| Seeded RNG for deterministic layout (test mode) | 📋 Planned | `SeedRng` UxBridgeCommand |
| Force-directed layout (default) | ✅ Done | `canvas/layout_behaviors_and_physics_spec.md` |
| Grid layout | 📋 Planned | `LayoutRegistry` entry |
| Hierarchical layout | 📋 Planned | |
| Radial-from-focus layout | 📋 Planned | |
| Community-clustered layout | 📋 Planned | |
| Layout mode switching (Canonical ↔ Divergent) | ✅ Done | `GraphLayoutMode`; `SetViewLayoutMode` intent |
| LocalSimulation for Divergent views | ✅ Done | Independent physics per Divergent pane |
| Camera fit / smart fit | ✅ Done | `RequestFitToScreen` |
| Target-locked / pointer-relative zoom | 📋 Planned | `2026-03-01_ux_migration_design_spec.md §5.2` |
| Zoom to node | 📋 Planned | |
| 2D ↔ 3D hotswitch (`ViewDimension`) | 📋 Planned | `SetViewDimension` intent exists; UI not yet wired |
| 3D canvas rendering variants (full 3D, stacked, soft 3D) | 🔭 Speculative | |
| Level-of-Detail (LOD) rendering: Point / Compact / Standard / Detail | 📋 Planned | Zoom-level-driven semantic zoom |
| Semantic fisheye / Degree of Interest (DOI) visualization | 🔭 Speculative | |
| SketchLay: user-guided layout via sketch constraints | 🔭 Speculative | Relative placement, alignment, fixed anchors; research in UX migration spec |
| Readability-driven layout adaptation | 📋 Planned | Haleem et al. readability metrics; condition-based suggestions |
| Edge bundling (high crossing count) | 📋 Planned | Readability adaptation trigger |
| Constraint-based layouts | 🔭 Speculative | `2026-02-19_layout_advanced_plan.md` |
| Progressive lenses (semantic zoom context layers) | 🔭 Speculative | |
| Physics extensibility (custom force models via mods) | 📋 Planned | `PhysicsProfileRegistry` mod extension |

### Workbench & Tile Tree

| Feature | Status | Notes |
|---------|--------|-------|
| egui_tiles tile tree (Tab Group, Split, Grid) | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| Graph View pane (`TileKind::Graph`) | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| Node Pane (`TileKind::Node`) | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| Tool Pane (`TileKind::Tool`) | ✅ Done | Diagnostics today; history/settings/accessibility panes planned |
| Tiled Pane / Docked Pane presentation modes | ✅ Done | `workbench/pane_presentation_and_locking_spec.md` |
| PaneLock (Unlocked / PositionLocked / FullyLocked) | ✅ Done | `workbench/pane_presentation_and_locking_spec.md` |
| Tile drag-and-drop, split, reorder | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| FrameTabSemantics persistence | ✅ Done | Semantic tab group membership survives egui_tiles simplification |
| Frame (persisted Tile Tree branch) | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| Frame Snapshot (FrameLayout + FrameManifest + FrameMetadata) | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| Frame switching / workbar | ✅ Done | `workbench/workbench_frame_tile_interaction_spec.md` |
| Frame creation from selection (`Ctrl+Shift+N`) | 📋 Planned | |
| Frame management keybindings (Ctrl+Tab, Ctrl+W, Ctrl+Shift+H/V) | 📋 Planned | |
| Graph-first frame semantics | 📋 Planned | Frame as graph-level entity; opening node creates view, not frame |
| Multiple graph views per workbench | ✅ Done | Multiple `GraphViewId` panes |
| Multiple workbenches (`Ctrl+Alt+N`, `Ctrl+Alt+Tab`) | 📋 Planned | Workbench switching; merge graphs |
| Canonical vs Divergent multi-view | ✅ Done | `GraphLayoutMode` |
| Camera sync between canonical views | 📋 Planned | |
| Last-focused pane restoration | ✅ Done | `workbench/pane_presentation_and_locking_spec.md`; `subsystem_focus/focus_and_region_navigation_spec.md` |
| Pane-close successor focus handoff | ✅ Done | Regression test coverage; `workbench/pane_presentation_and_locking_spec.md` |
| WorkbenchProfile (keybindings + layout policy + mouse map) | 📋 Planned | Default profiles: Standard, Laptop, Accessibility, Touch, Power User |
| Workflow = Lens × WorkbenchProfile | 📋 Planned | `WorkflowRegistry` (future) |
| Semantic gap principle (explicit registry/domain boundaries) | ✅ Done | Architectural governance rule |

### Surface Composition & Rendering

| Feature | Status | Notes |
|---------|--------|-------|
| egui (Glow/OpenGL) rendering | ✅ Done | `aspect_render/render_backend_contract_spec.md` |
| Three-pass composition model (UI Chrome → Content → Overlay Affordance) | 🔨 Active | `§0 PLANNING_REGISTER.md`; `compositor_adapter.rs` |
| CompositorAdapter (GL state isolation around Servo callbacks) | 🔨 Active | Save/restore/scrub GL state |
| TileRenderMode enum (CompositedTexture / NativeOverlay / EmbeddedEgui / Placeholder) | ✅ Done | `aspect_render/frame_assembly_and_compositor_spec.md` |
| Overlay Affordance Policy per TileRenderMode | 🔨 Active | Focus/hover rings correct in all modes |
| Focus ring visible above composited Servo content | 🔨 Active | Z-order fix in compositor |
| Differential composition (skip re-render of unchanged tiles) | 🔭 Speculative | `2026-02-26_composited_viewer_pass_contract.md` |
| Compositor replay traces (forensic debugging) | 🔭 Speculative | |
| Compositor chaos mode (GL isolation validation) | 📋 Planned | |
| Multi-backend hot-swap per tile at runtime | 🔭 Speculative | |
| Cross-tile compositor transitions (live content during animation) | 🔭 Speculative | |
| Cinematic tile transitions | 🔭 Speculative | |
| Content-aware adaptive overlay styling | 🔭 Speculative | |
| Mod-hosted compositor extension passes | 🔭 Speculative | |
| egui → wgpu custom canvas migration | 📋 Planned | `2026-03-01_webrender_wgpu_renderer_implementation_plan.md` |
| WebRender wgpu renderer | 📋 Planned | GL → wgpu migration |
| EGL embedder extension (hardware-accelerated backend) | 🔭 Speculative | `2026-02-17_egl_embedder_extension_plan.md` (archived) |

### Viewer System

| Feature | Status | Notes |
|---------|--------|-------|
| ViewerRegistry (MIME → viewer resolution) | ✅ Done | `system/register/viewer_registry_spec.md` |
| Servo-powered HTML/CSS/JS rendering | ✅ Done | Via Verso mod |
| HTTP / HTTPS protocol handling | ✅ Done | Via Verso mod |
| File protocol handling | ✅ Done | `system/register/protocol_registry_spec.md` |
| Placeholder viewer (fallback) | ✅ Done | `TileRenderMode::Placeholder` |
| Viewer capability declarations per surface | ✅ Done | `AccessibilityCapabilities`; `ux_semantics_capabilities` |
| Viewer attachment lifecycle | ✅ Done | `MapWebviewToNode`, `UnmapWebview` |
| WebView crash handling | ✅ Done | `WebViewCrashed` intent; `viewer/webview_lifecycle_and_crash_recovery_spec.md` |
| Wry native window integration | 📋 Planned | `NativeOverlay` render mode |
| PDF viewer | 📋 Planned | Non-web native viewer |
| CSV / data viewer | 📋 Planned | |
| Settings viewer (EmbeddedEgui) | 📋 Planned | |
| Markdown / plaintext viewer | 📋 Planned | Core Seed viewer |
| Filesystem ingest (files→nodes, folders→frames, folder-tag links) | 📋 Planned | Blocked behind common document-viewer readiness; see `viewer/2026-03-02_filesystem_ingest_graph_mapping_plan.md` |
| Clipping tool (DOM element extraction to node) | 📋 Planned | Web page excerpt → new node |
| Interactive HTML export with graph embedding | 🔭 Speculative | |
| DOM inspection / element picker | 📋 Planned | |
| WARC archive export / import | 📋 Planned | Forensic fidelity web archiving |
| Bookmarks import (browser) | 📋 Planned | |
| Browsing history import | 📋 Planned | |
| Data portability (cross-browser graph import/export) | 📋 Planned | |
| Multiple viewer backend hot-swap at runtime | 🔭 Speculative | |

### Omnibar & Control Surfaces

| Feature | Status | Notes |
|---------|--------|-------|
| Command Palette shell (search + contextual modes) | 🔨 Active | Canonical authority surface; search-first mode + contextual modes (`aspect_command/command_surface_interaction_spec.md`) |
| Radial Palette Mode (2-tier category→option rings) | 📋 Planned | Contextual command-palette mode with periphery rails, hover expansion, bounded radial labels |
| Omnibar (Ctrl+L; URL + graph search + web search) | 🔨 Active | Slash-commands planned |
| Context Palette Mode (2-tier strip/list) | 📋 Planned | Tier-1 horizontal category strip + Tier-2 vertical option list; shares model with Radial Palette Mode |
| ActionRegistry routing for all control surfaces | ✅ Done | No hardcoded command enums |
| Radial palette geometry/overflow/label contract | 📋 Planned | Tiered ring non-overlap, hover-scaling, bounded radial-label behavior |
| Radial marking menus (muscle memory gestures) | 🔭 Speculative | Faster selection via gesture direction |
| Command palette slot ordering (context-sensitive) | 📋 Planned | Most-relevant commands first |
| Omnibar slash-commands | 📋 Planned | e.g. `/tag`, `/frame`, `/physics` |
| Global keybinding remapping (WorkbenchProfile) | 📋 Planned | |
| Chord and sequence keybindings | ✅ Done | Input context stack |
| Gamepad full support (D-pad / stick navigation) | ✅ Done | Radial Palette Mode default input |

### Input Event Architecture

| Feature | Status | Notes |
|---------|--------|-------|
| Three-phase event dispatch (capture → at-target → bubble) | 📋 Planned | DOM Event Model pattern; `2026-03-01_ux_migration_design_spec.md §7` |
| Event propagation control (stopPropagation, preventDefault) | 📋 Planned | |
| Modal isolation (events consumed by modal, not leaked) | 📋 Planned | Modal at Workbench level |
| Input context stack and routing | ✅ Done | `aspect_input/input_interaction_spec.md` |
| InputRegistry keybinding resolution | ✅ Done | `system/register/input_registry_spec.md` |

### History & Traversal

| Feature | Status | Notes |
|---------|--------|-------|
| Traversal recording (all navigation events) | ✅ Done | Edges accumulate directed traversal events |
| Traversal types: link click, back/forward, address bar, programmatic | ✅ Done | `NavigationTrigger` |
| Edge metrics: total traversals, last time, dominant direction | ✅ Done | `subsystem_history/edge_traversal_spec.md` |
| Traversal archive (rolling window + overflow to disk) | ✅ Done | `subsystem_history/edge_traversal_spec.md` |
| WAL-based traversal logging | ✅ Done | `subsystem_history/edge_traversal_spec.md` |
| History Manager pane (Timeline + recent traversals) | 📋 Planned | Tool Pane; replaces legacy nav history panel |
| Timeline visualization of traversals | 📋 Planned | |
| Temporal navigation / time-travel preview | 📋 Planned | |
| Traversal scrubber (timeline replay) | 📋 Planned | |
| Graph state snapshots for recovery | 📋 Planned | |
| "Return to present" from temporal navigation | 📋 Planned | |
| Back / forward within node pane | ✅ Done | WebView history |
| Intra-node navigation separate from edge history | ✅ Done | Node history vs graph edge traversals |

### Knowledge Capture & Tagging

| Feature | Status | Notes |
|---------|--------|-------|
| Node tags (user-applied labels) | ✅ Done | `TagNode` / `UntagNode` intents |
| KnowledgeRegistry (UDC semantic tagging) | ✅ Done | Universal Decimal Classification |
| Node badges (compact metadata overlay) | 📋 Planned | LOD-driven display |
| Faceted object model (PMEST: Personality, Matter, Energy, Space, Time) | 📋 Planned | `NodeFacets` struct; `2026-03-01_ux_migration_design_spec.md §4` |
| Faceted filter surface (Lens component) | 📋 Planned | Spec needed |
| Facet rail navigation (Enter-to-pane routing) | 📋 Planned | Facet → open dedicated facet pane |
| Auto-grouping by relevance / time / domain | 📋 Planned | |
| "Desire paths" semantic suggestions | 🔭 Speculative | Traversal patterns surface implicit connections |
| Smart filtering and saved views | 📋 Planned | |
| Faceted search (multi-axis queries) | 📋 Planned | Replaces flat full-text search |
| Full-text search | 📋 Planned | Local tantivy index |
| Thumbnails / favicons for nodes | 📋 Planned | `2026-02-11_thumbnails_favicons_plan.md` |
| Node thumbnail capture pipeline | ✅ Done | `thumbnail_pipeline.rs` |
| Readability extraction (text, UDC tags from page) | 📋 Planned | Auto-tag on open |
| Report generation (signed browsing record) | 📋 Planned | Verso/Verse integration |

### Lens System

| Feature | Status | Notes |
|---------|--------|-------|
| Lens = Topology + Layout + Physics + Theme | ✅ Done | `LensConfig` |
| LensCompositor (resolves Lens from domain registries) | ✅ Done | `system/register/lens_compositor_spec.md` |
| File Explorer Lens preset | 📋 Planned | DAG topology, tree layout, solid physics |
| Brainstorm Lens preset | 📋 Planned | Free topology, force-directed, liquid physics |
| States-of-matter physics presets (Liquid / Gas / Solid) | 📋 Planned | Semantic physics metaphor |
| Lens switching (per-view) | ✅ Done | `SetViewLens` intent |
| Progressive lens composition | 🔭 Speculative | |

### Diagnostics Subsystem

| Feature | Status | Notes |
|---------|--------|-------|
| DiagnosticsRegistry (channel schema, severity, invariants) | ✅ Done | `system/register/diagnostics_registry_spec.md` |
| ChannelSeverity (Error / Warn / Info) | ✅ Done | `system/register/diagnostics_registry_spec.md` |
| Event ring (VecDeque<DiagnosticEvent>) | ✅ Done | `subsystem_diagnostics/diagnostics_observability_and_harness_spec.md` |
| Diagnostic Inspector pane | ✅ Done | Engine, Compositor, Intents views |
| Invariant watchdog (started → succeeded/failed coverage) | 📋 Planned | Thin coverage; needs expansion |
| AnalyzerRegistry (continuous stream processors) | 📋 Planned | |
| TestHarness in-pane runner (diagnostics_tests feature) | 📋 Planned | |
| Health summary (per-subsystem green/yellow/red) | 📋 Planned | |
| Invariant graph (DAG of pending/healthy/violated watchdogs) | 📋 Planned | |
| Violations view in inspector pane | 📋 Planned | |
| Channel config editor (live toggle, sample rate) | 📋 Planned | |
| Self-check channels (`diagnostics.selfcheck.*`) | 📋 Planned | Orphan detection, phase completeness |
| Compositor snapshot diagnostics | ✅ Done | `subsystem_diagnostics/diagnostics_observability_and_harness_spec.md` |
| Intent → frame correlation timeline | 📋 Planned | |

### UX Semantics Subsystem (New)

| Feature | Status | Notes |
|---------|--------|-------|
| UxTree (per-frame semantic projection of native UI) | 📋 Planned | `SUBSYSTEM_UX_SEMANTICS.md` |
| UxNodeId (stable path-based identifiers) | 📋 Planned | |
| UxProbeSet (structural invariant checks, every frame) | 📋 Planned | S/N/M invariant series |
| UxBridge (WebDriver custom command extensions) | 📋 Planned | `GetUxSnapshot`, `InvokeUxAction`, etc. |
| UxDriver (test-side harness client) | 📋 Planned | |
| UxScenario runner (YAML scenario files) | 📋 Planned | |
| UxSnapshot / UxBaseline / UxDiff (regression CI) | 📋 Planned | |
| AccessKit bridge consuming UxTree output | 📋 Planned | Phase 6 |

### Accessibility Subsystem

| Feature | Status | Notes |
|---------|--------|-------|
| Accessibility Inspector scaffold | ✅ Done | Bridge health + diagnostics pane |
| AccessKit integration (egui bridge) | 🔨 Active | Version mismatch (0.24 vs 0.21) blocking injection |
| WebView accessibility tree bridge | 🔨 Active | Forwarding in place; injection degraded |
| Graph Reader (virtual accessibility tree for graph canvas) | 📋 Planned | `GraphAccessKitAdapter`; Room Mode + Map Mode |
| Room Mode navigation (focused node as room with edge-doors) | 📋 Planned | Depth-1 local context |
| Map Mode navigation (full graph linearization) | 📋 Planned | Semantic hierarchy: Cluster → Hub → Leaf |
| Spatial D-pad navigation (arrow keys → nearest node by direction) | 📋 Planned | |
| F6 region cycle (Toolbar → Graph → Active Pane) | ✅ Done | Regression test coverage |
| Sonification (spatial audio for graph state) | 🔭 Speculative | `rodio` + `fundsp`; panning by X coord, pitch by degree |
| Screen reader smoke tests (NVDA/Orca) | 📋 Planned | Milestone gate requirement |
| High-contrast theme | 📋 Planned | |
| Focus Subsystem (deterministic focus routing) | 🔨 Active | |

### Security & Storage Subsystems

| Feature | Status | Notes |
|---------|--------|-------|
| Ed25519 keypair generation and OS keychain storage | 📋 Planned | Verse Tier 1 prerequisite |
| Trust store (IdentityRegistry) | 📋 Planned | |
| Access control scoping per workspace | 📋 Planned | |
| Persistence (WAL-based) | ✅ Done | `subsystem_storage/storage_and_persistence_integrity_spec.md` |
| Frame save / load | ✅ Done | `subsystem_storage/storage_and_persistence_integrity_spec.md` |
| Workspace manifest persistence | ✅ Done | `subsystem_storage/storage_and_persistence_integrity_spec.md` |
| AES-256-GCM at-rest encryption (SyncLog) | 📋 Planned | Verse Tier 1 |
| Single-write-path invariant | ✅ Done | `subsystem_storage/storage_and_persistence_integrity_spec.md` |
| Data portability (JSON graph snapshots) | ✅ Done | `subsystem_storage/storage_and_persistence_integrity_spec.md` |

### Mods Subsystem

| Feature | Status | Notes |
|---------|--------|-------|
| ModRegistry (native mod loading via inventory::submit!) | ✅ Done | `system/register/mod_registry_spec.md` |
| WASM mod loading (extism sandbox) | 📋 Planned | |
| ModManifest (provides + requires declaration) | ✅ Done | `subsystem_mods/SUBSYSTEM_MODS.md` |
| Core Seed (app functional with zero mods) | ✅ Done | Architectural invariant |
| Mod-contributed physics profiles | 📋 Planned | |
| Mod-contributed viewers | 📋 Planned | |
| Mod-contributed canvas regions / topology policies | 📋 Planned | |
| Mod-contributed compositor passes | 🔭 Speculative | |
| WASM mod ABI and sandboxing spec | 📋 Planned | `2026-02-24_wasm_mod_abi_research.md` |

### Agent System

| Feature | Status | Notes |
|---------|--------|-------|
| AgentRegistry (autonomous background agents) | ✅ Done | Registry defined |
| Agent-derived edges (recommendation edges) | 📋 Planned | `EdgeKind::AgentDerived` |
| Agent confidence scoring | 📋 Planned | |
| Agent edge time-decay | 📋 Planned | |
| Agent edge promotion to UserGrouped on traversal | 📋 Planned | |
| Multi-model cooperative execution | 🔭 Speculative | |
| Tiered AI orchestration (local tiny → retrieval → large-model escalation) | 🔭 Speculative | |
| STM/LTM memory architecture for agents | 🔭 Speculative | `2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md` |

### Test Infrastructure

| Feature | Status | Notes |
|---------|--------|-------|
| `[[test]]` scenarios binary | ✅ Done | `tests/scenarios/main.rs` |
| CI workflow for scenarios binary | ✅ Done | `.github/` |
| TestRegistry (app factory + diagnostics assertion surface) | ✅ Done | `subsystem_diagnostics/diagnostics_observability_and_harness_spec.md` |
| OnceLock capability test-safety (T1) | 📋 Planned | `2026-02-26_test_infrastructure_improvement_plan.md` |
| Test binary split (`test-utils` feature flag) (T2) | 📋 Planned | |
| Deterministic focus ownership tests | ✅ Done | `subsystem_focus/focus_and_region_navigation_spec.md` |
| F6 focus-cycle regression coverage | ✅ Done | `subsystem_focus/focus_and_region_navigation_spec.md` |
| Rolling traversal buffer bounds tests | ✅ Done | `subsystem_history/edge_traversal_spec.md` |
| Compositor chaos mode tests | 📋 Planned | |
| Lasso correctness regression tests | ✅ Done | `canvas/graph_node_edge_interaction_spec.md` |
| UxScenario suite (open_node, focus_cycle, modal_dismiss) | 📋 Planned | |
| Headless scenario execution (CI) | 📋 Planned | `GRAPHSHELL_HEADLESS=1` |

### iOS / Mobile

| Feature | Status | Notes |
|---------|--------|-------|
| iOS port | 🗄️ Archived | `2026-02-19_ios_port_plan.md`; no committed schedule |

---

## VERSO — Web Viewer + Local Collaboration Mod

Verso is the native mod providing web rendering (Servo/Wry) **and** local peer-to-peer collaboration (iroh-based bilateral sync). Two capabilities, one mod identity.

### Web Rendering (Servo)

| Feature | Status | Notes |
|---------|--------|-------|
| Servo integration as web rendering engine | ✅ Done | `viewer/viewer_presentation_and_fallback_spec.md` |
| Servo WebRender GL output | ✅ Done | `aspect_render/render_backend_contract_spec.md` |
| Servo → Graphshell compositor callback bridge | ✅ Done | `render_to_parent` |
| WebView lifecycle management | ✅ Done | create, destroy, focus, blur |
| Protocol handlers: http, https, file | ✅ Done | Via ProtocolRegistry |
| JavaScript execution context | ✅ Done | `viewer/viewer_presentation_and_fallback_spec.md` |
| Cookie and session management | ✅ Done | `viewer/viewer_presentation_and_fallback_spec.md` |
| CSS / font rendering | ✅ Done | `viewer/viewer_presentation_and_fallback_spec.md` |
| DOM inspection capabilities | 📋 Planned | |
| WebDriver integration (existing) | ✅ Done | `webdriver.rs` |
| Wry native window integration (`NativeOverlay` mode) | 📋 Planned | Cross-platform native viewers |
| WebRender → wgpu renderer migration | 📋 Planned | `2026-03-01_webrender_wgpu_renderer_implementation_plan.md` |
| Multiple viewer backend hot-swap | 🔭 Speculative | |
| Verso ModManifest declaration | 📋 Planned | Formal `provides` / `requires` |
| Verso Ed25519 identity (shared with collaboration layer) | 📋 Planned | One keypair for both capabilities |

### Local Collaboration (iroh Bilateral Sync)

This is the **local P2P layer** — private, device-to-device sync. It lives inside Verso, not Verse. Verse is the public community network (Tier 2, long-horizon).

| Feature | Status | Notes |
|---------|--------|-------|
| Ed25519 keypair (shared identity for web and sync) | 📋 Planned | OS keychain; `keyring` crate |
| Trust store (TrustedPeer records: NodeId + role + grants) | 📋 Planned | `IdentityRegistry` |
| iroh QUIC transport | 📋 Planned | Magic Sockets, NAT traversal |
| SyncWorker (ControlPanel-supervised tokio task) | 📋 Planned | Accept loop, peer connections, intent pipeline |
| SyncUnit wire format (rkyv + zstd) | 📋 Planned | Delta batches with VersionVector |
| Version vector (per-peer monotonic sequence numbers) | 📋 Planned | Causal ordering, conflict detection |
| Delta sync (send only changed intents since last contact) | 📋 Planned | |
| Conflict resolution: Last-Write-Wins (metadata) | 📋 Planned | |
| Conflict resolution: Additive (edges) | 📋 Planned | Edges never deleted unilaterally |
| SyncLog (append-only local intent journal) | 📋 Planned | AES-256-GCM at rest |
| QR code pairing ceremony | 📋 Planned | `qrcode` crate |
| Invite link pairing (`verso://pair/{NodeId}/{token}`) | 📋 Planned | |
| mDNS local network discovery | 📋 Planned | `mdns-sd` crate |
| Workspace access grants (ReadOnly / ReadWrite) | 📋 Planned | Per-peer, per-workspace |
| Selective workspace sharing | 📋 Planned | Not all workspaces shared by default |
| Offline-first operation | 📋 Planned | Full functionality without peers connected |
| Opportunistic sync on peer connection | 📋 Planned | |
| Peer presence indicators | 🔭 Speculative | Deferred; `2026-02-25_verse_presence_plan.md` |
| Remote cursors (ghost cursors with label + color) | 🔭 Speculative | Deferred |
| Remote selection highlights | 🔭 Speculative | Deferred |
| Follow mode (camera tracks peer viewport) | 🔭 Speculative | Deferred |
| Peer avatar strip (connection status) | 🔭 Speculative | Deferred |
| Sync status indicator in workbench | 📋 Planned | |
| Sync Panel (pane showing peer list + sync state) | 📋 Planned | |
| Conflict resolution UI (toast + manual) | 📋 Planned | |
| Noise protocol transport authentication | 📋 Planned | |
| Workspace-scoped access control | 📋 Planned | |
| Graceful degradation when peers offline | 📋 Planned | |

---

## VERSE — Public Community Network

Verse is the **public, federated P2P network** for community knowledge sharing. Tier 2 / long-horizon research. Distinct from Verso's local collaboration layer.

### Identity & Community Infrastructure

| Feature | Status | Notes |
|---------|--------|-------|
| libp2p transport (GossipSub community swarms) | 🔭 Speculative | Q3 2026 research validation target |
| Same Ed25519 keypair derives libp2p PeerId (from Verso identity) | 🔭 Speculative | Identity bridge: Verso → Verse |
| Community Manifest (signed definition, governance, visibility policy) | 🔭 Speculative | |
| VerseVisibility: PublicOpen / PublicWithFloor / SemiPrivate / Dark | 🔭 Speculative | |
| RebroadcastLevel: SilentAck → ExistenceBroadcast → ContentRelay → Endorsement | 🔭 Speculative | |
| MembershipRecord (signed consent to community policy) | 🔭 Speculative | |
| VerseGovernance (organizer stake threshold, admin threshold) | 🔭 Speculative | |
| Community moderation buffer and appeals | 🔭 Speculative | |

### Content Addressing & Knowledge Assets

| Feature | Status | Notes |
|---------|--------|-------|
| VerseBlob (content-addressed universal format, BLAKE3 CID) | 🔭 Speculative | BlobTypes: IntentDelta, IndexSegment, Engram, Opaque |
| Report (signed browsing record: URL + text + UDC tags + traversal context) | 🔭 Speculative | Passive indexing unit |
| MediaClip (WARC-format HTTP response archive) | 🔭 Speculative | Forensic fidelity web archiving |
| IndexArtifact (tantivy index segment blob) | 🔭 Speculative | Federated search unit |
| Bitswap content retrieval | 🔭 Speculative | |
| Nostr signaling (bootstrap peer discovery) | 🔭 Speculative | Optional; `protocol:nostr` |

### Federated Search

| Feature | Status | Notes |
|---------|--------|-------|
| Sharded tantivy index segments | 🔭 Speculative | |
| Federated query model (SearchProvider nodes) | 🔭 Speculative | SearchProvider earns tokens per query |
| Community-sourced browsing suggestions | 🔭 Speculative | |
| YaCy-inspired decentralized web index | 🔭 Speculative | `2026-02-23_modern_yacy_gap_analysis.md` |
| Faceted community search | 🔭 Speculative | |

### Economy & Incentives

| Feature | Status | Notes |
|---------|--------|-------|
| Proof of Access (receipt model, per-query) | 🔭 Speculative | Optional; `proof_of_access_ledger_spec.md` |
| Verse fungible tokens (utility token per community) | 🔭 Speculative | |
| Reputation vs token settlement | 🔭 Speculative | |
| Storage bounty economy | 🔭 Speculative | |
| CrawlBounty (curator posts bounty for external web content) | 🔭 Speculative | |
| Crawler role (claims and fulfills CrawlBounty requests) | 🔭 Speculative | |
| Validator role (spot-checks submitted IndexArtifacts) | 🔭 Speculative | |
| Filecoin StakeRecord integration | 🔭 Speculative | |
| Token-weighted governance | 🔭 Speculative | |

### Intelligence & Adaptation (FLora)

| Feature | Status | Notes |
|---------|--------|-------|
| Engram (portable model customization payload) | 🔭 Speculative | `engram_spec.md`; envelope + typed EngramMemory items |
| TransferProfile (engram envelope with metadata + provenance) | 🔭 Speculative | |
| FLora (Federated Learning on Ramified Adaptation) | 🔭 Speculative | Community-shared LoRA adapter layers |
| Adapter weight delta exchange | 🔭 Speculative | |
| Mini-adapter LoRA contribution flow | 🔭 Speculative | |
| Tiered AI orchestration via community adapters | 🔭 Speculative | |
| STM/LTM memory architecture (working context + persistent storage) | 🔭 Speculative | |
| Self-hosted Verse node | 🔭 Speculative | `self_hosted_verse_node_spec.md` |
| Self-hosted model customization | 🔭 Speculative | `self_hosted_model_spec.md` |
| Community knowledge skill layers | 🔭 Speculative | |
| ArchetypeProfile (preset model configuration) | 🔭 Speculative | |

### Verse Participant Roles

| Role | Status | Notes |
|------|--------|-------|
| User (create/publish Reports) | 🔭 Speculative | |
| Seeder / rebroadcaster (host storage) | 🔭 Speculative | |
| Indexer / deduplicator | 🔭 Speculative | |
| Attester / validator | 🔭 Speculative | |
| Curator (create/govern communities) | 🔭 Speculative | |
| Adapter contributor (submit Engrams) | 🔭 Speculative | |
| Storage contributor (earn reputation) | 🔭 Speculative | |
| FLora consumer (use community adapters) | 🔭 Speculative | |
| Self-hosted node operator | 🔭 Speculative | |

---

## FEATURE COUNT SUMMARY

| System | ✅ Done | 🔨 Active | 📋 Planned | 🔭 Speculative | 🗄️ Archived |
|--------|---------|-----------|-----------|---------------|------------|
| Graphshell | ~70 | ~8 | ~90 | ~35 | 1 |
| Verso (web) | ~12 | 0 | ~8 | ~2 | 0 |
| Verso (local collab / iroh) | 0 | 0 | ~20 | ~6 | 0 |
| Verse (public network) | 0 | 0 | 0 | ~45 | 0 |
| **Total** | **~82** | **~8** | **~118** | **~88** | **1** |

---

## NOTES ON BRANDING

- **Verso** owns both the Servo/Wry web viewer and the local (iroh-based) bilateral device sync layer. One mod, two capability families, one Ed25519 identity. The `verso://pair/` URL scheme signals this ownership.
- **Verse** is solely the public community network (Tier 2, long-horizon). It builds on the same Ed25519 keypair as Verso (identity bridge) but is a separate network layer with a separate scope, timeline, and governance model.
- The name alignment: *Verso* (local, private, fast) and *Verse* (public, federated, community) form a natural pair — private collaboration vs public network.

---

## ISSUE CATEGORIZATION FOR WGPU MIGRATION READINESS

This section maps open GitHub issues into migration timing buckets aligned with:

- `2026-03-01_ux_migration_design_spec.md`
- `subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `2026-03-01_ux_migration_feature_spec_coverage_matrix.md`
- `2026-03-01_ux_migration_lifecycle_audit_register.md`

### Practical cutline (canonical)

- If a feature changes interaction semantics or contract invariants, it is **Pre-WGPU required**.
- If a feature changes visual sophistication, optional modes, or speculative capability, it is **Post-WGPU preferred**.

### Stale-but-relevant reinterpretations (canonical)

- **"Magnetic zones"** are interpreted as **frame-affinity organizational behavior** under graph-first frame semantics (`workbench/graph_first_frame_semantics_spec.md`).
- **Legacy context menu as a primary surface** remains deprecated; contextual invocation is routed through Command Palette mode.
- **Edge semantics remain event-stream-first**: `Traversal` events are primary and projected into durable edge state (`EdgePayload`), not old edge-type-only framing.

### Pre-WGPU closure checklist (canonical gate)

- [ ] Event dispatch contract closure: `#261`, `#269`.
- [x] Radial geometry/overflow contract closure: `#263`, `#270`.
- [ ] Canvas interaction invariants closure (selection/lasso/zoom/edge focus): `#271`, `#173`, `#185`, `#102`, `#104`, `#101`, `#103`.
- [ ] Viewer fallback/degraded-state clarity closure: `#188`, `#162`.
- [ ] UxHarness critical-path evidence closure: `#251`, `#257`, `#273`.
- [ ] UxTree staged authority gate closure: `#272`.
- [ ] Terminology reinterpretation pass complete in affected docs:
	- "Magnetic zones" → frame-affinity behavior.
	- Context-menu-primary references removed/reframed to Command Palette contextual mode.
	- Edge semantics phrased event-stream-first.

### Pre-WGPU required (UX contract closure)

- Event dispatch: `#261`, `#269`
- Command/radial contract closure: `#263`, `#270`
- Canvas interaction invariants: `#271`, `#173`, `#185`, `#102`, `#104`, `#101`, `#103`
- Viewer fallback clarity: `#188`, `#162`
- UxHarness migration gate: `#251`, `#257`, `#273`
- UxTree staged authority roadmap: `#272`

### Post-WGPU preferred (defer until renderer switch stabilizes)

- Readability adaptation deepening and advanced layout heuristics (`#265` follow-on scope)
- Advanced radial/gesture expert behaviors after baseline radial contract closure
- Speculative interaction R&D from inventory (SketchLay, DOI/fisheye, broader progressive lenses)

### Parallel migration lanes (required overall, separate from UX closure)

- `#180`, `#181`, `#182`, `#183`, `#184`, `#245`

### Missing issues added in this pass

- `#269` Phase A supplement: UxTree event dispatch canonical-spec closure
- `#270` Phase C supplement: radial geometry and overflow contract closure
- `#271` Pre-WGPU canvas interaction invariants closure
- `#272` UxTree convergence roadmap: staged authority gates
- `#273` Pre-WGPU UxHarness critical-path gate
