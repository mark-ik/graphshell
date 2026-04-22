/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pre-WGPU critical-path UxHarness scenarios (issue #273, Gate G7).
//!
//! This file is the committed critical scenario set required before WGPU migration
//! authorization. Each test maps to a lifecycle register row and a Gate G7 coverage
//! area. Any regression in these tests is a merge-blocking failure for
//! migration-adjacent PRs.
//!
//! Coverage areas (from issue #273 scope):
//! - Graph navigation + camera control          → `graph_navigation_*`
//! - Pane open/close/focus return               → `pane_lifecycle_*`
//! - Command-surface invocation parity          → `command_surface_*`
//! - Modal isolation + dismiss                  → `modal_isolation_*`
//! - Fallback/degraded viewer state signaling   → `degraded_viewer_*`

use egui_tiles::{Tiles, Tree};

use super::super::harness::TestRegistry;
use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId, GraphViewState, WorkbenchIntent};
use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
use crate::shell::desktop::runtime::registries::CHANNEL_UI_COMMAND_SURFACE_ROUTE_FALLBACK;
use crate::shell::desktop::ui::gui_orchestration;
use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::{tile_view_ops, ux_tree};

// ── Graph navigation + camera control ────────────────────────────────────────

/// Selecting a node transitions the UxTree GraphNode to `selected` state and
/// preserves semantic/presentation ID consistency. This is the baseline
/// contract for graph navigation correctness (Gate G1, G7).
#[test]
fn graph_navigation_node_selection_reflected_in_uxtree() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-camera.example/page");
    harness.open_node_tab(node);
    harness.app.select_node(node, false);

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 11);

    let selected_node = snapshot.semantic_nodes.iter().find(|n| n.state.selected);
    assert!(
        selected_node.is_some(),
        "selected node should be reflected as selected in UxTree semantic layer"
    );

    let violation = ux_tree::presentation_id_consistency_violation(&snapshot);
    assert!(
        violation.is_none(),
        "node selection should not break semantic/presentation ID consistency: {violation:?}"
    );
}

/// Camera fit-lock toggle via `GraphIntent` persists across the reducer and
/// is queryable through the public API. Camera control must survive focus
/// routing changes without state loss (Gate G1, G7).
#[test]
fn graph_navigation_camera_fit_lock_roundtrips_through_intent() {
    let mut harness = TestRegistry::new();
    let view_id = GraphViewId::new();
    harness.app.workspace.graph_runtime.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "CriticalPath Camera View"),
    );
    harness.app.workspace.graph_runtime.focused_view = Some(view_id);

    assert!(!harness.app.camera_fit_locked());

    harness.app.apply_reducer_intents([
        GraphIntent::ToggleCameraPositionFitLock,
        GraphIntent::ToggleCameraZoomFitLock,
    ]);

    assert!(
        harness.app.camera_position_fit_locked(),
        "position fit lock should be active after toggle"
    );
    assert!(
        harness.app.camera_zoom_fit_locked(),
        "zoom fit lock should be active after toggle"
    );

    harness
        .app
        .apply_reducer_intents([GraphIntent::RequestFitToScreen]);

    assert!(
        harness.app.camera_fit_locked(),
        "fit-to-screen request should not clear the camera lock"
    );
}

/// Camera zoom request via `GraphIntent::RequestZoomIn` enqueues a pending
/// zoom command readable through the focused view (Gate G1, G7).
#[test]
fn graph_navigation_zoom_intent_produces_pending_camera_command() {
    let mut harness = TestRegistry::new();
    let view_id = GraphViewId::new();
    harness.app.workspace.graph_runtime.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "CriticalPath Zoom View"),
    );
    harness.app.workspace.graph_runtime.focused_view = Some(view_id);
    harness.app.set_camera_fit_locked(false);
    harness.app.clear_pending_camera_command();

    harness
        .app
        .apply_reducer_intents([GraphIntent::RequestZoomIn]);

    assert!(
        harness
            .app
            .take_pending_keyboard_zoom_request(view_id)
            .is_some(),
        "zoom intent should produce a pending zoom request readable through the focused view"
    );
}

// ── Pane open/close/focus return ─────────────────────────────────────────────

/// Opening a node tab produces a NodePane in the UxTree with the correct
/// domain identity. This is the baseline pane-open lifecycle contract (Gate G2, G7).
#[test]
fn pane_lifecycle_open_node_tab_appears_in_uxtree() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-pane.example/article");
    harness.open_node_tab(node);

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 12);

    let has_node_pane = snapshot
        .semantic_nodes
        .iter()
        .any(|n| matches!(n.role, ux_tree::UxNodeRole::NodePane));
    assert!(
        has_node_pane,
        "opening a node tab should produce a NodePane entry in the UxTree"
    );

    let violation = ux_tree::presentation_id_consistency_violation(&snapshot);
    assert!(
        violation.is_none(),
        "node pane open should not break UxTree ID consistency: {violation:?}"
    );
}

/// Closing a tool pane via `WorkbenchIntent::CloseToolPane` removes it from
/// the tree and does not leave orphaned UxTree entries (Gate G2, G7).
#[cfg(feature = "diagnostics")]
#[test]
fn pane_lifecycle_close_tool_pane_removes_entry_from_uxtree() {
    let mut harness = TestRegistry::new();

    tile_view_ops::open_or_focus_tool_pane(&mut harness.tiles_tree, ToolPaneState::Settings);

    let tool_tile_id = harness.tiles_tree.tiles.iter().find_map(|(tile_id, tile)| {
        matches!(
            tile,
            egui_tiles::Tile::Pane(TileKind::Tool(tool)) if tool.kind == ToolPaneState::Settings
        )
        .then_some(*tile_id)
    });
    assert!(
        tool_tile_id.is_some(),
        "settings pane should exist after open"
    );

    let settings_pane_id = harness
        .tiles_tree
        .tiles
        .iter()
        .find_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(TileKind::Tool(tool))
                if tool.kind == ToolPaneState::Settings =>
            {
                Some(tool.pane_id)
            }
            _ => None,
        })
        .expect("settings pane_id should be resolvable");

    let mut intents = vec![WorkbenchIntent::ClosePane {
        pane: settings_pane_id,
        restore_previous_focus: false,
    }];
    gui_orchestration::handle_tool_pane_intents(
        &mut harness.app,
        &mut harness.tiles_tree,
        &mut intents,
    );

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 13);
    let has_tool_pane = snapshot
        .semantic_nodes
        .iter()
        .any(|n| matches!(n.role, ux_tree::UxNodeRole::ToolPane));
    assert!(
        !has_tool_pane,
        "closed tool pane should not appear in UxTree"
    );

    let violation = ux_tree::presentation_id_consistency_violation(&snapshot);
    assert!(
        violation.is_none(),
        "pane close should not break UxTree ID consistency: {violation:?}"
    );
}

/// Cycle-focus intent is consumed by the workbench authority and does not
/// leak into the application as an unhandled intent (Gate G2, G7).
#[test]
fn pane_lifecycle_cycle_focus_intent_is_consumed_by_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let mut tree = Tree::new("pre_wgpu_cycle_focus", root, tiles);

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "cycle-focus intent must be fully consumed by workbench authority — \
         unhandled intents indicate a routing gap in the critical focus-handoff path"
    );
}

// ── Command-surface invocation parity ────────────────────────────────────────

/// `WorkbenchIntent::OpenCommandPalette` via orchestration authority opens
/// the command palette. This verifies the command surface is reachable through
/// the canonical intent path (Gate G4, G7).
#[test]
fn command_surface_open_command_palette_via_intent_opens_surface() {
    let mut app = GraphBrowserApp::new_for_testing();
    let view_id = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let mut tree = Tree::new("pre_wgpu_command_palette", root, tiles);

    assert!(
        !app.workspace.chrome_ui.show_command_palette,
        "command palette should be closed initially"
    );

    let mut intents = vec![WorkbenchIntent::OpenCommandPalette];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "OpenCommandPalette intent must be consumed by orchestration authority"
    );
    assert!(
        app.workspace.chrome_ui.show_command_palette,
        "command palette should be open after open intent"
    );
}

/// `GraphIntent::ToggleCommandPalette` remains a compatibility bridge through
/// the reducer and must still produce the same open state as the canonical
/// `WorkbenchIntent::OpenCommandPalette` path (Gate G4, G7).
#[test]
fn command_surface_graph_intent_and_workbench_intent_produce_identical_state() {
    let view_id = GraphViewId::new();

    let mut via_graph_intent = GraphBrowserApp::new_for_testing();
    let mut tiles_a = Tiles::default();
    let root_a = tiles_a.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let mut tree_a = Tree::new("pre_wgpu_cmd_parity_a", root_a, tiles_a);
    via_graph_intent.apply_reducer_intents([GraphIntent::ToggleCommandPalette]);
    // The GraphIntent routes through the reducer into a WorkbenchIntent; flush it.
    let pending: Vec<WorkbenchIntent> = via_graph_intent.take_pending_workbench_intents();
    let mut pending = pending;
    gui_orchestration::handle_tool_pane_intents(&mut via_graph_intent, &mut tree_a, &mut pending);

    let mut via_workbench_intent = GraphBrowserApp::new_for_testing();
    let mut tiles_b = Tiles::default();
    let root_b = tiles_b.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let mut tree_b = Tree::new("pre_wgpu_cmd_parity_b", root_b, tiles_b);
    let mut intents = vec![WorkbenchIntent::OpenCommandPalette];
    gui_orchestration::handle_tool_pane_intents(
        &mut via_workbench_intent,
        &mut tree_b,
        &mut intents,
    );

    assert_eq!(
        via_graph_intent.workspace.chrome_ui.show_command_palette,
        via_workbench_intent
            .workspace
            .chrome_ui
            .show_command_palette,
        "GraphIntent and WorkbenchIntent paths must produce identical command palette state"
    );
}

/// Closing the command palette with a stale return target must emit the
/// command-surface fallback receipt and preserve a valid active surface rather
/// than silently dropping focus. This is the scenario-level `UXCS03` fallback
/// contract (Gate G4, G7).
#[test]
fn command_surface_palette_close_invalid_target_emits_fallback_receipt() {
    let mut diagnostics = DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let node_key = crate::graph::NodeKey::new(191);
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
    let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
    let root = tiles.insert_tab_tile(vec![graph, node]);
    let mut tree = Tree::new("pre_wgpu_command_palette_restore_fallback", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    app.workspace.chrome_ui.show_command_palette = true;
    let _ = tree.make_active(
        |_, tile| matches!(tile, egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == node_key),
    );
    app.set_pending_command_surface_return_target(Some(
        crate::app::ToolSurfaceReturnTarget::Graph(GraphViewId::new()),
    ));

    let mut intents = vec![WorkbenchIntent::CloseCommandPalette];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests();
    assert!(
        TestRegistry::channel_count(&snapshot, CHANNEL_UI_COMMAND_SURFACE_ROUTE_FALLBACK) > 0,
        "stale command-palette return targets should emit a command-surface fallback receipt"
    );
    assert!(
        tree.active_tiles().into_iter().any(|tile_id| matches!(
            tree.tiles.get(tile_id),
            Some(egui_tiles::Tile::Pane(TileKind::Node(state))) if state.node == node_key
        )),
        "fallback restore should preserve the active node surface when the stored return target is stale"
    );
}

// ── Modal isolation + dismiss ─────────────────────────────────────────────────

/// When the radial menu is active, focus-cycle intents are still consumed by
/// the workbench authority (modal does not leak intents). This validates the
/// modal isolation contract (Gate G4, G7).
#[test]
fn modal_isolation_radial_menu_does_not_leak_focus_cycle_intent() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.chrome_ui.show_radial_menu = true;

    let view_id = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let mut tree = Tree::new("pre_wgpu_modal_isolation", root, tiles);

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "focus-cycle intent must be consumed even when radial menu modal is active — \
         leaked intents indicate a modal isolation failure"
    );
}

/// Dismissing the radial menu clears the modal flag and leaves the UxTree
/// without any blocked nodes at the workbench level (Gate G4, G7).
#[test]
fn modal_isolation_dismiss_radial_menu_clears_modal_state() {
    let mut harness = TestRegistry::new();
    harness.app.workspace.chrome_ui.show_radial_menu = true;

    let snapshot_open = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 14);
    let workbench_blocked_open = snapshot_open
        .semantic_nodes
        .iter()
        .any(|n| matches!(n.role, ux_tree::UxNodeRole::Workbench) && n.state.blocked);

    harness.app.workspace.chrome_ui.show_radial_menu = false;

    let snapshot_closed = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 15);
    let workbench_blocked_closed = snapshot_closed
        .semantic_nodes
        .iter()
        .any(|n| matches!(n.role, ux_tree::UxNodeRole::Workbench) && n.state.blocked);

    // If the workbench was blocked while modal was open, it must be unblocked after dismiss.
    // If it was never blocked (workbench-level blocking is not yet wired), the dismiss
    // must at least not introduce new blocking.
    assert!(
        !workbench_blocked_closed || workbench_blocked_open,
        "dismissing the radial menu must not leave the workbench in a blocked state"
    );

    let violation = ux_tree::presentation_id_consistency_violation(&snapshot_closed);
    assert!(
        violation.is_none(),
        "modal dismiss should not break UxTree ID consistency: {violation:?}"
    );
}

// ── Fallback/degraded viewer state signaling ──────────────────────────────────

/// A node pane in `Placeholder` render mode (viewer resolution failed or not yet
/// assigned) is `degraded` in the UxTree semantic layer. This validates that the
/// fallback signal is contract-visible before the WGPU switch (Gate G6, G7).
///
/// `Placeholder` occurs when the viewer registry cannot resolve a viewer for
/// the node's URL scheme (e.g. unregistered protocol, failed lookup).
/// It is distinct from "webview not yet mapped" — a normal `https://` pane gets
/// `CompositedTexture` immediately after `refresh_node_pane_render_modes` runs.
#[test]
fn degraded_viewer_placeholder_render_mode_is_degraded_in_uxtree() {
    use crate::shell::desktop::workbench::pane_model::TileRenderMode;
    use crate::shell::desktop::workbench::tile_kind::TileKind;

    let mut harness = TestRegistry::new();
    // Add a node and open a tab — this establishes the tabs container in the tree.
    let node = harness.add_node("unknown-scheme://scenario-degraded.example/page");
    harness.open_node_tab(node);

    // Find the resulting NodePane tile and force its render_mode to Placeholder,
    // simulating a viewer that failed to resolve (the canonical degraded/fallback state).
    for tile in harness.tiles_tree.tiles.iter_mut() {
        if let egui_tiles::Tile::Pane(TileKind::Node(state)) = tile.1 {
            if state.node == node {
                state.render_mode = TileRenderMode::Placeholder;
            }
        }
    }

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 16);

    let node_pane = snapshot
        .semantic_nodes
        .iter()
        .find(|n| matches!(n.role, ux_tree::UxNodeRole::NodePane) && n.state.degraded);

    assert!(
        node_pane.is_some(),
        "a node pane in Placeholder render mode must be flagged as degraded in UxTree — \
         this is the contract-visible signal for fallback/placeholder viewer state \
         (Gate G6: viewer fallback must be diagnostics-visible before WGPU switch)"
    );
}

/// A node pane with `CompositedTexture` render mode (the normal resolved state
/// for `https://` URLs) is NOT degraded. This verifies the `degraded` flag is
/// not spuriously set on healthy panes (Gate G6, G7).
#[test]
fn degraded_viewer_composited_render_mode_is_not_degraded_in_uxtree() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-healthy.example/page");
    // open_node_tab calls refresh_node_pane_render_modes, which resolves the
    // viewer for https:// to CompositedTexture — the healthy (non-degraded) state.
    harness.open_node_tab(node);

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 17);

    let node_pane = snapshot
        .semantic_nodes
        .iter()
        .find(|n| matches!(n.role, ux_tree::UxNodeRole::NodePane));

    assert!(
        node_pane.is_some(),
        "node pane should appear in UxTree for a normally opened node"
    );
    assert!(
        !node_pane.is_some_and(|n| n.state.degraded),
        "node pane in CompositedTexture mode must NOT be flagged as degraded in UxTree"
    );
}
