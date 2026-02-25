# Immediate Priorities & Forgotten Concepts (2026-02-24)

**Status**: Active / Execution (updated 2026-02-24)
**Context**: Synthesis of roadmap, research, and codebase state.

## 0. Latest Checkpoint Delta (2026-02-24)

- Registry Phase 6.2 boundary hardening advanced: workspace-only reducer path extracted and covered by boundary tests.
- Registry Phase 6.3 single-write-path slices closed for runtime/persistence: direct persistence topology writes were converged to graph-owned helpers, runtime contract coverage now includes persistence runtime sections, and targeted boundary tests are green.
- Registry Phase 6.4 started with a mechanical host subtree move: `running_app_state.rs` and `window.rs` are now canonical under `shell/desktop/host/` with root re-export shims retained during transition.
- Registry Phase 6.4 import canonicalization advanced beyond `shell/desktop/**`: remaining root-shim host imports in `egl/app.rs` and `webdriver.rs` were moved to canonical `shell/desktop/host/*` paths; shim files remain in place for transition compatibility.
- Phase 5 sync UI/action path advanced: pair-by-code decode, async discovery enqueue path, and Phase 5 diagnostics channel + invariant contracts are now in code with passing targeted tests.
- Compile baseline remains green (`cargo check`), warning baseline unchanged.

## 1. Top 10 Priority Tasks (Strategic Blockers)

1.  **Registry Phase 6.4+ Closure**: Continue topology-consolidation closure after 6.3 by executing filesystem move/removal-shim slices (6.4/6.5) while preserving reducer/runtime authority boundaries.
2.  **Verse Tier 1 (Phase 5.4–5.5)**: Complete delta sync and workspace access control done gates (conflict pipeline coverage + access-control harness scenarios).
3.  **Universal Content Model**: Implement `Node` struct extensions (`mime_hint`, `address_kind`) and `ViewerRegistry` selection logic.
4.  **Control UI/UX**: Implement `InputMode` detection and the unified `CommandPalette` / `RadialMenu` logic (extract from `render/mod.rs`).
5.  **Multi-Graph Pane**: Finish the UI for splitting panes and assigning Lenses (backend types `GraphViewId`/`GraphViewState` exist; UI is missing).
6.  **Layout Behaviors**: Implement "Reheat on Node Add" and "New Node Placement" logic (prevent center-spawn clutter).
7.  **Spatial Accessibility**: Implement the `WebView` accessibility bridge (Phase 1) to allow screen readers to enter web content.
8.  **Performance Tuning**: Implement Viewport Culling in `render/mod.rs` (Phase 1).
9.  **Wry Integration**: Implement `WryManager` and `WryViewer` for Windows compatibility.
10. **Bookmarks/History Import**: Implement the `ImportWizardMod` native mod.

---

## 2. Top 10 Forgotten Concepts (Vision & Research Gaps)

Items defined in research/vision docs but currently missing from the active execution queue.

1.  **Visual Tombstones**: Ghost nodes/edges to preserve structure after deletion (`2026-02-24_visual_tombstones_research.md`).
2.  **Temporal Navigation**: Time-travel slider for graph state (`GRAPHSHELL_AS_BROWSER.md`).
3.  **Semantic Fisheye**: Distortion-free focus+context rendering (`2026-02-18_graph_ux_research_report.md`).
4.  **Lasso Zoning**: Spatial rules/magnetic zones for organizing nodes (`PROJECT_DESCRIPTION.md`).
5.  **Collaborative Ghost Cursors**: Real-time presence in P2P sessions (`GRAPHSHELL_AS_BROWSER.md`).
6.  **Audio-Reactive Layout**: Physics driven by audio input (`2026-02-24_physics_engine_extensibility_plan.md`).
7.  **Gemini Protocol**: Native support for the Gemini web (`2026-02-24_universal_content_model_plan.md`).
8.  **Interactive HTML Export**: Self-contained graph exports (`PROJECT_PHILOSOPHY.md`).
9.  **Crawler Economy**: Bounty model for external web ingestion (Verse Tier 2).
10. **Graph-to-List Conversion**: Linearized view for accessibility/screen readers (`2026-02-24_spatial_accessibility_plan.md`).

---

## 3. Top 10 Quickest Improvements (Tactical Refactors)

Low-effort, high-value changes to clean up debt or unblock features.

1.  **Extract `radial_menu.rs`**: Move ~300 lines from `render/mod.rs` to `desktop/radial_menu.rs`.
2.  **Extract `command_palette.rs`**: Move ~150 lines from `render/mod.rs` to `desktop/command_palette.rs`.
3.  **Add `mime_hint` & `address_kind`**: Update `Node` struct in `graph/mod.rs` (Unblocks Universal Content).
4.  **Reheat on Node Add**: Add `physics.is_running = true` to `add_node_and_sync` in `app.rs`.
5.  **Implement `InputMode`**: Add enum and detection logic to `app.rs` (Unblocks Control UI).
6.  **Rename "Functional Physics"**: Ensure all UI strings match "Canvas Editor" (Consistency).
7.  **Add `LayoutMode` enum**: Add to `app.rs` if missing (Unblocks Lens config).
8.  **Fix `WebViewUrlChanged` ordering**: Ensure `push_traversal` captures prior URL in `app.rs` before update.
9.  **Add `CanvasRegistry` toggles**: Add fields like `viewport_culling_enabled` to the registry struct.
10. **Standardize `GraphAction`**: Ensure all UI interactions in `render/mod.rs` map to `GraphAction` enum before processing.

---

## Execution Strategy

1.  **Execute Quickest Improvements #1 & #2** (UI Extraction) to clean up `render/mod.rs`.
2.  **Execute Quickest Improvement #4** (Physics Reheat) to fix the "dead graph" feel on node add.
3.  **Execute Section 4 closure backlog in order** (Phase 5.4 → 5.5 → 6.4 → 6.5) to close remaining registry-plan done gates.

---

## 4. Registry Plan Closure Backlog (Audited 2026-02-24)

This is the strict closure checklist derived from the current `2026-02-22_registry_layer_plan.md` state and code/test audit.

### 4.1 Phase 5.4 — Delta Sync Done-Gate Closure

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

### 4.2 Phase 5.5 — Workspace Access Control Done-Gate Closure

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

### 4.3 Phase 6.4 — Filesystem/Import Canonicalization Closure

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

### 4.4 Phase 6.5 — Transition Shim Removal & Final Boundary Lock

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

### 4.5 Immediate Next Sequence (Recommended Order)

1. Implement `verse_delta_sync_basic` + conflict diagnostics channels.
2. Implement `verse_access_control` harness and deny-path assertions.
3. Complete remaining 6.4 import canonicalization (`persistence` path cleanup).
4. Execute 6.5 shim removal in one controlled slice with full-suite validation.