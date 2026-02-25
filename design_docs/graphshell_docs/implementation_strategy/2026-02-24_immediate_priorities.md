# Immediate Priorities Register, Forgotten Concepts, and Quick Wins (2026-02-24)

**Status**: Active / Execution (revised 2026-02-25)
**Context**: Consolidated execution register synthesized from current implementation strategy, research, architecture, and roadmap docs.

**Audit basis (2026-02-25 review)**:
- `2026-02-22_registry_layer_plan.md`
- `2026-02-22_multi_graph_pane_plan.md` (scope expanded in paired doc sync to pane-hosted multi-view architecture)
- `2026-02-24_layout_behaviors_plan.md`
- `2026-02-24_performance_tuning_plan.md`
- `2026-02-24_control_ui_ux_plan.md`
- `2026-02-24_spatial_accessibility_plan.md`
- `2026-02-24_universal_content_model_plan.md`
- `2026-02-23_wry_integration_strategy.md`
- `2026-02-20_edge_traversal_impl_plan.md`
- `2026-02-18_graph_ux_research_report.md`
- `2026-02-24_interaction_and_semantic_design_schemes.md`
- `2026-02-24_diagnostics_research.md`
- `2026-02-24_visual_tombstones_research.md`
- `2026-02-24_spatial_accessibility_research.md`
- `GRAPHSHELL_AS_BROWSER.md`
- `IMPLEMENTATION_ROADMAP.md`
- `design_docs/PROJECT_DESCRIPTION.md`

---

## 0. Latest Checkpoint Delta (Code + Doc Audit)

### Code checkpoint (2026-02-24)

- Registry Phase 6.2 boundary hardening advanced: workspace-only reducer path extracted and covered by boundary tests.
- Registry Phase 6.3 single-write-path slices closed for runtime/persistence: direct persistence topology writes were converged to graph-owned helpers, runtime contract coverage now includes persistence runtime sections, and targeted boundary tests are green.
- Registry Phase 6.4 started with a mechanical host subtree move: `running_app_state.rs` and `window.rs` are now canonical under `shell/desktop/host/` with root re-export shims retained during transition.
- Registry Phase 6.4 import canonicalization advanced beyond `shell/desktop/**`: remaining root-shim host imports in `egl/app.rs` and `webdriver.rs` were moved to canonical `shell/desktop/host/*` paths; shim files remain in place for transition compatibility.
- Phase 5 sync UI/action path advanced: pair-by-code decode, async discovery enqueue path, and Phase 5 diagnostics channel + invariant contracts are now in code with passing targeted tests.
- Compile baseline remains green (`cargo check`), warning baseline unchanged.

### Doc audit delta (2026-02-25)

- Immediate-priority list promoted from a loose synthesis into a source-linked 10/10/10 register.
- Multi-pane planning is now treated as a **pane-hosted multi-view problem** (graph + viewer + tool panes), not only "multi-graph."
- Several low-effort, high-impact items from UX and diagnostics research were missing from the active queue and are now explicitly tracked.

---

## 1. Top 10 Priority Tasks (Strategic Blockers / Sequencing Drivers)

These are ordered for execution impact, not desirability. Items 1-4 are closure work that reduces migration risk before broader feature acceleration.

| Rank | Priority Task | Why Now | Primary Source Docs | Next Slice / Done Gate |
| --- | --- | --- | --- | --- |
| 1 | **Registry Phase 5.4 done-gate closure (Verse delta sync)** | Remaining Phase 5 Tier 1 credibility gap; diagnostics + harness coverage are partially implemented but not closed. | `2026-02-22_registry_layer_plan.md`, this doc §5.1 | Add `verse_delta_sync_basic`, emit conflict diagnostics channels, targeted tests + `cargo check` green. |
| 2 | **Registry Phase 5.5 done-gate closure (workspace access control)** | Access control behavior exists conceptually but lacks required end-to-end harness and denial-path proof. | `2026-02-22_registry_layer_plan.md`, this doc §5.2 | Add `verse_access_control` harness, deny-path diagnostics assertions, focused revoke/forget tests. |
| ~~3~~ | ~~Registry Phase 6.4 canonical import/path closure~~ | ~~Complete (2026-02-25)~~ | — | ~~Done~~ |
| ~~4~~ | ~~Registry Phase 6.5 shim removal + final boundary lock + doc path sync~~ | ~~Complete (2026-02-25): root shims deleted, mutators tightened to `pub(crate)`, docs updated~~ | — | ~~Done~~ |
| 3 | **Pane-hosted multi-view architecture (generalize "multi-graph pane")** | Multiple current plans assume panes host graph views, Servo/Wry viewers, and tool surfaces; the plan needs one canonical pane-view model first. | `2026-02-22_multi_graph_pane_plan.md`, `GRAPHSHELL_AS_BROWSER.md`, `2026-02-23_wry_integration_strategy.md`, `2026-02-24_universal_content_model_plan.md` | Define pane view capsule/descriptor model and lifecycle rules; preserve `GraphViewState` as graph-pane payload, not universal pane state. |
| 4 | **Graph multi-view implementation (GraphViewId, per-pane Lens, Canonical/Divergent)** | Backend types exist conceptually; UI and reducer integration are still missing. Unlocks Lens workflows and layout experimentation. | `2026-02-22_multi_graph_pane_plan.md`, `2026-02-24_interaction_and_semantic_design_schemes.md` | Hard-break singular camera/egui state -> per-view map; `TileKind::Graph(GraphViewId)` + split/lens UI working. |
| 5 | **Universal content model foundation (Steps 1-3)** | Required before native viewers, consistent viewer selection, and robust Wry/Servo co-existence. | `2026-02-24_universal_content_model_plan.md`, `GRAPHSHELL_AS_BROWSER.md` | `Node.mime_hint` + `address_kind`, `ViewerRegistry::select_for`, baseline `viewer:plaintext` embedded renderer. |
| 6 | **Wry backend foundation (Steps 1-5)** | Windows compatibility path is a major practical blocker; depends on viewer contract and pane/overlay rules being explicit. | `2026-02-23_wry_integration_strategy.md`, `2026-02-24_universal_content_model_plan.md` | Feature gate, `WryManager`, `WryViewer`, overlay tracking, lifecycle reconcile integration. |
| 7 | **Control UI/UX consolidation (ActionRegistry-driven command surfaces)** | Input surface fragmentation in `render/mod.rs` is a readability and maintainability drag; blocks gamepad-ready control UX. | `2026-02-24_control_ui_ux_plan.md`, `2026-02-23_graph_interaction_consistency_plan.md` | Extract radial/palette modules; route both through `ActionRegistry::list_actions_for_context`; unify command palette scopes. |
| 8 | **Scale + accessibility baseline (Viewport Culling + WebView A11y Bridge)** | Performance and accessibility each have implementation-ready Phase 1 work that should start before feature breadth increases complexity. | `2026-02-24_performance_tuning_plan.md`, `2026-02-24_spatial_accessibility_plan.md` | Land viewport culling policy + implementation slice; land WebView accessibility bridge critical fix. |

### Near-Miss (Implementation-Ready but Not Top 10)

- **Bookmarks/History Import (`ImportWizardMod`)** remains implementation-ready and valuable (`2026-02-11_bookmarks_history_import_plan.md`), but it is not a current architecture blocker.
- **Diagnostics pane expansion** is high leverage (`2026-02-24_diagnostics_research.md`) and appears in Quick Wins / Forgotten Concepts below; it should be pulled forward if debugging velocity drops.

---

## 2. Top 10 Forgotten Concepts for Adoption (Vision / Research Ideas Missing from Active Queue)

These are not "do now" items. They are concepts that should be explicitly adopted into planning so they do not disappear between migration and feature work.

| Rank | Forgotten Concept | Adoption Value | Source Docs | Adoption Trigger |
| --- | --- | --- | --- | --- |
| 1 | **Visual Tombstones (ghost nodes/edges after deletion)** | Preserves structural memory and reduces disorientation after destructive edits. | `2026-02-24_visual_tombstones_research.md`, `2026-02-25_visual_tombstones_plan.md` ✅ adopted | After traversal/history UI and deletion UX are stable. |
| 2 | **Temporal Navigation / Time-Travel Preview** | Makes traversal history and deterministic intent log materially useful to users (not just diagnostics). | `2026-02-20_edge_traversal_impl_plan.md` (Stage F), `GRAPHSHELL_AS_BROWSER.md`, `2026-02-18_graph_ux_research_report.md` | After Stage E History Manager closure and preview-mode effect isolation hardening. |
| 3 | **Collaborative Presence (ghost cursors, remote selection, follow mode)** | Turns Verse sync from data sync into shared work. | `2026-02-18_graph_ux_research_report.md` §15.2, `GRAPHSHELL_AS_BROWSER.md`, Verse vision docs cited there | After Phase 5 done gates and identity/presence semantics are stable. |
| 4 | **Semantic Fisheye + DOI (focus+context without geometric distortion)** | High-value readability improvement for dense graphs; preserves mental map while surfacing relevance. | `2026-02-18_graph_ux_research_report.md` §§13.2, 14.8, 14.9 | After basic LOD and viewport culling are in place. |
| 5 | **Magnetic Zones / Group-in-a-Box / Query-to-Zone** | Adds spatial organization as a first-class workflow, not just emergent physics. | `2026-02-24_layout_behaviors_plan.md` Phase 3, `2026-02-18_graph_ux_research_report.md` §13.1 | After layout injection hook and zone persistence rules are specified. |
| 6 | **Graph Reader ("Room" + "Map" linearization) and list-view fallback** | Critical accessibility concept beyond the initial webview bridge; gives non-visual users graph comprehension. | `2026-02-24_spatial_accessibility_research.md`, `2026-02-24_spatial_accessibility_plan.md` Phase 2 | After Phase 1 WebView Bridge lands. |
| 7 | **Unified Omnibar (URL + graph search + web search heuristics)** | Core browser differentiator; unifies navigation and retrieval. | `GRAPHSHELL_AS_BROWSER.md` §7, `2026-02-18_graph_ux_research_report.md` §15.4 | After command palette/input routing stabilization. |
| 8 | **Progressive Lenses + Lens/Physics binding policy** | Makes Lens abstraction feel native and semantic, not static presets. | `2026-02-24_interaction_and_semantic_design_schemes.md`, `2026-02-24_physics_engine_extensibility_plan.md` (lens-physics binding preference) | After Lens resolution is active runtime path and physics presets are distinct in behavior. |
| 9 | **2D↔3D Hotswitch with `ViewDimension` and position parity** | Named first-class vision feature; fits the new per-view architecture and future Rapier/3D work. | `2026-02-24_physics_engine_extensibility_plan.md`, `design_docs/PROJECT_DESCRIPTION.md` | After pane-hosted view model and `GraphViewState` are stable. |
| 10 | **Interactive HTML Export (self-contained graph artifact)** | Strong shareability and offline review workflow; distinctive output mode. | `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md` (archived concept) | After viewer/content model and export-safe snapshot shape are defined. |

---

## 3. Top 10 Quickest Improvements (Low-Effort / High-Leverage Slices)

These are intentionally scoped to small slices that can ship independently without waiting for larger architecture work.

| Rank | Quick Improvement | Why It Pays Off | Primary Source Docs |
| --- | --- | --- | --- |
| 1 | **Extract `desktop/radial_menu.rs` from `render/mod.rs`** | Reduces render module sprawl and unblocks control UI redesign without behavior changes. | `2026-02-24_control_ui_ux_plan.md` |
| 2 | **Extract `desktop/command_palette.rs` from `render/mod.rs`** | Same benefit as #1; clarifies ownership for unified command surface work. | `2026-02-24_control_ui_ux_plan.md` |
| 3 | **Reheat physics on `AddNode` / `AddEdge`** | Fixes "dead graph" feel immediately when physics is paused. | `2026-02-24_layout_behaviors_plan.md` §1.1, `2026-02-18_graph_ux_research_report.md` §5.3 |
| 4 | **Spawn new nodes near semantic parent (parent + jitter)** | Improves mental-map preservation and reduces convergence churn. | `2026-02-24_layout_behaviors_plan.md` §1.2, `2026-02-18_graph_ux_research_report.md` §§2.1, 2.6 |
| 5 | **Fix `WebViewUrlChanged` prior-URL ordering in traversal append path** | Prevents incorrect traversal records and future temporal-navigation corruption. | `2026-02-20_edge_traversal_impl_plan.md`, `2026-02-20_edge_traversal_model_research.md` |
| 6 | **Wire `Ctrl+Click` multi-select in graph pane** | Tiny code slice with immediate UX gain; unlocks group operations expectations. | `2026-02-18_graph_ux_research_report.md` §§1.3, 6.3 |
| 7 | **Add semantic container tab titles (`Split ↔`, `Split ↕`, `Tab Group`, `Grid`)** | Converts "looks broken" tile labels into teachable architecture UI. | `2026-02-23_graph_interaction_consistency_plan.md` Phase 4 |
| 8 | **Add zoom-adaptive label LOD thresholds (hide/domain/full)** | Immediate clarity and performance win at low zoom, low implementation risk. | `2026-02-24_performance_tuning_plan.md` Phase 2.1, `2026-02-18_graph_ux_research_report.md` §7.3 |
| 9 | **Add `ChannelSeverity` to diagnostics channel descriptors** | Small schema extension that unlocks better pane prioritization and health summary. | `2026-02-24_diagnostics_research.md` §4.6, §7 |
| 10 | **Add/confirm `CanvasRegistry` culling + LOD policy toggles** | Minimal schema/policy work that unblocks performance slices and keeps behavior policy-driven. | `2026-02-24_performance_tuning_plan.md`, `2026-02-22_registry_layer_plan.md` |

### Quick Win Notes

- Items 1-2 pair naturally and should be landed together if the extraction is mechanical.
- Items 3-5 are correctness/feel fixes and should not wait for full layout/traversal phases.
- Items 9-10 are low-churn infrastructure improvements that improve future implementation discipline.

---

## 4. Recommended Execution Sequence (2026-02-25 Refresh)

### Wave A: Close Migration Done Gates (Highest Risk Reduction)

1. Registry Phase 5.4 closure (delta sync harness + conflict diagnostics)
2. Registry Phase 5.5 closure (access control harness + deny-path coverage)
3. Registry Phase 6.4 canonical imports/path cleanup
4. Registry Phase 6.5 shim removal + final boundary lock + doc path refresh

### Wave B: Establish Pane/View and Viewer Foundations

1. Pane-hosted multi-view architecture doc+type sync (graph/viewer/tool pane model)
2. Graph multi-view implementation (`GraphViewId`, per-view state, split/lens UI)
3. Universal content Steps 1-3 (data model + viewer selection + plaintext baseline)
4. Wry Steps 1-5 (feature gate through lifecycle integration)

### Wave C: UX Consolidation, Scale, and Accessibility Baselines

1. Control UI/UX extraction + ActionRegistry routing
2. Viewport culling + LOD policy activation
3. WebView accessibility bridge (Phase 1 critical fix)
4. Pull from Quick Wins list continuously between larger slices

---

## 5. Registry Plan Closure Backlog (Audited 2026-02-24, retained 2026-02-25)

This is the strict closure checklist derived from the current `2026-02-22_registry_layer_plan.md` state and code/test audit.

### 5.1 Phase 5.4 — Delta Sync Done-Gate Closure

1. **Add missing harness scenario `verse_delta_sync_basic`**
   - Create scenario under `desktop/tests/scenarios/` and include it in `desktop/tests/scenarios/mod.rs`.
   - Validate two-instance flow: node created on A appears on B within 5 seconds.
   - Validate concurrent rename conflict resolves deterministically (LWW behavior) without crash.

2. **Close conflict diagnostics gap in runtime code**
   - Implement emission paths for `verse.sync.conflict_detected` and `verse.sync.conflict_resolved` where conflict logic runs.
   - Ensure channels are seeded/registered in diagnostics registry defaults and covered by contract tests.

3. **Acceptance checks (must all pass)**
   - `cargo test verse_delta_sync_basic`
   - Diagnostics assertions include `unit_sent`, `unit_received`, `intent_applied`, `conflict_detected`, `conflict_resolved`.
   - `cargo check` remains green.

### 5.2 Phase 5.5 — Workspace Access Control Done-Gate Closure

1. **Add missing harness scenario `verse_access_control`**
   - Validate grant matrix for `ReadOnly` and `ReadWrite` workspace permissions.
   - Confirm read-only peer receives remote updates but local mutating intents for that workspace are rejected.

2. **Harden access-denied behavior and coverage**
   - Ensure inbound non-granted workspace sync always emits `verse.sync.access_denied` and does not mutate graph state.
   - Add focused tests for deny paths and revoke/forget flows.

3. **Acceptance checks (must all pass)**
   - `cargo test verse_access_control`
   - Access-control path emits `verse.sync.access_denied` deterministically.
   - `cargo check` remains green.

### 5.3 Phase 6.4 — Filesystem/Import Canonicalization Closure

1. **Finish canonical imports away from root compatibility paths**
   - Remove remaining `crate::persistence::*` consumers by migrating to `crate::services::persistence::*` (and `types` submodule path equivalents) in runtime/UI/tests where appropriate.
   - Continue mechanical path migration slices per subtree with compile validation after each slice.

2. **Align test/harness imports during each move slice**
   - Update `desktop/tests/scenarios/*` imports in the same commit as each path migration.
   - Keep boundary/seam contracts green after each move.

3. **Acceptance checks (must all pass)**
   - `cargo check` after each subtree slice.
   - `cargo test contract_runtime_layers_do_not_call_graph_topology_mutators_directly`
   - `cargo test servo_callbacks_only_enqueue_events`

### 5.4 Phase 6.5 — Transition Shim Removal & Final Boundary Lock

1. **Delete all temporary root re-export shims**
   - Remove shim files/usages for: `running_app_state.rs`, `window.rs`, `search.rs`, `persistence/mod.rs` (root compatibility layer).
   - Update all callsites to canonical module paths before deleting shims.

2. **Enforce single-write-path visibility target**
   - Tighten graph topology mutator visibility in `model/graph/mod.rs` to the planned boundary level and resolve resulting callers through reducer-owned paths.

3. **Update docs to canonical paths**
   - Refresh strategy/architecture map references that still point at shim or pre-move paths.

4. **Acceptance checks (must all pass)**
   - No transition shims remain at crate root.
   - Full suite passes: `cargo test` and `cargo check`.
   - Registry done-gate language in strategy docs matches repository reality.

### 5.5 Immediate Next Sequence (Recommended Order)

1. Implement `verse_delta_sync_basic` + conflict diagnostics channels.
2. Implement `verse_access_control` harness and deny-path assertions.
3. Complete remaining 6.4 import canonicalization (`persistence` path cleanup).
4. Execute 6.5 shim removal in one controlled slice with full-suite validation.
