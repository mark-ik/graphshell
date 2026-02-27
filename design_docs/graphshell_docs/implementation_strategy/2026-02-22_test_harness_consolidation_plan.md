# Test Harness Consolidation Plan (2026-02-22)

**Status**: In Progress
**Goal**: Consolidate integration tests into a unified harness driven by the Diagnostic System, enabling "Observability-Driven Testing" and high automation coverage.

> **Terminology note (2026-02-26)**: The struct this document calls `TestHarness` has been renamed
> to `TestRegistry` in code and canonical docs. The name `TestHarness` now refers to the planned
> in-pane runner (feature-gated, background execution, panic isolation). Read `TestRegistry`
> wherever this document refers to the `cargo test` fixture struct. See `SUBSYSTEM_DIAGNOSTICS.md §4`.

## Consolidated Checkpoint (2026-02-23)

Completed in this checkpoint:
- Migrated persistence switching scenario to harness:
    - `switch_persistence_dir_reloads_graph_state` -> `desktop/tests/scenarios/persistence.rs`
- Migrated one preference persistence scenario to harness:
    - `set_toast_anchor_preference_persists_across_restart` -> `desktop/tests/scenarios/persistence.rs`
- Added first grouping-intent scenario coverage:
    - `create_user_grouped_edge_from_primary_selection_creates_grouped_edge` -> `desktop/tests/scenarios/grouping.rs`
- **Added semantic tagging scenario coverage:**
    - `set_node_pinned_intent_syncs_pin_tag` -> `desktop/tests/scenarios/tags.rs`
    - `tag_node_pin_updates_pinned_state` -> `desktop/tests/scenarios/tags.rs`
- Removed migrated duplicate tests from `app.rs` in same slice.

Validation evidence for this checkpoint:
- `cargo test desktop::tests::scenarios::persistence:: -- --nocapture` (pass, 12 tests)
- `cargo test desktop::tests::scenarios::grouping:: -- --nocapture` (pass, 1 test)
- `cargo test desktop::tests::scenarios::tags:: -- --nocapture` (pass, 2 tests)
- `cargo test desktop::tests::scenarios::registries -- --nocapture` (pass)
- `cargo check` (pass)
- Full scenario matrix: 49 tests passing

## Context
Currently, tests are scattered across modules (`gui_tests.rs`, `persistence_ops.rs`, `app.rs`). Many rely on internal visibility or fragile state checks.
We have introduced a robust **Diagnostic System** (`DiagnosticsState`, `DiagnosticEvent`) that exposes the system's internal topology and state as structured data.
We will leverage this to build a unified `TestHarness` that treats the app as a black box (mostly) and asserts on diagnostic signals.

## Architecture

### 1. The `src/tests/` Module
A new top-level module (or `desktop/tests/`) to house the harness and scenarios.

- `harness.rs`: Wraps `GraphBrowserApp` + `Gui` (headless) + `DiagnosticsState`.
- `scenarios/`: Submodules for specific feature areas (e.g., `routing.rs`, `layout.rs`).

### 2. The `TestHarness` Struct
```rust
pub struct TestHarness {
    pub app: GraphBrowserApp,
    pub gui: Gui, // Headless/Test configuration
    pub events: Receiver<DiagnosticEvent>,
}

impl TestHarness {
    pub fn new() -> Self { ... }
    pub fn step(&mut self) { ... } // Runs one frame/tick
    pub fn click_node(&mut self, key: NodeKey) { ... }
    pub fn assert_intent(&self, predicate: impl Fn(&GraphIntent) -> bool) { ... }
    pub fn snapshot(&self) -> Value { ... } // Returns DiagnosticsState snapshot
}
```

### 3. Observability-Driven Testing
Instead of checking `app.selected_nodes.len()`, we check:
1.  **Intents**: Did the action produce the expected `GraphIntent`?
2.  **Compositor State**: Does the `CompositorFrameSample` show the expected tiles and rects?
3.  **Engine Topology**: Did the channel message counts increment?

---

## Tutorial: Solving the Black Tile Bug

**The Bug**: WebView tiles appear in the tree but render as black/empty.
**The Cause**: The tile exists in `egui_tiles`, but the `GraphBrowserApp` has not mapped a `WebViewId` to the `NodeKey`, or the `OffscreenRenderingContext` is missing.

### Step 1: Write the Repro Test
We create a test scenario that opens a node and asserts that it is "visible" in the compositor.

```rust
#[test]
fn test_webview_tile_renders_correctly() {
    let mut harness = TestHarness::new();
    let node = harness.add_node("https://example.com");
    
    // Action: Open the node in a tile
    harness.app.request_open_node_tile_mode(node, PendingTileOpenMode::Tab);
    harness.step(); // Process intents
    harness.step(); // Run layout/compositor

    // Assertion: Check Diagnostic Snapshot
    let snapshot = harness.snapshot();
    let tile = find_tile_for_node(&snapshot, node);
    
    assert!(tile.is_some(), "Tile should exist in tree");
    let tile = tile.unwrap();
    
    // The Bug: These assertions fail if the tile is black
    assert!(tile.mapped_webview, "Node must map to a WebViewId");
    assert!(tile.has_context, "Tile must have a GL context");
    assert!(tile.rect.width() > 0.0, "Tile must have non-zero size");
}
```

### Step 2: Run and Fail
Running this test reveals:
`assertion failed: tile.mapped_webview` -> The app logic created the tile but didn't fire `MapWebviewToNode`.

### Step 3: Fix and Confirm
We modify `desktop/tile_view_ops.rs` or `app.rs` to ensure the mapping intent is fired.
Re-running the test passes.

---

## Implementation Stages

### Stage 1: Harness Foundation
1.  Create `desktop/tests/harness.rs`.
2.  Implement `TestHarness::new()` with a headless `Gui` instance.
3.  Expose `DiagnosticsState` from `Gui` (already done).

**Stage 1 progress (2026-02-22):**
- Added `desktop/tests` scaffold:
    - `desktop/tests/mod.rs`
    - `desktop/tests/harness.rs`
    - `desktop/tests/scenarios/mod.rs`
    - `desktop/tests/scenarios/black_tile.rs`
- Wired test module registration in `desktop/mod.rs` under `#[cfg(test)]`.
- Added minimal `TestHarness` API for observability-driven assertions:
    - app construction + node creation/open helpers
    - diagnostics-driven frame sampling and snapshot extraction
    - snapshot helpers for tile/channel assertions
- Added initial scenario coverage:
    - `webview_tile_snapshot_reports_mapping_and_context_health`
    - `engine_snapshot_exposes_servo_runtime_channels`
- Added test-only diagnostics accessors used by harness:
    - `DiagnosticsState::force_drain_for_tests`
    - `DiagnosticsState::snapshot_json_for_tests`
- Validation:
    - `cargo test desktop::tests::scenarios::black_tile:: -- --nocapture` (pass)
    - `cargo check --message-format short` (pass)

### Stage 2: Migration
1.  Move `workspace_routing` tests to the harness.
2.  Move `persistence` tests to the harness.

**Stage 2 progress (2026-02-22):**
- Migrated first `workspace_routing` case into harness scenarios:
    - Added `desktop/tests/scenarios/routing.rs`
    - Added test: `open_node_workspace_routed_falls_back_to_current_workspace_for_zero_membership`
- Registered routing scenario in `desktop/tests/scenarios/mod.rs`.
- Removed duplicated original test from `app.rs` to keep migration authoritative.
- Stabilized harness diagnostics assertions by switching from global diagnostics emission
    to harness-local test-only diagnostics event injection.
- Continued routing migration with additional cases:
    - `open_node_workspace_routed_with_preferred_workspace_requests_restore`
    - `remove_selected_nodes_clears_workspace_membership_entry`
    - `resolve_workspace_open_prefers_recent_membership`
    - `resolve_workspace_open_honors_preferred_workspace`
    - `set_node_url_preserves_workspace_membership`
- Removed duplicated originals from `app.rs` for migrated cases above.
- Started persistence-focused Stage 2 migration:
    - Added `desktop/tests/scenarios/persistence.rs`
    - Added tests:
        - `open_node_workspace_routed_preserves_unsaved_prompt_state_until_restore`
        - `workspace_has_unsaved_changes_for_graph_mutations`
        - `workspace_not_modified_for_non_graph_mutations`
        - `workspace_not_modified_for_set_node_position`
        - `workspace_has_unsaved_changes_for_set_node_pinned`
    - Removed duplicated originals from `app.rs` for migrated persistence cases.
- Continued persistence migration with unsaved-prompt/save-state cases:
    - `workspace_modified_for_graph_mutations_even_when_not_synthesized`
    - `unsaved_prompt_warning_resets_on_additional_graph_mutation`
    - `save_named_workspace_clears_unsaved_prompt_state`
    - Removed duplicated originals from `app.rs` for migrated cases above.
- Validation:
    - `cargo test desktop::tests::scenarios::persistence:: -- --nocapture` (pass, 8 tests)
    - `cargo test desktop::tests::scenarios::routing:: -- --nocapture` (pass)
    - `cargo test desktop::tests::scenarios::black_tile:: -- --nocapture` (pass)
    - `cargo check` (pass)

### Stage 3: Expansion
1.  Add `CompositorFrameSample` assertions for layout tests.
2.  Add `Engine` topology assertions for performance tests.

**Stage 3 progress (2026-02-22):**
- Added `desktop/tests/scenarios/layout.rs` and registered it in `desktop/tests/scenarios/mod.rs`.
- Added initial layout/compositor scenario coverage:
    - `compositor_frames_capture_sequence_and_active_tile_count_transitions`
    - `compositor_tile_rects_are_non_zero_in_healthy_layout_path`
    - `healthy_layout_path_keeps_active_tile_violation_channel_zero`
    - `unhealthy_layout_signal_is_observable_via_active_tile_violation_channel`
- Expanded Stage 3 layout coverage with topology assertions:
    - `compositor_multi_tile_layout_samples_have_non_overlapping_rects`
    - `compositor_hierarchy_samples_include_split_container_and_child_tiles`
- Migrated Session Autosave/Retention tests into harness scenarios (`desktop/tests/scenarios/persistence.rs`):
    - `session_workspace_blob_autosave_uses_runtime_layout_hash_and_caches_runtime_layout`
    - `session_workspace_blob_autosave_rotates_previous_latest_bundle_on_layout_change`
- Removed migrated autosave/retention duplicates from `app.rs`.
- Validation:
    - `cargo test desktop::tests::scenarios::layout:: -- --nocapture` (pass, 6 tests)
    - `cargo test desktop::tests::scenarios::persistence:: -- --nocapture` (pass, 10 tests)
    - `cargo test desktop::tests::scenarios::routing:: -- --nocapture` (pass, 6 tests)
    - `cargo test desktop::tests::scenarios::black_tile:: -- --nocapture` (pass, 2 tests)
    - `cargo check` (pass)

**Stage 3 scope (next):**
- Add a `layout.rs` scenario module under `desktop/tests/scenarios/` for tile geometry + viewport invariants.
- Add scenario assertions for:
    - active tile count transitions during open/close flows
    - stable non-zero tile rects after routing and restore
    - active-tile invariant channels (`tile_render_pass.active_tile_violation`) remaining zero in healthy paths
- Add Engine topology checks for hot-path channels and percentile latency controls where deterministic in tests.

**Stage 3 acceptance criteria:**
- Layout/compositor assertions execute only via harness snapshots (no private-field reach-through).
- At least one scenario fails when an expected mapping/context invariant is intentionally broken.
- Existing Stage 2 suites continue to pass unchanged.

### Stage 4: Completion + Hardening
1.  Finalize migration coverage and remove obsolete duplicates from legacy test locations.
2.  Document the harness as the default integration test entrypoint.
3.  Keep targeted command matrix stable for CI/local validation.

**Stage 4 acceptance criteria:**
- Migrated tests live in `desktop/tests/scenarios/*` with no equivalent duplicates in `app.rs`.
- Consolidation doc includes final migrated test inventory and command matrix.
- `cargo test desktop::tests::scenarios:: -- --nocapture` and `cargo check` are green.

---

## Consolidation Inventory (Full Scope)

This inventory maps all functional areas to migration stages.

### Phase A: Core Architecture (Stages 1-2) - **Active**
- [x] Harness Scaffold
- [x] Workspace Routing (Basic)
- [x] Persistence (Basic)
- [x] **Session Autosave & Retention**
- `test_session_workspace_blob_autosave_uses_runtime_layout_hash_and_caches_runtime_layout`
- `test_session_workspace_blob_autosave_rotates_previous_latest_bundle_on_layout_change`
- [~] **Persistence Switching & Preferences**
- [x] `test_switch_persistence_dir_reloads_graph_state`
- [x] Preference persistence (toast anchor)
- [ ] Remaining preference persistence (lasso binding, shortcuts)

### Phase B: Layout & Compositor (Stage 3) - **Active**
- [x] **Tile Geometry Invariants**
    - Verify active tile count matches expected open nodes.
    - Verify tile rects are non-zero and non-overlapping (sanity check).
    - Verify `tile_render_pass.active_tile_violation` channel remains zero.
- [~] **Multi-Pane Grouping**
    - Verify split operations create correct container hierarchy.
    - [x] Verify drag-to-group creates `UserGrouped` edges (via intent inspection).

### Phase C: Interaction & Control Plane (Stage 4)
- [ ] **Navigation & History**
    - Verify `Back`/`Forward` intents update history index correctly.
    - Verify `WebViewUrlChanged` triggers correct node updates.
    - **Validation**: Replaces manual "Navigation: Back/Forward Delegate Event Ordering" checks.
- [~] **Graph Interactions & Semantic Tagging**
    - Verify `SelectNode` (single/multi) updates selection state.
    - Verify `CreateUserGroupedEdge` intents are emitted on grouping actions.
    - [x] Verify `SetNodePinned` intent synchronizes with `#pin` semantic tag (tags scenario).
    - [x] Verify `TagNode`/`UntagNode` for `#pin` updates `node.is_pinned` state (tags scenario).
    - Verify `Undo`/`Redo` restores previous snapshot state.
- [ ] **Search & Filtering**
    - Verify Omnibar `@` scopes filter results correctly (using harness snapshot of matches).
    - Verify Graph Search (Ctrl+F) highlights/filters nodes.

### Phase C.1: Undo/Redo Logic (Consolidated)
- [ ] **Stack Mechanics**
    - `test_capture_undo_checkpoint_pushes_and_clears_redo`
    - `test_undo_stack_trimmed_at_max`
    - `test_new_action_clears_redo_stack`
- [ ] **State Restoration**
    - `test_perform_undo_reverts_to_previous_graph`
    - `test_perform_redo_reapplies_after_undo`
    - `test_undo_returns_false_when_stack_empty`

### Phase C.2: Workspace Routing Explainability (Consolidated)
- [ ] **Resolver Trace Coverage**
    - `test_resolve_workspace_open_emits_trace_candidates_ranking_and_reason`
    - `test_resolve_workspace_open_explicit_target_trace_reason`
- [ ] **Membership Affordance Coverage**
    - `test_membership_badge_hides_for_local_only_membership`
    - `test_workspace_target_palette_actions_include_membership_hint`
- [ ] **Batch Operation Observability**
    - `test_prune_empty_workspaces_emits_intent_and_diagnostics`
    - `test_retention_sweep_emits_intent_and_diagnostics`

### Phase D: Performance & Engine (Stage 5)
- [ ] **Engine Topology**
    - Verify channel message counts increment during activity.
    - Verify latency percentiles stay within bounds (using simulated clock if possible).

---

## Migration Strategy per Area

1.  **Identify**: Locate existing tests in `app.rs`, `gui_tests.rs`, or `test_guide.md`.
2.  **Port**: Rewrite as a scenario in `desktop/tests/scenarios/<area>.rs` using `TestHarness`.
3.  **Verify**: Run the new scenario.
4.  **Delete**: Remove the old test code or manual checklist item.

---

## Command Matrix (authoritative)

Run after each migration increment:

- `cargo test desktop::tests::scenarios::layout:: -- --nocapture`
- `cargo test desktop::tests::scenarios::persistence:: -- --nocapture`
- `cargo test desktop::tests::scenarios::routing:: -- --nocapture`
- `cargo test desktop::tests::scenarios::registries:: -- --nocapture`
- `cargo test desktop::tests::scenarios::tags:: -- --nocapture`
- `cargo test desktop::tests::scenarios::black_tile:: -- --nocapture`
- `cargo check`

Run at stage boundaries:

- `cargo test desktop::tests::scenarios:: -- --nocapture`

---

## Risks and Mitigations

- **Risk: black-box drift back to private-field assertions**
    - Mitigation: prefer public app API + diagnostics snapshot assertions; only add test-only accessors when unavoidable and scoped.
- **Risk: duplicate tests diverge between `app.rs` and scenarios**
    - Mitigation: remove migrated originals in the same PR/change-set.
- **Risk: flaky diagnostics-based assertions**
    - Mitigation: use harness-local deterministic event/sample injection paths rather than global channel timing.

---

## Done vs Remaining (as of 2026-02-23)

**Done**
- Stage 1 harness scaffold complete and green.
- Stage 2 routing migration increment complete (6 routing scenarios).
- Stage 2 persistence migration increment complete for unsaved prompt + workspace mutation semantics (10 tests).
- Stage 3 expansion complete: non-overlap, split/container hierarchy, layout compositor assertions (6 layout tests).
- Phase A Session Autosave & Retention migration complete (scenarios authoritative; `app.rs` duplicates removed).
- Phase A Persistence Switching migration complete (`switch_persistence_dir_reloads_graph_state`).
- Phase A Preference Persistence: toast anchor migrated.
- Semantic tagging scenarios added: `#pin` tag sync and `TagNode`/`UntagNode` state (2 tests in `tags.rs`).
- First grouping-intent scenario: `create_user_grouped_edge_from_primary_selection_creates_grouped_edge` (1 test in `grouping.rs`).
- Registries scenarios passing (`cargo test desktop::tests::scenarios::registries -- --nocapture`).
- Full scenario matrix: **49 tests passing** (2026-02-23 checkpoint).

**Remaining**

- Phase A: remaining preference persistence tests (lasso binding, shortcut bindings).
- Phase C.1: Undo/Redo stack mechanics and state restoration scenarios.
- Phase C.2: Workspace routing explainability (resolver trace, membership affordance, batch observability).
- Phase C: Navigation & History scenarios (`Back`/`Forward` intent ordering).
- Phase C: Search & Filtering harness assertions (Omnibar scopes, Ctrl+F).
- Stage 4 final migration cleanup and closure pass (remove remaining legacy duplicates, update command matrix evidence).

---

## Next Execution Slice

Immediate next batch:

1. Migrate remaining preference persistence tests (lasso binding + shortcut bindings) into `desktop/tests/scenarios/persistence.rs`.
2. Add Phase C.1 Undo/Redo scenarios (`stack_mechanics` + `state_restoration` sub-modules or inline in a new `undo.rs`).
3. Extend grouping coverage to split/container hierarchy semantics in harness-observable outputs.
4. Run full stage-boundary matrix and append evidence: `cargo test desktop::tests::scenarios:: -- --nocapture`.

---

## Governance: Ensuring Future Compliance

To ensure future features respect the Observability-Driven Testing paradigm:

1.  **The "No Invisible State" Rule**: Any new state field in `GraphBrowserApp` or `Gui` that affects logic must be exposed in `DiagnosticsState` (via snapshot or event). If you can't see it in the Inspector, you can't test it reliably.
2.  **The "Intent-First" Rule**: All user-visible changes must flow through `GraphIntent`. Direct mutation of state from UI code is forbidden (except for transient visual-only state like hover highlights). This ensures the "Intents" tab is always the source of truth.
3.  **Test Harness Requirement**: New features must include a `scenarios/*.rs` integration test. `#[test]` functions in implementation files are reserved for pure unit logic (e.g. math, parsing) only.

## Definition of "Comprehensive Diagnostics"

We define "Comprehensive" based on the component's ownership:

### 1. Graphshell (Owned)
- **Target**: 100% State Visibility.
- **Metric**: Every `GraphIntent` variant is logged. Every `TileKind` is inspectable. Every persistence IO operation emits a start/end span.

### 2. Servo (Embedded)
- **Target**: 100% Boundary Visibility.
- **Metric**: We cannot instrument Servo internals (DOM/JS engine) without forking. Instead, we instrument the **Bridge**: every Delegate callback, every IPC message size/latency, and every resource request. We treat Servo as a black box with a highly instrumented surface.

### 3. Mods / Plugins (Hosted)
- **Target**: 100% Host Boundary Visibility.
- **Metric**: When we add Wasm mods, we instrument the **Host Runtime**. We trace when a mod is invoked, how much CPU/Memory it uses, and exactly what `GraphIntent`s it emits. We do not trace inside the mod's binary.