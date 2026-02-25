# Planning Register, Backlog Ticket Stubs, and Copilot Implementation Guides (Consolidated)

**Status**: Active / Canonical (consolidated 2026-02-25)
**Purpose**: Single source for execution priorities, issue-ready backlog stubs, and Copilot implementation guidance.

## Contents

1. Immediate Priorities Register (10/10/10)
2. Backlog Ticket Stubs
3. Copilot Implementation Guides

---

## 1. Immediate Priorities Register (10/10/10)

_Source file before consolidation: `2026-02-24_immediate_priorities.md`_


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
| 1 | **Visual Tombstones (ghost nodes/edges after deletion)** | Preserves structural memory and reduces disorientation after destructive edits. | `2026-02-24_visual_tombstones_research.md` | After traversal/history UI and deletion UX are stable. |
| 2 | **Temporal Navigation / Time-Travel Preview** | Makes traversal history and deterministic intent log materially useful to users (not just diagnostics). | `2026-02-20_edge_traversal_impl_plan.md` (Stage F), `GRAPHSHELL_AS_BROWSER.md`, `2026-02-18_graph_ux_research_report.md` | After Stage E History Manager closure and preview-mode effect isolation hardening. |
| 3 | **Collaborative Presence (ghost cursors, remote selection, follow mode)** | Turns Verse sync from data sync into shared work. | `2026-02-18_graph_ux_research_report.md` §15.2, `GRAPHSHELL_AS_BROWSER.md`, Verse vision docs cited there | After Phase 5 done gates and identity/presence semantics are stable. |
| 4 | **Semantic Fisheye + DOI (focus+context without geometric distortion)** | High-value readability improvement for dense graphs; preserves mental map while surfacing relevance. | `2026-02-18_graph_ux_research_report.md` §§13.2, 14.8, 14.9 | After basic LOD and viewport culling are in place. |
| 5 | **Magnetic Zones / Group-in-a-Box / Query-to-Zone** | Adds spatial organization as a first-class workflow, not just emergent physics. | `2026-02-24_layout_behaviors_plan.md` Phase 3 (expanded with persistence scope, interaction model, and implementation sequence), `2026-02-18_graph_ux_research_report.md` §13.1 | **Prerequisites now documented** in `layout_behaviors_plan.md` §3.0–3.5. Implementation blocked on: (1) layout injection hook (Phase 2), (2) Canonical/Divergent scope settlement. Trigger: when both blockers are resolved, execute implementation sequence in §3.5. |
| 6 | **Graph Reader ("Room" + "Map" linearization) and list-view fallback** | Critical accessibility concept beyond the initial webview bridge; gives non-visual users graph comprehension. | `2026-02-24_spatial_accessibility_research.md`, `2026-02-24_spatial_accessibility_plan.md` Phase 2 | After Phase 1 WebView Bridge lands. |
| 7 | **Unified Omnibar (URL + graph search + web search heuristics)** | Core browser differentiator; unifies navigation and retrieval. | `GRAPHSHELL_AS_BROWSER.md` §7, `2026-02-18_graph_ux_research_report.md` §15.4 | After command palette/input routing stabilization. |
| 8 | **Progressive Lenses + Lens/Physics binding policy** | Makes Lens abstraction feel native and semantic, not static presets. | `2026-02-24_interaction_and_semantic_design_schemes.md`, `2026-02-24_physics_engine_extensibility_plan.md` (lens-physics binding preference) | After Lens resolution is active runtime path and physics presets are distinct in behavior. |
| 9 | **2D↔3D Hotswitch with `ViewDimension` and position parity** | Named first-class vision feature; fits the new per-view architecture and future Rapier/3D work. | `2026-02-24_physics_engine_extensibility_plan.md`, `design_docs/PROJECT_DESCRIPTION.md` | After pane-hosted view model and `GraphViewState` are stable. |
| 10 | **Interactive HTML Export (self-contained graph artifact)** | Strong shareability and offline review workflow; distinctive output mode. | `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md` (archived concept) | After viewer/content model and export-safe snapshot shape are defined. |

Appended adoption note (preserved from PR `#55`, pending table refactor):
- Visual Tombstones (`Rank 1`) is now backed by `design_docs/graphshell_docs/implementation_strategy/2026-02-25_visual_tombstones_plan.md` and should be treated as `✅ adopted` in future table cleanup.

Appended adoption note (preserved from PR `#56`, pending table refactor):
- Temporal Navigation / Time-Travel Preview (`Rank 2`) should be treated as `✅ adopted` and promoted to a tracked staged backlog item via `design_docs/graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md` Stage F.

Appended staged backlog summary (preserved from PR `#56`, pending section refactor):
- **Stage F: Temporal Navigation (Tracked Staged Backlog Item)** — Deferred until Stage E History Manager maturity (tiered storage, dissolution correctness, and stable WAL shape).
- Deliverables preserved from PR summary: timeline index, `replay_to_timestamp(...)`, detached preview graph state, timeline slider/return-to-present UI, and preview ghost rendering.
- Preview-mode effect isolation contract (preserved): no WAL writes, no webview lifecycle mutations, no live graph mutations, no persistence side effects, and clean return-to-present with no preview-state leakage.
- Designated enforcement point preserved: `desktop/gui_frame.rs` effect-suppression gates.
- Preserved non-goals: collaborative replay, undo/redo replacement, scrubber polish fidelity, timeline snapshot export.

Appended adoption note (preserved from PR `#58`, pending table refactor):
- Semantic Fisheye + DOI (`Rank 4`) is now backed by `design_docs/graphshell_docs/implementation_strategy/2026-02-25_doi_fisheye_plan.md` and should be linked from the forgotten-concepts table during later cleanup.

Appended adoption note (preserved from PR `#60`, pending table refactor):
- Progressive Lenses + Lens/Physics Binding Policy (`Rank 8`) now has a strategy doc: `design_docs/graphshell_docs/implementation_strategy/2026-02-25_progressive_lens_and_physics_binding_plan.md`; treat the concept as policy-specified (implementation still blocked on runtime prerequisites).

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

---

## 2. Backlog Ticket Stubs

_Source file before consolidation: `2026-02-25_backlog_ticket_stubs.md`_


**Status**: Draft / Ready for issue tracker import
**Source register**: `2026-02-24_immediate_priorities.md`
**Purpose**: Turn the 10/10/10 register into concrete ticket stubs with scope, dependencies, and acceptance targets.

---

## 1. Priority Task Ticket Stubs (Top 10)

**GitHub issue status legend (used below)**:
- `Ready`: can be picked up now
- `Queued`: should wait for sequencing, but not hard-blocked
- `Deferred (blocked)`: prerequisites not yet met; do not start until listed blockers are complete

### P1. Registry Phase 5.4 Done-Gate Closure (Verse Delta Sync)

- **Type**: Feature / Test-harness / Diagnostics
- **GitHub Issue Title**: `Registry: close Phase 5.4 Verse delta-sync done gate (harness + conflict diagnostics)`
- **GitHub Labels**: `priority/top10`, `registry`, `verse`, `testing`, `diagnostics`, `architecture`
- **GitHub Milestone**: `Wave A / Registry closure`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**: `None`
- **Backlog Lane**: `Now`
- **Milestone**: `Wave A / Registry closure`
- **Effort**: `M` (2-4 focused slices)
- **Goal**: Close Phase 5.4 done gate with `verse_delta_sync_basic` harness coverage and conflict diagnostics emission.
- **Scope**:
  - Add `verse_delta_sync_basic` scenario.
  - Cover two-instance sync and deterministic rename conflict resolution.
  - Emit `verse.sync.conflict_detected` and `verse.sync.conflict_resolved`.
- **Out of Scope**:
  - Verse Tier 2 protocol work
  - Presence/collaboration UI
- **Dependencies**: Existing Phase 5 diagnostics channel registration baseline.
- **Subtasks (recommended split)**:
  - `P1.a` Add `verse_delta_sync_basic` scenario shell + wiring.
  - `P1.b` Implement conflict diagnostics emission paths.
  - `P1.c` Add assertions and stabilize timing/flake behavior.
- **Definition of Ready**:
  - Conflict-resolution callsite(s) identified in runtime path.
  - Harness pattern for two-instance Verse scenarios confirmed.
- **Acceptance**:
  - `cargo test verse_delta_sync_basic`
  - Diagnostics assertions include required conflict channels.
  - `cargo check` green.
- **Definition of Done**:
  - Phase 5.4 done-gate language in registry plan is true in code/tests.
  - No fallback/manual verification caveats remain for conflict diagnostics.
- **Review Check (comprehension)**:
  - Conflicts are a runtime behavior problem plus diagnostics observability gap, not a UI problem.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:143`

### P2. Registry Phase 5.5 Done-Gate Closure (Workspace Access Control)

- **Type**: Feature / Test-harness / Security behavior
- **GitHub Issue Title**: `Registry: close Phase 5.5 Verse workspace access-control done gate`
- **GitHub Labels**: `priority/top10`, `registry`, `verse`, `security`, `testing`, `diagnostics`
- **GitHub Milestone**: `Wave A / Registry closure`
- **GitHub Issue Status**: `Queued` (sequenced after `P1`, not strictly blocked)
- **Blocking Prerequisites**:
  - `None (hard blockers)`
- **Sequencing Prerequisites (recommended first)**:
  - `P1` (shared Verse harness context and diagnostics patterns)
- **Backlog Lane**: `Now`
- **Milestone**: `Wave A / Registry closure`
- **Effort**: `M` (2-4 focused slices)
- **Goal**: Close Phase 5.5 done gate with access-control harness coverage and deny-path diagnostics.
- **Scope**:
  - Add `verse_access_control` scenario.
  - Validate `ReadOnly`/`ReadWrite` matrix behavior.
  - Ensure denied sync emits `verse.sync.access_denied` and does not mutate graph state.
- **Out of Scope**:
  - Full permission UX/editor
  - Multi-peer trust/policy redesign
- **Dependencies**: P1 recommended first (shared Verse harness context).
- **Subtasks (recommended split)**:
  - `P2.a` Harness scenario for RO/RW matrix.
  - `P2.b` Deny-path runtime hardening + diagnostics emission.
  - `P2.c` Focused tests for revoke/forget/ungranted inbound sync.
- **Definition of Ready**:
  - Permission model enums/paths and deny hooks are located.
  - Required diagnostics channel exists in defaults or tracked gap is explicit.
- **Acceptance**:
  - `cargo test verse_access_control`
  - Deterministic deny diagnostics emission.
  - `cargo check` green.
- **Definition of Done**:
  - Access-denied paths are behaviorally enforced and test-proven.
  - Phase 5.5 done-gate statements can be updated without caveats.
- **Review Check (comprehension)**:
  - This is both authorization correctness and sync-state non-mutation protection.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:159`

### P3. Registry Phase 6.4 Canonical Import/Path Closure

- **Type**: Refactor / Architecture migration
- **GitHub Issue Title**: `Registry: finish Phase 6.4 canonical imports and path migration cleanup`
- **GitHub Labels**: `priority/top10`, `registry`, `refactor`, `architecture`, `migration`
- **GitHub Milestone**: `Wave A / Registry closure`
- **GitHub Issue Status**: `Closed (completed 2026-02-25)`
- **Blocking Prerequisites**: `None`
- **Backlog Lane**: `Now`
- **Milestone**: `Wave A / Registry closure`
- **Effort**: `M-L` (depends on remaining churn)
- **Goal**: Complete filesystem/import canonicalization (especially remaining `persistence` path cleanup) with test continuity.
- **Scope**:
  - Migrate remaining root compatibility imports to canonical service paths.
  - Update test/harness imports in same slices.
  - Keep seam/boundary contracts green after each subtree move.
- **Out of Scope**:
  - Logic rewrites in moved files
  - Shim removal (Phase 6.5)
- **Dependencies**: None; can proceed in parallel with P1/P2 if capacity allows.
- **Subtasks (recommended split)**:
  - `P3.a` Audit remaining root compat imports (`persistence`, `search`, etc.).
  - `P3.b` Mechanical subtree migration slices with compile checks.
  - `P3.c` Test/harness import cleanup paired with each slice.
- **Definition of Ready**:
  - Remaining import violations are enumerated.
  - Contract tests used as migration guardrails are known and runnable.
- **Acceptance**:
  - `cargo check` after each subtree slice
  - Boundary contract tests pass
- **Definition of Done**:
  - No remaining targeted 6.4 import/path compatibility consumers in scoped areas.
  - Repository is ready for shim deletion without hidden path churn.
- **Review Check (comprehension)**:
  - 6.4 is a mechanical migration discipline task; logic changes should be isolated elsewhere.
- **Status update (2026-02-25)**:
  - Closed via `mark-ik/graphshell#3`.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:174`

### P4. Registry Phase 6.5 Shim Removal + Boundary Lock + Doc Path Sync

- **Type**: Refactor / Cleanup / Docs sync
- **GitHub Issue Title**: `Registry: execute Phase 6.5 shim removal and final boundary lock`
- **GitHub Labels**: `priority/top10`, `registry`, `refactor`, `architecture`, `docs`
- **GitHub Milestone**: `Wave A / Registry closure`
- **GitHub Issue Status**: `Closed (completed 2026-02-25)`
- **Blocking Prerequisites**:
  - `P3` complete (Phase 6.4 canonical imports/path closure)
- **Backlog Lane**: `Now`
- **Milestone**: `Wave A / Registry closure`
- **Effort**: `M`
- **Goal**: Remove transition shims and finalize single-write-path boundary enforcement.
- **Scope**:
  - Delete root re-export shims.
  - Tighten graph mutator visibility.
  - Refresh docs to canonical paths.
- **Out of Scope**:
  - New feature work
  - Large module moves not required by shim removal
- **Dependencies**: P3 complete.
- **Subtasks (recommended split)**:
  - `P4.a` Shim-usage audit + callsite updates.
  - `P4.b` Delete shims and compile fix fallout.
  - `P4.c` Mutator visibility tightening and caller reroutes.
  - `P4.d` Doc path synchronization.
- **Definition of Ready**:
  - Shim inventory confirmed.
  - Final boundary target visibility is unambiguous.
- **Acceptance**:
  - No transition shims remain.
  - `cargo test` + `cargo check` green.
  - Strategy docs match repo reality.
- **Definition of Done**:
  - Compiler enforces single-write-path boundary as planned.
  - Strategy docs no longer describe transitional structure as current.
- **Review Check (comprehension)**:
  - This is the closure task that converts "migration in progress" into enforceable architecture.
- **Status update (2026-02-25)**:
  - Closed via `mark-ik/graphshell#4`.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:189`

### P5. Pane-Hosted Multi-View Architecture (Doc+Type Foundation)

- **Type**: Architecture / Design-to-implementation prep
- **GitHub Issue Title**: `Workbench: establish pane-hosted view payload architecture (graph/node/tool panes)`
- **GitHub Labels**: `priority/top10`, `architecture`, `workbench`, `pane-system`, `design`
- **GitHub Milestone**: `Wave B / Pane-view foundation`
- **GitHub Issue Status**: `Closed (completed 2026-02-25)`
- **Blocking Prerequisites**:
  - `None (hard blockers)`
- **Sequencing Prerequisites (recommended first)**:
  - `P1-P4` registry closure wave complete or near-complete (to reduce path-churn during foundational pane changes)
- **Backlog Lane**: `Next`
- **Milestone**: `Wave B / Pane-view foundation`
- **Effort**: `M`
- **Goal**: Establish pane-hosted view payload model (Graph / Node Viewer / Tool) as the canonical workbench abstraction.
- **Scope**:
  - Define pane payload types and migration target shape.
  - Align render/compositor dispatch and persistence model with pane payloads.
  - Preserve `GraphViewState` as graph-pane-only payload state.
- **Out of Scope**:
  - Full graph multi-view UI
  - Wry backend implementation details
- **Dependencies**: None; architecture slice before graph multi-view implementation.
- **Subtasks (recommended split)**:
  - `P5.a` Type-model proposal in code/doc (PaneId + PaneViewState).
  - `P5.b` Dispatch path mapping (render/compositor).
  - `P5.c` Persistence migration target shape and transition note.
- **Definition of Ready**:
  - Current tile/pane/view model ownership is mapped.
  - Conflicting docs are identified (most already synced).
- **Acceptance**:
  - Pane payload model is represented in code-facing types or tracked implementation doc.
  - No conflicting graph-only pane assumptions in active strategy docs.
- **Definition of Done**:
  - Workbench layer abstraction can host graph, node-viewer, and tool panes conceptually and in implementation planning.
  - P6/P7/P8 can proceed without revisiting pane semantics.
- **Review Check (comprehension)**:
  - This task prevents graph-pane improvements from hardcoding assumptions that break viewer/tool panes later.
- **Status update (2026-02-25)**:
  - Closed via `mark-ik/graphshell#5`.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-22_multi_graph_pane_plan.md:34`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

### P6. Graph Multi-View Implementation (`GraphViewId`, Canonical/Divergent, Per-Pane Lens)

- **Type**: Feature / App state + UI
- **GitHub Issue Title**: `Graph panes: implement multi-view state (GraphViewId, per-pane lens, canonical/divergent)`
- **GitHub Labels**: `priority/top10`, `feature`, `graph-ui`, `workbench`, `lens`, `layout`
- **GitHub Milestone**: `Wave B / Graph multi-view`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - `P5` complete (pane-hosted view payload architecture foundation)
- **Sequencing Prerequisites (recommended first)**:
  - `P1-P4` registry closure wave complete (cleaner paths / lower churn)
- **Backlog Lane**: `Next`
- **Milestone**: `Wave B / Graph multi-view`
- **Effort**: `L`
- **Goal**: Implement multiple graph panes with independent cameras and lens/layout state.
- **Scope**:
  - Add `GraphViewId` / `GraphViewState`.
  - Replace singular graph camera/egui state with per-view storage.
  - Graph pane split + lens selector UI.
- **Out of Scope**:
  - 2D/3D hotswitch
  - Full advanced projection suite (timeline/kanban/map)
- **Dependencies**: P5 (pane payload model), registry Phase 6 preferred for cleaner paths.
- **Subtasks (recommended split)**:
  - `P6.a` State model + focused view wiring.
  - `P6.b` `TileKind`/pane payload integration for graph panes.
  - `P6.c` Render path accepts `GraphViewId`.
  - `P6.d` Split graph view + per-pane lens selector UI.
  - `P6.e` Canonical/Divergent mode controls + commit action stub.
- **Definition of Ready**:
  - Singular graph camera/state callsites are identified.
  - Lens resolution path is stable enough to call from per-view render path.
- **Acceptance**:
  - Two graph panes render same graph with independent cameras.
  - Per-pane lens switching works.
  - Canonical/Divergent toggle path exists (initial UI can be minimal).
- **Definition of Done**:
  - Multi-graph-pane experience is usable without global camera conflicts.
  - Core graph-pane interactions target focused/hovered pane correctly.
- **Review Check (comprehension)**:
  - This is the first user-visible payoff of the pane-hosted architecture, but only for graph pane payloads.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-22_multi_graph_pane_plan.md:120`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

### P7. Universal Content Model Foundation (Steps 1-3)

- **Type**: Feature / Data model / Viewer registry
- **GitHub Issue Title**: `Universal content foundation: node MIME/address hints + viewer selection + plaintext viewer`
- **GitHub Labels**: `priority/top10`, `feature`, `viewer`, `data-model`, `registry`, `persistence`
- **GitHub Milestone**: `Wave B / Viewer foundation`
- **GitHub Issue Status**: `Queued`
- **Blocking Prerequisites**:
  - `None (hard blockers in docs)`
- **Sequencing Prerequisites (recommended first)**:
  - `P5` pane/view foundation aligned (avoids rework in viewer-pane assumptions)
  - `P1-P4` registry closure wave preferred (reduces migration churn)
- **Backlog Lane**: `Next`
- **Milestone**: `Wave B / Viewer foundation`
- **Effort**: `L`
- **Goal**: Land `mime_hint` + `address_kind`, viewer selection policy, and baseline plaintext viewer.
- **Scope**:
  - Node data model + WAL logging.
  - `ViewerRegistry::select_for(...)`.
  - `viewer:plaintext` embedded renderer baseline.
- **Out of Scope**:
  - `viewer:image`, `viewer:pdf`, `viewer:audio`, `viewer:directory` implementations beyond baseline planning
  - Typed `Address` enum schema migration
- **Dependencies**: Viewer trait contract in current registry layer; pane/viewer model alignment recommended.
- **Subtasks (recommended split)**:
  - `P7.a` Node fields + WAL entries + intents.
  - `P7.b` MIME detection pipeline (extension + content-byte pass).
  - `P7.c` `ViewerRegistry::select_for(...)` + lifecycle integration.
  - `P7.d` Plaintext viewer baseline renderer + tests.
- **Definition of Ready**:
  - `Node` struct and WAL schema migration touchpoints are identified.
  - Viewer selection callsite in lifecycle reconcile is confirmed.
- **Acceptance**:
  - Data-model tests pass.
  - PDF/text file node routes to expected viewer by policy.
  - No regression to Servo default web path.
- **Definition of Done**:
  - Node content metadata is durable and viewer routing is policy-driven.
  - Plaintext content path proves non-web embedded viewer contract in production code.
- **Review Check (comprehension)**:
  - This is the prerequisite for heterogeneous node viewers, not just a viewer feature add.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_universal_content_model_plan.md:145`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

### P8. Wry Backend Foundation (Steps 1-5)

- **Type**: Feature / Platform compatibility / Viewer backend
- **GitHub Issue Title**: `Verso/Wry: add feature-gated overlay backend through lifecycle integration`
- **GitHub Labels**: `priority/top10`, `feature`, `viewer`, `wry`, `windows`, `verso`, `platform`
- **GitHub Milestone**: `Wave B / Viewer foundation`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - `P7` complete (universal content foundation / viewer selection baseline)
- **Sequencing Prerequisites (recommended first)**:
  - `P5` pane/view foundation aligned (node viewer pane semantics explicit)
- **Backlog Lane**: `Next`
- **Milestone**: `Wave B / Viewer foundation`
- **Effort**: `L`
- **Goal**: Add feature-gated Wry backend through lifecycle integration.
- **Scope**:
  - Feature gate + build paths.
  - `WryManager`, `WryViewer`.
  - Tile/pane overlay tracking.
  - Lifecycle reconcile integration.
- **Out of Scope**:
  - Full settings/per-workspace UI (Step 7 can follow)
  - Linux/macOS platform parity hardening beyond initial compile/runtime path
- **Dependencies**: P7 (viewer contract/selection foundation) strongly preferred.
- **Subtasks (recommended split)**:
  - `P8.a` Cargo feature gate + compile-path hygiene.
  - `P8.b` `WryManager` scaffold.
  - `P8.c` `WryViewer` registration and placeholder render path.
  - `P8.d` Overlay tracking in compositor for node viewer panes/tiles.
  - `P8.e` Lifecycle reconcile integration.
- **Definition of Ready**:
  - Wry feature dependencies/build instructions verified for target platform (Windows first).
  - Overlay sync callsite (`TileCompositor` or pane-compositor equivalent) identified.
- **Acceptance**:
  - `cargo build` with and without `--features wry`.
  - Wry-backed node viewer pane receives overlay sync and lifecycle transitions.
- **Definition of Done**:
  - Wry is selectable as a backend and behaves correctly as a pane-only overlay viewer path.
  - Graph canvas fallback behavior for Wry nodes remains intact.
- **Review Check (comprehension)**:
  - Wry is a backend for node viewer panes, not a replacement pane system.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-23_wry_integration_strategy.md:137`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

### P9. Control UI/UX Consolidation (ActionRegistry-Driven Command Surfaces)

- **Type**: UX / Refactor / Input
- **GitHub Issue Title**: `Control UI: unify command palette and radial menu via ActionRegistry`
- **GitHub Labels**: `priority/top10`, `ui`, `ux`, `input`, `action-registry`, `refactor`
- **GitHub Milestone**: `Wave C / UX consolidation`
- **GitHub Issue Status**: `Queued`
- **Blocking Prerequisites**:
  - `None`
- **Backlog Lane**: `Next`
- **Milestone**: `Wave C / UX consolidation`
- **Effort**: `M-L`
- **Goal**: Replace fragmented command surfaces with extracted, `ActionRegistry`-driven radial menu + command palette.
- **Scope**:
  - Extract modules from `render/mod.rs`.
  - Route content through `ActionRegistry::list_actions_for_context`.
  - Unify global/contextual command palette surface.
- **Out of Scope**:
  - Full gamepad navigation polish (can be follow-on if scope pressure)
  - All settings UI remapping controls
- **Dependencies**: None; can start with extraction and registry wiring.
- **Subtasks (recommended split)**:
  - `P9.a` Extract `radial_menu.rs`.
  - `P9.b` Extract `command_palette.rs`.
  - `P9.c` Replace hardcoded enums with `ActionRegistry` content.
  - `P9.d` Merge contextual/global palette surface behavior.
- **Definition of Ready**:
  - Existing radial/palette code paths and hardcoded command enums are identified.
  - `ActionRegistry::list_actions_for_context` returns enough metadata for UI rendering.
- **Acceptance**:
  - Radial and palette populate via `ActionRegistry`.
  - No hardcoded parallel command enums remain in these surfaces.
- **Definition of Done**:
  - Command surfaces are modular and backended by registry data.
  - `render/mod.rs` no longer owns large UI implementations for these controls.
- **Review Check (comprehension)**:
  - This is partly architecture hygiene and partly UX consistency; extraction and registry routing are the core deliverables.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_control_ui_ux_plan.md:226`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

### P10. Scale + Accessibility Baseline (Viewport Culling + WebView A11y Bridge)

- **Type**: Performance + Accessibility
- **GitHub Issue Title**: `Baseline quality: graph viewport culling + WebView accessibility bridge`
- **GitHub Labels**: `priority/top10`, `performance`, `a11y`, `graph-ui`, `webview`, `baseline`
- **GitHub Milestone**: `Wave C / Baselines`
- **GitHub Issue Status**: `Queued`
- **Blocking Prerequisites**:
  - `None`
- **Sequencing Prerequisites (recommended first)**:
  - `P5` if culling implementation touches pane/view dispatch heavily (otherwise optional)
- **Backlog Lane**: `Next`
- **Milestone**: `Wave C / Baselines`
- **Effort**: `L` (or split into two `M` tickets)
- **Goal**: Land Phase 1 slices from performance and accessibility plans before feature breadth increases.
- **Scope**:
  - Viewport culling policy + implementation (graph-pane scoped).
  - WebView accessibility bridge critical fix.
- **Out of Scope**:
  - Full graph linearization / sonification stack
  - Full LOD/occlusion/perf phase completion beyond culling baseline
- **Dependencies**: None; may split into two sub-tickets if staffing differs.
- **Subtasks (recommended split)**:
  - `P10.a` Viewport culling policy toggle(s) + graph render path integration.
  - `P10.b` Culling validation/benchmark instrumentation.
  - `P10.c` WebView bridge event forwarding and AccessKit tree graft path.
  - `P10.d` Accessibility validation harness/manual checks.
- **Definition of Ready**:
  - Graph render visible-set candidate path is identified.
  - Current webview accessibility callback flow is traced end-to-end.
- **Acceptance**:
  - Visible-set reduction + frame-time improvement observed.
  - Screen reader can enter/read embedded web content path.
- **Definition of Done**:
  - Both performance and a11y Phase 1 "critical" slices are landed or split into independently tracked child issues with clear handoff.
- **Review Check (comprehension)**:
  - This is intentionally a paired baseline ticket because both are implementation-ready and reduce risk before more surface-area work.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_performance_tuning_plan.md:24`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_spatial_accessibility_plan.md:32`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

---

## 2. Forgotten Concept Adoption Ticket Stubs (Top 10)

**GitHub issue pattern for concept-adoption tickets**:
- These are mostly roadmap/design issues (not implementation tickets yet).
- Mark as `Deferred (blocked)` when a concept depends on prerequisite feature/platform work not yet landed.
- Deliverable is usually a new or updated implementation strategy section/doc with acceptance criteria.

### F1. Adopt Visual Tombstones into Active UX Roadmap

- **Type**: Concept adoption / UX roadmap
- **GitHub Issue Title**: `Roadmap: adopt visual tombstones (ghost nodes/edges) into active UX planning`
- **GitHub Labels**: `concept/adoption`, `ux`, `graph-ui`, `research-followup`, `future-roadmap`
- **GitHub Milestone**: `Concept Adoption / UX`
- **GitHub Issue Status**: `Done`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Promote ghost-node/ghost-edge deletion memory concept into planned UX work (not just research).
- **Scope**:
  - Add a concrete implementation strategy stub or phase entry.
  - Define toggle + retention semantics.
- **Dependencies**: None.
- **Acceptance**:
  - Concept appears in an implementation strategy with initial scope and constraints.
- **Definition of Done**:
  - A current (non-research-only) strategy doc contains a phased tombstone plan with toggles/retention notes.
- **Review Check (comprehension)**:
  - This is an adoption/planning task, not implementation of ghost rendering.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-24_visual_tombstones_research.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-25_visual_tombstones_plan.md` ✅ phased plan with toggle + retention semantics

### F2. Adopt Temporal Navigation / Time-Travel Preview

- **Type**: Concept adoption / Traversal UX
- **GitHub Issue Title**: `Traversal roadmap: adopt temporal navigation / time-travel preview as staged deliverable`
- **GitHub Labels**: `concept/adoption`, `traversal`, `history`, `timeline`, `research-followup`
- **GitHub Milestone**: `Concept Adoption / Traversal`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - Edge traversal `Stage E` maturity (history manager + archive behaviors stable enough to plan against)
- **Goal**: Track temporal navigation as a planned Stage F deliverable with explicit preview-mode constraints.
- **Scope**:
  - Promote from “future concept” into staged backlog under traversal/history work.
  - Link to preview-mode effect isolation contract.
- **Dependencies**: Edge traversal Stage E maturity.
- **Acceptance**:
  - Temporal navigation appears in active roadmap/strategy with prerequisites and non-goals.
- **Definition of Done**:
  - Stage F is promoted from distant concept to a tracked backlog/milestone item with preview isolation constraints.
- **Review Check (comprehension)**:
  - Temporal navigation depends on traversal truth and preview safety, not just UI timeline controls.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md:526`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

### F3. Adopt Collaborative Presence (Ghost Cursors / Remote Selection / Follow Mode)

- **Type**: Concept adoption / Verse UX
- **GitHub Issue Title**: `Verse roadmap: adopt collaborative presence UX (ghost cursors, remote selection, follow mode)`
- **GitHub Labels**: `concept/adoption`, `verse`, `collaboration`, `ux`, `future-roadmap`
- **GitHub Milestone**: `Concept Adoption / Verse UX`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - `P1` and `P2` complete (Verse Phase 5.4/5.5 done-gate closure)
- **Goal**: Add presence UX to Verse roadmap as post-Phase-5 work, not just vision prose.
- **Scope**:
  - Define minimum presence events and rendering cues.
  - Identify diagnostics and privacy constraints.
- **Dependencies**: Verse Phase 5 done-gate closure.
- **Acceptance**:
  - Presence feature tracked in implementation strategy with phased scope.
- **Definition of Done**:
  - Presence is explicitly sequenced after Verse correctness/done-gate closure.
- **Review Check (comprehension)**:
  - Presence is a sync UX layer built on top of stable Verse semantics, not a substitute for them.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-18_graph_ux_research_report.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

### F4. Adopt DOI + Semantic Fisheye Focus/Context Pipeline

- **Type**: Concept adoption / Graph readability
- **GitHub Issue Title**: `Graph UX roadmap: adopt DOI + semantic fisheye focus/context pipeline`
- **GitHub Labels**: `concept/adoption`, `graph-ui`, `ux`, `performance`, `research-followup`
- **GitHub Milestone**: `Concept Adoption / Graph UX`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - Basic graph LOD/culling baseline landed (at least `P10` performance slice for culling/LOD direction)
- **Goal**: Introduce DOI-driven rendering and semantic fisheye as a concrete post-LOD roadmap item.
- **Scope**:
  - Define DOI data contract and update cadence.
  - Split rendering concerns (size/opacity/LOD) from filtering behavior.
- **Dependencies**: Basic culling/LOD in place.
- **Acceptance**:
  - Dedicated strategy entry exists with metrics/perf guardrails.
- **Definition of Done**:
  - DOI + fisheye planning includes update cadence, cost guardrails, and rendering behavior separation.
- **Review Check (comprehension)**:
  - DOI/fisheye should build on stable LOD/culling primitives, not precede them.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-18_graph_ux_research_report.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-25_doi_fisheye_plan.md`

### F5. Adopt Magnetic Zones / Group-in-a-Box Workflow

- **Type**: Concept adoption / Spatial organization
- **GitHub Issue Title**: `Layout roadmap: adopt magnetic zones / group-in-a-box workflow as tracked feature`
- **GitHub Labels**: `concept/adoption`, `layout`, `graph-ui`, `spatial-organization`, `research-followup`
- **GitHub Milestone**: `Concept Adoption / Layout`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - Layout injection hook and zone persistence semantics defined (currently design gaps)
  - Multi-view Canonical/Divergent semantics settled enough to define zone scope
- **Goal**: Promote zones from research/layout notes into tracked implementation sequence.
- **Scope**:
  - Define zone persistence scope (view/workspace/lens).
  - Clarify interaction model and overlap rules.
- **Dependencies**: Layout injection hook and multi-view semantics.
- **Acceptance**:
  - Zones have a tracked implementation plan or extended section in layout behaviors plan.
- **Definition of Done**:
  - Zone scope (`view/workspace/lens`) and interaction model are documented as prerequisites for implementation.
- **Review Check (comprehension)**:
  - Zoning is both a layout-force feature and a persistence/scope design problem.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_layout_behaviors_plan.md:57`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

### F6. Adopt Graph Reader / List-View Accessibility Fallback

- **Type**: Concept adoption / Accessibility
- **GitHub Issue Title**: `Accessibility roadmap: adopt graph reader / list-view fallback (Room + Map linearization)`
- **GitHub Labels**: `concept/adoption`, `a11y`, `graph-ui`, `accesskit`, `research-followup`
- **GitHub Milestone**: `Concept Adoption / Accessibility`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - WebView accessibility bridge Phase 1 (`P10` a11y slice or equivalent) landed
- **Goal**: Elevate graph linearization “Room/Map” concepts into active accessibility implementation planning.
- **Scope**:
  - Define initial linearization mode and user entry points.
  - Connect to AccessKit virtual tree / navigation model.
- **Dependencies**: WebView bridge Phase 1 recommended first.
- **Acceptance**:
  - Accessibility plan Phase 2 references concrete linearization mode choices.
- **Definition of Done**:
  - Accessibility plan contains explicit Graph Reader mode(s), navigation entrypoints, and output shape.
- **Review Check (comprehension)**:
  - The graph reader is the graph accessibility core, but WebView bridge is still the first critical fix.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-24_spatial_accessibility_research.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

### F7. Adopt Unified Omnibar (URL + Graph Search + Web Search)

- **Type**: Concept adoption / Browser UX
- **GitHub Issue Title**: `Browser UX roadmap: adopt unified omnibar (URL + graph search + web search)`
- **GitHub Labels**: `concept/adoption`, `browser-ux`, `input`, `search`, `action-registry`
- **GitHub Milestone**: `Concept Adoption / Browser UX`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - Control UI/input routing stabilization (at least `P9` command-surface consolidation direction)
- **Goal**: Promote unified omnibar heuristics into a trackable implementation item.
- **Scope**:
  - Define heuristics and conflict resolution order.
  - Identify `ActionRegistry` / `InputRegistry` integration points.
- **Dependencies**: Control UI/input routing stabilization.
- **Acceptance**:
  - Omnibar implementation plan stub exists with heuristics + validation cases.
- **Definition of Done**:
  - Heuristic order and ambiguity resolution are documented, with planned integration points.
- **Review Check (comprehension)**:
  - Omnibar adoption is mostly an interaction-model and routing decision, not just a text field UI.
- **Source refs**:
  - `design_docs/graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md:297`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

### F8. Adopt Progressive Lenses + Lens/Physics Binding Policy

- **Type**: Concept adoption / Lens UX
- **GitHub Issue Title**: `Lens UX roadmap: adopt progressive lenses and lens/physics binding policy`
- **GitHub Labels**: `concept/adoption`, `lens`, `physics`, `ux`, `research-followup`
- **GitHub Milestone**: `Concept Adoption / Lens UX`
- **GitHub Issue Status**: `Deferred (blocked)` — policy doc written; blocked on runtime prerequisites
- **Blocking Prerequisites**:
  - Active Lens resolution path stable in runtime
  - Distinct physics preset behaviors established
  - ~~Lens/physics binding preference semantics resolved~~ ✓ resolved in `2026-02-25_progressive_lens_and_physics_binding_plan.md`
- **Goal**: Move progressive lens behavior from research notes into planned design/implementation.
- **Scope**:
  - Define trigger semantics (`Always/Ask/Never`, thresholds/interpolation).
  - Specify lens-to-physics binding contract.
- **Dependencies**: Active Lens resolution path + distinct physics presets.
- **Acceptance**:
  - Lens/physics binding is represented in a strategy doc with explicit open questions resolved. ✓ **Done** — see `2026-02-25_progressive_lens_and_physics_binding_plan.md`
- **Definition of Done**:
  - Progressive lens switching trigger semantics (`Always/Ask/Never`, thresholds/interpolation) are specified. ✓ **Done** — §2 of strategy doc.
- **Review Check (comprehension)**:
  - This feature is a policy/interaction problem first; implementing it too early causes surprising behavior.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-24_interaction_and_semantic_design_schemes.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_physics_engine_extensibility_plan.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-25_progressive_lens_and_physics_binding_plan.md` *(strategy doc)*

### F9. Adopt 2D↔3D Hotswitch as a Tracked Multi-View Capability

- **Type**: Concept adoption / Future architecture
- **GitHub Issue Title**: `Future architecture: adopt 2D↔3D hotswitch as tracked graph-view capability`
- **GitHub Labels**: `concept/adoption`, `3d`, `graph-ui`, `architecture`, `future-roadmap`
- **GitHub Milestone**: `Concept Adoption / 3D`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - `P5` pane-hosted view foundation complete
  - `P6` graph multi-view state (`GraphViewState`) stabilized
- **Goal**: Add `ViewDimension` / position-parity hotswitch to medium-term roadmap tied to graph-pane architecture.
- **Scope**:
  - Define where `ViewDimension` lives and how snapshots degrade gracefully.
  - Link to graph-pane payload model.
- **Dependencies**: Pane-hosted multi-view + `GraphViewState` stabilization.
- **Acceptance**:
  - 3D hotswitch appears as a roadmap milestone or dedicated implementation strategy.
- **Definition of Done**:
  - `ViewDimension` ownership and snapshot fallback/degradation rules are tracked in a current plan.
- **Review Check (comprehension)**:
  - 3D hotswitch belongs to graph-view state, so it depends on graph multi-view architecture maturity.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_physics_engine_extensibility_plan.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

### F10. Adopt Interactive HTML Export (Self-Contained Graph Artifact)

- **Type**: Concept adoption / Sharing/export
- **GitHub Issue Title**: `Export roadmap: adopt interactive HTML export (self-contained graph artifact)`
- **GitHub Labels**: `concept/adoption`, `export`, `sharing`, `browser-ux`, `future-roadmap`
- **GitHub Milestone**: `Concept Adoption / Export`
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - Current export/snapshot artifact strategy clarified for viewer/content metadata
  - Privacy/redaction behavior defined for export path
- **Goal**: Bring interactive export back into active planning with a modern scope statement.
- **Scope**:
  - Define export artifact type and offline capabilities.
  - Identify privacy/redaction and viewer fallback constraints.
- **Dependencies**: Snapshot/export format clarity; content/viewer metadata maturity.
- **Acceptance**:
  - Export concept is captured in a current (non-archived) strategy doc.
- **Definition of Done**:
  - Export concept is restated in a current plan with artifact scope, privacy constraints, and fallback rules.
- **Review Check (comprehension)**:
  - This is a roadmap resurrection task from archived philosophy, not immediate implementation.
- **Source refs**:
  - `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

---

## 3. Quick Win Ticket Stubs (Top 10)

**GitHub issue pattern for quick wins**:
- Default to `Ready` unless a hard prerequisite is actually missing.
- Prefer narrow, mergeable slices and explicit "no behavior change" statements for refactors.

### Q1. Extract `desktop/radial_menu.rs` from `render/mod.rs`

- **Type**: Refactor
- **GitHub Issue Title**: `Refactor: extract radial menu module from render/mod.rs`
- **GitHub Labels**: `quick-win`, `refactor`, `ui`, `rendering`, `low-risk`
- **GitHub Milestone**: `Quick Wins / UI cleanup`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Move radial menu implementation into its own module with no behavior change.
- **Scope**: Mechanical extraction + callsite wiring.
- **Dependencies**: None.
- **Acceptance**: Build/tests unchanged; `render/mod.rs` reduced.
- **Definition of Done**:
  - Radial menu code lives in dedicated module and callsite remains behaviorally equivalent.
- **Review Check (comprehension)**:
  - This is a mechanical extraction intended to reduce future merge conflict risk in `render/mod.rs`.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_control_ui_ux_plan.md:226`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q2. Extract `desktop/command_palette.rs` from `render/mod.rs`

- **Type**: Refactor
- **GitHub Issue Title**: `Refactor: extract command palette module from render/mod.rs`
- **GitHub Labels**: `quick-win`, `refactor`, `ui`, `rendering`, `low-risk`
- **GitHub Milestone**: `Quick Wins / UI cleanup`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Move command palette implementation into its own module with no behavior change.
- **Scope**: Mechanical extraction + callsite wiring.
- **Dependencies**: None (pair with Q1).
- **Acceptance**: Build/tests unchanged; `render/mod.rs` reduced.
- **Definition of Done**:
  - Command palette code is isolated in a dedicated module and behavior remains unchanged.
- **Review Check (comprehension)**:
  - Pairing with `Q1` is recommended to make `P9` cheaper later.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_control_ui_ux_plan.md:226`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q3. Reheat Physics on `AddNode` / `AddEdge`

- **Type**: Behavior fix
- **GitHub Issue Title**: `Graph UX: reheat physics automatically on AddNode/AddEdge`
- **GitHub Labels**: `quick-win`, `graph-ui`, `physics`, `ux-polish`, `behavior-fix`
- **GitHub Milestone**: `Quick Wins / Graph feel`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Resume physics automatically on structural graph changes.
- **Scope**: Set `physics.is_running = true` in reducer path for structural intents (excluding replay/load paths).
- **Dependencies**: None.
- **Acceptance**: Adding node/edge while paused visibly resumes simulation.
- **Definition of Done**:
  - Structural adds reliably resume simulation without resetting positions/velocities unnecessarily.
- **Review Check (comprehension)**:
  - This addresses a perceived "dead graph" bug/feel issue, not physics algorithm quality.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_layout_behaviors_plan.md:18`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q4. Spawn New Nodes Near Semantic Parent (Parent + Jitter)

- **Type**: Behavior fix / UX quality
- **GitHub Issue Title**: `Graph layout UX: place new linked nodes near semantic parent (plus jitter)`
- **GitHub Labels**: `quick-win`, `graph-ui`, `layout`, `ux-polish`, `behavior-fix`
- **GitHub Milestone**: `Quick Wins / Graph feel`
- **GitHub Issue Status**: `Queued`
- **Blocking Prerequisites**:
  - `None (hard blockers)`
- **Sequencing Prerequisites (recommended first)**:
  - Verify parent/source identity is available on the target creation path; if not, split a tiny plumbing subtask first
- **Goal**: Improve mental-map preservation by avoiding center-spawn default for linked nodes.
- **Scope**: Carry parent/source identity through creation path and initialize position near parent.
- **Dependencies**: Parent identity availability in relevant event path.
- **Acceptance**: Link/open-new-context nodes appear near source node.
- **Definition of Done**:
  - Source-linked node creation uses parent-relative placement where source is known; fallback behavior remains safe when unknown.
- **Review Check (comprehension)**:
  - The main risk is event-path identity plumbing, not placement math.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_layout_behaviors_plan.md:37`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q5. Fix `WebViewUrlChanged` Prior-URL Ordering in Traversal Append Path

- **Type**: Correctness bugfix
- **GitHub Issue Title**: `Traversal correctness: fix WebViewUrlChanged prior-URL capture ordering`
- **GitHub Labels**: `quick-win`, `bug`, `traversal`, `history`, `correctness`
- **GitHub Milestone**: `Quick Wins / Correctness`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Ensure traversal records capture prior URL before mutation.
- **Scope**: Audit `WebViewUrlChanged` ordering and `push_traversal` callsite sequencing.
- **Dependencies**: None.
- **Acceptance**: Ordering tests/targeted repro confirm correct `from_url`/`to_url`.
- **Definition of Done**:
  - Traversal append ordering is deterministic and regression-covered.
- **Review Check (comprehension)**:
  - This is a data-integrity fix that protects future history/timeline features.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-20_edge_traversal_impl_plan.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q6. Wire `Ctrl+Click` Multi-Select in Graph Pane

- **Type**: UX improvement
- **GitHub Issue Title**: `Graph selection UX: wire Ctrl+Click multi-select toggle`
- **GitHub Labels**: `quick-win`, `graph-ui`, `selection`, `ux-polish`, `input`
- **GitHub Milestone**: `Quick Wins / Graph interactions`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Enable multi-select toggle behavior already supported by `SelectionState`.
- **Scope**: Pass modifier state from graph render input to selection handling.
- **Dependencies**: None.
- **Acceptance**: `Ctrl+Click` toggles node membership in selection set.
- **Definition of Done**:
  - Multi-select toggle works consistently and does not regress single-select behavior.
- **Review Check (comprehension)**:
  - This is mostly input wiring because the state model already supports it.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-18_graph_ux_research_report.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q7. Add Semantic Container Tab Titles (`Split ↔`, `Split ↕`, `Tab Group`, `Grid`)

- **Type**: UX clarity improvement
- **GitHub Issue Title**: `Workbench UX: replace container debug titles with semantic tab labels`
- **GitHub Labels**: `quick-win`, `workbench`, `ux-polish`, `tile-tree`, `labels`
- **GitHub Milestone**: `Quick Wins / Workbench clarity`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Replace raw container debug labels with semantic user-facing titles.
- **Scope**: Update tile/tab title rendering for container nodes.
- **Dependencies**: None.
- **Acceptance**: Container tabs no longer display ambiguous `Horizontal`/`Vertical` labels.
- **Definition of Done**:
  - User-facing labels are semantic and consistent with interaction-consistency terminology.
- **Review Check (comprehension)**:
  - This is a discoverability fix that preserves the real tile-tree architecture.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-23_graph_interaction_consistency_plan.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q8. Add Zoom-Adaptive Label LOD Thresholds (Hide / Domain / Full)

- **Type**: UX + Performance improvement
- **GitHub Issue Title**: `Graph rendering: add zoom-adaptive label LOD thresholds (hide/domain/full)`
- **GitHub Labels**: `quick-win`, `graph-ui`, `performance`, `lod`, `ux-polish`
- **GitHub Milestone**: `Quick Wins / Rendering clarity`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Reduce clutter and rendering cost at low zoom with simple label LOD.
- **Scope**: Threshold-based label display logic keyed to graph-pane zoom.
- **Dependencies**: None.
- **Acceptance**: Labels change modes at configured thresholds without panics/regressions.
- **Definition of Done**:
  - Thresholds are implemented, tested/manual-validated, and scoped per graph-pane zoom.
- **Review Check (comprehension)**:
  - This is the cheapest practical step toward fuller LOD/culling work.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_performance_tuning_plan.md:40`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q9. Add `ChannelSeverity` to Diagnostics Channel Descriptors

- **Type**: Diagnostics schema improvement
- **GitHub Issue Title**: `Diagnostics schema: add ChannelSeverity to channel descriptors`
- **GitHub Labels**: `quick-win`, `diagnostics`, `schema`, `observability`, `low-risk`
- **GitHub Milestone**: `Quick Wins / Diagnostics`
- **GitHub Issue Status**: `Ready`
- **Blocking Prerequisites**:
  - `None`
- **Goal**: Add severity tier (`Info/Warn/Error`) for better diagnostics pane prioritization.
- **Scope**: Extend descriptor schema + defaults + any impacted tests.
- **Dependencies**: None.
- **Acceptance**: Channel descriptors include severity with sane defaults; existing tests updated.
- **Definition of Done**:
  - Severity is present in descriptors/defaults and does not break existing diagnostics flows.
- **Review Check (comprehension)**:
  - This is a schema-enrichment slice that unlocks better diagnostics UI later.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-24_diagnostics_research.md:126`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

### Q10. Add/Confirm `CanvasRegistry` Culling + LOD Policy Toggles

- **Type**: Registry policy wiring
- **GitHub Issue Title**: `CanvasRegistry: add/confirm viewport culling and label/edge LOD policy toggles`
- **GitHub Labels**: `quick-win`, `registry`, `performance`, `canvas`, `policy`
- **GitHub Milestone**: `Quick Wins / Performance plumbing`
- **GitHub Issue Status**: `Queued`
- **Blocking Prerequisites**:
  - `None (hard blockers)`
- **Sequencing Prerequisites (recommended first)**:
  - Registry path churn in Wave A sufficiently stable to avoid merge-conflict-heavy schema edits
- **Goal**: Ensure performance behaviors are controlled by policy (`CanvasRegistry`) rather than hardcoded toggles.
- **Scope**:
  - `viewport_culling_enabled`
  - `label_culling_enabled`
  - edge LOD policy setting
- **Dependencies**: Registry layout surface schema ownership.
- **Acceptance**: Performance plan Phase 1/2 toggles resolve from `CanvasRegistry`.
- **Definition of Done**:
  - Culling/LOD toggles exist (or are confirmed existing) in `CanvasRegistry` and are the canonical policy source for performance slices.
- **Review Check (comprehension)**:
  - This is a small registry-surface alignment task that de-risks later performance implementation.
- **Source refs**:
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_performance_tuning_plan.md:15`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:89`

---

## 4. Suggested Tracker Labels (Optional)

- **Priority tasks**: `priority/top10`, `architecture`, `registry`, `viewer`, `ui`, `performance`, `a11y`
- **Forgotten concepts**: `concept/adoption`, `research-followup`, `future-roadmap`
- **Quick wins**: `quick-win`, `low-risk`, `refactor`, `ux-polish`, `diag`

---

## 5. Import Notes

- These are **stubs**, not final implementation specs.
- If importing into an issue tracker, keep the `P#`, `F#`, `Q#` prefixes so the register and tracker stay aligned.
- Large tickets (`P5`-`P10`) should usually be split into implementation sub-issues after assignment.

---

## 3. Copilot Implementation Guides

_Source file before consolidation: `2026-02-25_copilot_implementation_guides.md`_


Generated 2026-02-25 during PR review. These guides cover the open Copilot PRs
that had empty "Initial plan" commits. Apply to the corresponding `copilot/`
branches.

---

## Already Implemented (apply these to copilot branches)

### #47 wire-ctrl-click-multi-select (`copilot/wire-ctrl-click-multi-select`)

**File:** `render/mod.rs:125`

The infrastructure is already in place — `multi_select_modifier` flows through
`collect_graph_actions` → `GraphAction::SelectNode { multi_select }` →
`GraphIntent::SelectNode { multi_select }` → `SelectionState::select()`.

**One-line fix:**
```rust
// Before (line 125):
let ctrl_pressed = ui.input(|i| i.modifiers.ctrl);
// After:
let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
```

Extend ctrl detection to include `i.modifiers.command` so macOS Cmd+Click
also triggers multi-select. No other changes needed.

---

### #49 replace-debug-titles-with-semantic-labels (`copilot/replace-debug-titles-with-semantic-labels`)

**File:** `shell/desktop/workbench/tile_render_pass.rs:84-93`

Replace the `tile_hierarchy_lines` format strings in the diagnostics display:

```rust
// Tabs container:
format!("Tab Group ({} tabs)", tabs.children.len())

// Linear container:
use egui_tiles::LinearDir;
let dir_label = match linear.dir {
    LinearDir::Horizontal => "Split ↔",
    LinearDir::Vertical => "Split ↕",
};
format!("{} ({} panes)", dir_label, linear.children.len())

// Generic container:
format!("Panel Group ({:?})", other.kind())
```

See `shell/desktop/workbench/tile_behavior.rs:409` for existing `LinearDir` usage.

---

### #48 add-channel-severity-to-descriptors (`copilot/add-channel-severity-to-descriptors`)

**File:** `registries/atomic/diagnostics.rs`

**Step 1** — Add enum before `DiagnosticChannelDescriptor`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ChannelSeverity {
    #[default]
    Info,
    Warn,
    Error,
}
```

**Step 2** — Add field to both descriptor structs:
```rust
pub(crate) struct DiagnosticChannelDescriptor {
    pub(crate) channel_id: &'static str,
    pub(crate) schema_version: u16,
    pub(crate) severity: ChannelSeverity,  // ADD
}

pub(crate) struct RuntimeChannelDescriptor {
    // ... existing fields ...
    pub(crate) severity: ChannelSeverity,  // ADD
}
```

**Step 3** — Propagate in `RuntimeChannelDescriptor::from_contract`:
```rust
severity: descriptor.severity,
```

**Step 4** — Update all `DiagnosticChannelDescriptor { ... }` struct literals to
include `severity:`. Use these defaults:
- `*_FAILED`, `*_DENIED`, `*_REJECTED`, `*_UNAVAILABLE` → `ChannelSeverity::Error`
- `*_FALLBACK_USED`, `*_MISSING`, `*_CONFLICT`, `*_TIMEOUT`, `*_LIMIT` → `ChannelSeverity::Warn`
- Everything else → `ChannelSeverity::Info`

**Step 5** — Add `severity: ChannelSeverity::Info` to all `RuntimeChannelDescriptor { ... }`
literals (mod/verse registrations default to Info; callers can set severity via API).

**Step 6** — Add convenience constructors on `RuntimeChannelDescriptor` for runtime registrations:
- `RuntimeChannelDescriptor::info(...)`
- `RuntimeChannelDescriptor::warn(...)`
- `RuntimeChannelDescriptor::error(...)`

Prefer these helpers (backed by a shared `RuntimeChannelDescriptor::new(...)`) over manual
struct literals when registering dynamic channels. This reduces field-order/brace mistakes and
makes severity intent explicit at call sites.

---

## Concrete Features (require implementation)

### #50 add-zoom-adaptive-label-lod (`copilot/add-zoom-adaptive-label-lod`)

**Current state:** `model/graph/egui_adapter.rs:414-446` already implements
zoom-adaptive labels with thresholds at `0.6` (no labels) and `1.5`
(domain vs full labels).

**What's needed:** Make the thresholds configurable via `CanvasStylePolicy`
so they can be tuned per canvas profile.

**Files:**
- `registries/domain/layout/canvas.rs:31-33` — extend `CanvasStylePolicy`:
  ```rust
  pub(crate) struct CanvasStylePolicy {
      pub(crate) labels_always: bool,
      pub(crate) label_lod_enabled: bool,    // ADD: enable zoom-adaptive LOD
      pub(crate) label_hide_below: f32,       // ADD: default 0.6
      pub(crate) label_domain_below: f32,     // ADD: default 1.5
  }
  ```
- `model/graph/egui_adapter.rs:418-446` — pass thresholds into
  `label_text_for_zoom_value()` instead of hardcoding `0.6` / `1.5`.
- `render/mod.rs:106-112` — thread canvas profile policy into the adapter
  when building egui graph state.

---

### #51 add-viewport-culling-policy (`copilot/add-viewport-culling-policy`)

**Files:**
- `registries/domain/layout/canvas.rs` — add culling toggle to `CanvasTopologyPolicy`:
  ```rust
  pub(crate) struct CanvasTopologyPolicy {
      pub(crate) viewport_culling_enabled: bool,  // ADD: default true
      pub(crate) label_lod_policy_enabled: bool,  // ADD
  }
  ```
- `render/spatial_index.rs:58` — `nodes_in_canvas_rect()` is already implemented.
  Wire it in `render/mod.rs` where graph nodes are rendered: skip nodes outside
  the viewport rect when `viewport_culling_enabled`.
- `registries/domain/layout/canvas.rs:65-100` — update `resolve()` to include
  the new fields in `CanvasSurfaceResolution`.

---

### #52 add-node-mime-address-hints (`copilot/add-node-mime-address-hints`)

**Context:**
- `registries/atomic/viewer.rs` — `ViewerRegistry` with MIME/extension → viewer
  mappings. `select_for_uri()` already uses MIME hint.
- `shell/desktop/workbench/pane_model.rs:113` — `NodePaneState` has
  `viewer_id_override: Option<ViewerId>`.

**What's needed:**
1. Add `mime_hint: Option<String>` to `graph::Node` (in `graph/mod.rs`).
2. When a node is navigated/loaded, detect MIME from content-type response and
   store it in `node.mime_hint`.
3. In `pane_model.rs`, when opening a node viewer, call
   `viewer_registry.select_for_uri(node.url, node.mime_hint.as_deref())` to
   pick the appropriate viewer.
4. Add `viewer:plaintext` descriptor to `ViewerRegistry::core_seed()` for
   `text/plain` MIME type (already has `mime_hint` in `ViewerDescriptor`).

---

### #53 implement-multi-view-state (`copilot/implement-multi-view-state`)

**Context:** Infrastructure already exists!
- `app.rs:79-91` — `GraphViewId` is defined.
- `app.rs:217-228` — `GraphViewState { id, name, camera, lens, layout_mode, ... }`.
- `app.rs:1183,1186` — `workspace.views: HashMap<GraphViewId, GraphViewState>`,
  `workspace.focused_view: Option<GraphViewId>`.
- `pane_model.rs:90-104` — `GraphPaneRef` links a pane to a `GraphViewId`.
- `pane_model.rs:74-88` — `ViewLayoutMode { Canonical, Divergent }`.

**What's needed:** Wire the `GraphPaneRef.view_id` through the render pipeline so
each graph pane reads its own `GraphViewState` (camera, lens) rather than the
shared workspace state.

1. In `tile_render_pass.rs`, when rendering a graph tile, read the tile's
   `GraphPaneRef.view_id` and look up `app.workspace.views[view_id]` for
   camera/lens state.
2. Add `GraphIntent::CreateGraphView { name }` and
   `GraphIntent::SwitchPaneToView { pane_id, view_id }` intents.
3. Handle `SetViewLens` (already exists in `GraphIntent`) to update per-view lens.

---

### #39 improve-graph-viewport-culling (`copilot/improve-graph-viewport-culling`)

**Context:** `render/spatial_index.rs` already implements R*-tree spatial
indexing with `nodes_in_canvas_rect()`.

**What's needed:**
1. In `render/mod.rs`, after building `egui_state`, apply viewport culling:
   use `app.workspace.spatial_index` (if it exists) or build one from node
   positions to get only visible node keys.
2. Pass only visible nodes to the egui_graphs renderer.
3. Add WebView accessibility bridge: when the graph has a selected/focused node,
   emit an accessibility event with the node's URL and title via AccessKit or
   a platform accessibility API.

---

## Roadmap / Planning Issues (design docs only)

These issues track future architectural directions. The "Initial plan" commit is
the appropriate deliverable. For each, add a design doc stub that captures the
acceptance criteria as a tracked milestone:

### #41 adopt-unified-omnibar
- Unified omnibar: URL bar + graph search + web search in one input.
- Add planning stub to `design_docs/.../implementation_strategy/`.

### #53-61 (roadmap adopt-* issues)
- Each needs a design doc confirming the concept is tracked as a roadmap item.
- See existing `2026-02-25_interactive_html_export_plan.md` for format reference.

---

## PRs Blocked on Conflicts

### #42 refactor-radial-menu-module / #45 refactor-extract-command-palette-module

These PRs conflict with the merged #38 (`unify-command-palette-radial-menu`).
#38 already extracts both modules (`render/radial_menu.rs` and
`render/command_palette.rs`) via `ActionRegistry`. PRs #42 and #45 should be
**closed** as superseded, or rebased onto main if additional changes are needed
beyond what #38 already delivers.
