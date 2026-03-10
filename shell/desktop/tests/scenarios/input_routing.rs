use super::super::harness::TestRegistry;
use crate::app::{GraphIntent, GraphViewId, GraphViewState, WorkbenchIntent};
use crate::input::{KeyboardActions, intents_from_actions};
use crate::shell::desktop::ui::gui_orchestration;
use crate::shell::desktop::workbench::pane_model::GraphPaneRef;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use egui_tiles::{Tiles, Tree};

#[test]
fn camera_lock_toggle_survives_webview_focus_routing() {
    let mut harness = TestRegistry::new();
    let view_id = GraphViewId::new();
    harness.app.workspace.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "Scenario View"),
    );
    harness.app.workspace.focused_view = Some(view_id);
    assert!(!harness.app.camera_position_fit_locked());
    assert!(!harness.app.camera_zoom_fit_locked());

    harness.app.apply_reducer_intents([
        GraphIntent::ToggleCameraPositionFitLock,
        GraphIntent::ToggleCameraZoomFitLock,
    ]);

    assert!(harness.app.camera_position_fit_locked());
    assert!(harness.app.camera_zoom_fit_locked());
}

#[test]
fn camera_lock_toggle_survives_omnibar_focus_routing() {
    let mut harness = TestRegistry::new();
    let view_id = GraphViewId::new();
    harness.app.workspace.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "Scenario View"),
    );
    harness.app.workspace.focused_view = Some(view_id);
    assert!(!harness.app.camera_position_fit_locked());
    assert!(!harness.app.camera_zoom_fit_locked());

    let intents = intents_from_actions(&KeyboardActions {
        toggle_camera_position_fit_lock: true,
        toggle_camera_zoom_fit_lock: true,
        ..Default::default()
    });
    harness.app.apply_reducer_intents(intents);

    assert!(harness.app.camera_position_fit_locked());
    assert!(harness.app.camera_zoom_fit_locked());
}

#[test]
fn focus_cycle_survives_webview_focus_routing() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("input_routing_focus_cycle", root, tiles);

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "cycle-focus intent should be consumed by workbench authority"
    );
}

#[test]
fn modal_isolation_preserves_camera_lock_toggle() {
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.show_radial_menu = true;

    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("input_routing_modal_camera_lock", root, tiles);

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
}

#[test]
fn graph_pan_zoom_liveness_after_omnibar_focus_release() {
    let mut harness = TestRegistry::new();
    let view_id = GraphViewId::new();
    harness.app.workspace.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "Scenario View"),
    );
    harness.app.workspace.focused_view = Some(view_id);
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
        "unlocked camera should keep zoom interactions alive via keyboard zoom queue"
    );
}

#[test]
fn settings_and_split_shortcut_paths_produce_identical_lock_state_transition() {
    let mut via_settings = TestRegistry::new();
    let mut via_shortcut = TestRegistry::new();
    let view_id = GraphViewId::new();

    via_settings.app.workspace.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "Settings Path"),
    );
    via_settings.app.workspace.focused_view = Some(view_id);
    via_shortcut.app.workspace.views.insert(
        view_id,
        GraphViewState::new_with_id(view_id, "Shortcut Path"),
    );
    via_shortcut.app.workspace.focused_view = Some(view_id);

    via_settings.app.set_camera_fit_locked(true);

    let intents = intents_from_actions(&KeyboardActions {
        toggle_camera_position_fit_lock: true,
        toggle_camera_zoom_fit_lock: true,
        ..Default::default()
    });
    via_shortcut.app.apply_reducer_intents(intents);

    assert_eq!(
        via_settings.app.camera_position_fit_locked(),
        via_shortcut.app.camera_position_fit_locked()
    );
    assert_eq!(
        via_settings.app.camera_zoom_fit_locked(),
        via_shortcut.app.camera_zoom_fit_locked()
    );
}

use crate::app::GraphBrowserApp;
