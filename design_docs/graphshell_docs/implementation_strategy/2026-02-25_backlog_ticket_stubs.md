# Backlog Ticket Stubs: Priorities, Forgotten Concepts, Quick Wins (2026-02-25)

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
- **GitHub Issue Status**: `Queued`
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
- **GitHub Issue Status**: `Deferred (blocked)`
- **Blocking Prerequisites**:
  - Active Lens resolution path stable in runtime
  - Distinct physics preset behaviors established
  - Lens/physics binding preference semantics resolved
- **Goal**: Move progressive lens behavior from research notes into planned design/implementation.
- **Scope**:
  - Define trigger semantics (`Always/Ask/Never`, thresholds/interpolation).
  - Specify lens-to-physics binding contract.
- **Dependencies**: Active Lens resolution path + distinct physics presets.
- **Acceptance**:
  - Lens/physics binding is represented in a strategy doc with explicit open questions resolved.
- **Definition of Done**:
  - Progressive lens switching trigger semantics (`Always/Ask/Never`, thresholds/interpolation) are specified.
- **Review Check (comprehension)**:
  - This feature is a policy/interaction problem first; implementing it too early causes surprising behavior.
- **Source refs**:
  - `design_docs/graphshell_docs/research/2026-02-24_interaction_and_semantic_design_schemes.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_physics_engine_extensibility_plan.md`
  - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:70`

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
