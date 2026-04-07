/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Input handling for the graph browser.
//!
//! Keyboard shortcuts are handled here. Mouse interaction (drag, pan, zoom,
//! selection) is handled by egui_graphs via the GraphView widget.

use crate::app::{
    EdgeCommand, GraphBrowserApp, GraphIntent, GraphMutation, ViewAction, WorkbenchIntent,
};
use crate::shell::desktop::runtime::registries::input::{InputBinding, action_id};
use crate::shell::desktop::runtime::registries::phase2_describe_input_bindings;
use crate::util::{GraphshellSettingsPath, VersoAddress};
use egui::Key;

/// Keyboard actions collected from egui input events.
///
/// This struct decouples input detection (requires `egui::Context`) from
/// action application (pure state mutation), making actions testable.
#[derive(Default)]
pub struct KeyboardActions {
    pub toggle_overview_plane: bool,
    pub close_overview_plane: bool,
    pub open_tag_panel: bool,
    pub toggle_physics: bool,
    pub toggle_camera_position_fit_lock: bool,
    pub toggle_camera_zoom_fit_lock: bool,
    pub open_camera_controls: bool,
    pub toggle_view: bool,
    pub fit_to_screen: bool,
    pub open_physics_settings: bool,
    pub toggle_history_manager: bool,
    pub toggle_help_panel: bool,
    pub toggle_command_palette: bool,
    pub toggle_radial_menu: bool,
    pub toggle_workbench_overlay: bool,
    pub close_workbench_overlay: bool,
    pub toggle_semantic_tab_group: bool,
    pub create_node: bool,
    pub connect_selected_pair: bool,
    pub connect_both_directions: bool,
    pub remove_user_edge: bool,
    pub pin_selected: bool,
    pub unpin_selected: bool,
    pub toggle_pin_primary: bool,
    pub zoom_in: bool,
    pub zoom_out: bool,
    pub zoom_reset: bool,
    pub zoom_to_selected: bool,
    pub reheat_physics: bool,
    pub delete_selected: bool,
    pub clear_graph: bool,
    pub select_all: bool,
    pub undo: bool,
    pub redo: bool,
    pub cycle_focus_region: bool,
}

fn key_binding_pressed(input: &egui::InputState, binding: &InputBinding) -> bool {
    match binding {
        InputBinding::Key { modifiers, keycode } => {
            let active_modifiers =
                crate::shell::desktop::runtime::registries::input::ModifierMask::from_egui(
                    &input.modifiers,
                );
            if active_modifiers != *modifiers {
                return false;
            }

            match keycode {
                crate::shell::desktop::runtime::registries::input::Keycode::Named(named) => {
                    let expected = match named {
                        crate::shell::desktop::runtime::registries::input::NamedKey::Enter => {
                            Key::Enter
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::ArrowLeft => {
                            Key::ArrowLeft
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::ArrowRight => {
                            Key::ArrowRight
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::F5 => Key::F5,
                        crate::shell::desktop::runtime::registries::input::NamedKey::F1 => Key::F1,
                        crate::shell::desktop::runtime::registries::input::NamedKey::F2 => Key::F2,
                        crate::shell::desktop::runtime::registries::input::NamedKey::F3 => Key::F3,
                        crate::shell::desktop::runtime::registries::input::NamedKey::F6 => Key::F6,
                        crate::shell::desktop::runtime::registries::input::NamedKey::F7 => Key::F7,
                        crate::shell::desktop::runtime::registries::input::NamedKey::F9 => Key::F9,
                        crate::shell::desktop::runtime::registries::input::NamedKey::Home => {
                            Key::Home
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::Escape => {
                            Key::Escape
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::Delete => {
                            Key::Delete
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::Backspace => {
                            Key::Backspace
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::Plus => {
                            Key::Plus
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::Minus => {
                            Key::Minus
                        }
                        crate::shell::desktop::runtime::registries::input::NamedKey::Num0 => {
                            Key::Num0
                        }
                    };
                    input.key_pressed(expected)
                        || (expected == Key::Plus && input.key_pressed(Key::Equals))
                }
                crate::shell::desktop::runtime::registries::input::Keycode::Char(ch) => {
                    let expected = match ch.to_ascii_lowercase() {
                        'a' => Key::A,
                        'c' => Key::C,
                        'f' => Key::F,
                        'g' => Key::G,
                        'h' => Key::H,
                        'i' => Key::I,
                        'k' => Key::K,
                        'l' => Key::L,
                        'n' => Key::N,
                        'o' => Key::O,
                        'p' => Key::P,
                        '?' => Key::Questionmark,
                        'r' => Key::R,
                        't' => Key::T,
                        'u' => Key::U,
                        'y' => Key::Y,
                        'z' => Key::Z,
                        _ => return false,
                    };
                    input.key_pressed(expected)
                }
            }
        }
        _ => false,
    }
}

fn action_binding_pressed(
    input: &egui::InputState,
    action_id: &str,
    bindings: &[crate::shell::desktop::runtime::registries::input::InputActionBindingDescriptor],
) -> bool {
    bindings
        .iter()
        .filter(|entry| entry.action_id == action_id)
        .filter_map(|entry| entry.current_binding.as_ref())
        .any(|binding| key_binding_pressed(input, binding))
}

/// Collect keyboard actions from the egui context (input detection only).
pub(crate) fn collect_actions(ctx: &egui::Context, graph_app: &GraphBrowserApp) -> KeyboardActions {
    // Don't handle shortcuts while egui is actively capturing keyboard input
    // (for example, URL bar text editing).
    let keyboard_captured_by_egui = ctx.wants_keyboard_input();
    let mut actions = KeyboardActions::default();
    let binding_descriptors = phase2_describe_input_bindings();

    ctx.input(|i| {
        // Escape always works as a dismiss/back affordance for active surfaces.
        if i.key_pressed(Key::Escape) {
            if keyboard_captured_by_egui {
                // Escape will unfocus the text field (handled by egui)
                return;
            }
            if graph_app.graph_view_layout_manager_active() {
                actions.close_overview_plane = true;
            } else if graph_app.workbench_overlay_visible() {
                actions.close_workbench_overlay = true;
            }
        }

        // Home: Toggle view (always works)
        if i.key_pressed(Key::Home) {
            actions.toggle_view = true;
        }

        // F9: open camera controls settings (global shortcut).
        if i.key_pressed(Key::F9) {
            actions.open_camera_controls = true;
        }

        // Skip remaining shortcuts while egui is consuming keyboard input.
        if keyboard_captured_by_egui {
            return;
        }

        if action_binding_pressed(
            i,
            action_id::graph::TOGGLE_OVERVIEW_PLANE,
            &binding_descriptors,
        ) {
            actions.toggle_overview_plane = true;
        }

        if action_binding_pressed(i, action_id::graph::TOGGLE_PHYSICS, &binding_descriptors) {
            actions.toggle_physics = true;
        }

        if action_binding_pressed(i, action_id::graph::NODE_EDIT_TAGS, &binding_descriptors) {
            actions.open_tag_panel = true;
        }

        if action_binding_pressed(i, action_id::graph::ZOOM_IN, &binding_descriptors) {
            actions.zoom_in = true;
        }
        if action_binding_pressed(i, action_id::graph::ZOOM_OUT, &binding_descriptors) {
            actions.zoom_out = true;
        }
        if action_binding_pressed(i, action_id::graph::ZOOM_RESET, &binding_descriptors) {
            actions.zoom_reset = true;
        }

        if action_binding_pressed(
            i,
            action_id::workbench::OPEN_PHYSICS_SETTINGS,
            &binding_descriptors,
        ) {
            actions.open_physics_settings = true;
        }
        if action_binding_pressed(
            i,
            action_id::workbench::OPEN_CAMERA_CONTROLS,
            &binding_descriptors,
        ) {
            actions.open_camera_controls = true;
        }
        if action_binding_pressed(
            i,
            action_id::workbench::OPEN_HISTORY_MANAGER,
            &binding_descriptors,
        ) {
            actions.toggle_history_manager = true;
        }
        if action_binding_pressed(
            i,
            action_id::workbench::TOGGLE_SEMANTIC_TAB_GROUP,
            &binding_descriptors,
        ) {
            actions.toggle_semantic_tab_group = true;
        }

        if action_binding_pressed(i, action_id::graph::NODE_NEW, &binding_descriptors) {
            actions.create_node = true;
        }

        if action_binding_pressed(
            i,
            action_id::graph::TOGGLE_ZOOM_FIT_LOCK,
            &binding_descriptors,
        ) {
            actions.toggle_camera_zoom_fit_lock = true;
        }

        if action_binding_pressed(
            i,
            action_id::graph::TOGGLE_POSITION_FIT_LOCK,
            &binding_descriptors,
        ) {
            actions.toggle_camera_position_fit_lock = true;
        }

        if action_binding_pressed(i, action_id::graph::REHEAT_PHYSICS, &binding_descriptors) {
            actions.reheat_physics = true;
        }

        if action_binding_pressed(i, action_id::graph::EDGE_CONNECT_PAIR, &binding_descriptors) {
            actions.connect_selected_pair = true;
        }
        if action_binding_pressed(i, action_id::graph::EDGE_CONNECT_BOTH, &binding_descriptors) {
            actions.connect_both_directions = true;
        }
        if action_binding_pressed(i, action_id::graph::EDGE_REMOVE_USER, &binding_descriptors) {
            actions.remove_user_edge = true;
        }

        if action_binding_pressed(i, action_id::graph::NODE_PIN_SELECTED, &binding_descriptors) {
            actions.pin_selected = true;
        }
        if action_binding_pressed(
            i,
            action_id::graph::NODE_UNPIN_SELECTED,
            &binding_descriptors,
        ) {
            actions.unpin_selected = true;
        }
        if action_binding_pressed(i, action_id::graph::NODE_PIN_TOGGLE, &binding_descriptors) {
            actions.toggle_pin_primary = true;
        }

        if action_binding_pressed(i, action_id::workbench::HELP_OPEN, &binding_descriptors) {
            actions.toggle_help_panel = true;
        }
        if action_binding_pressed(
            i,
            action_id::graph::COMMAND_PALETTE_OPEN,
            &binding_descriptors,
        ) {
            actions.toggle_command_palette = true;
        }
        if action_binding_pressed(i, action_id::graph::RADIAL_MENU_OPEN, &binding_descriptors) {
            actions.toggle_radial_menu = true;
        }
        if action_binding_pressed(
            i,
            action_id::workbench::TOGGLE_WORKBENCH_OVERLAY,
            &binding_descriptors,
        ) {
            actions.toggle_workbench_overlay = true;
        }

        if action_binding_pressed(i, action_id::graph::NODE_DELETE, &binding_descriptors) {
            actions.delete_selected = true;
        }
        if action_binding_pressed(i, action_id::graph::CLEAR, &binding_descriptors) {
            actions.clear_graph = true;
        }
        if action_binding_pressed(i, action_id::workbench::UNDO, &binding_descriptors) {
            actions.undo = true;
        }
        if action_binding_pressed(i, action_id::workbench::REDO, &binding_descriptors) {
            actions.redo = true;
        }

        if action_binding_pressed(i, action_id::graph::SELECT_ALL, &binding_descriptors) {
            actions.select_all = true;
        }

        if action_binding_pressed(
            i,
            action_id::graph::CYCLE_FOCUS_REGION,
            &binding_descriptors,
        ) {
            actions.cycle_focus_region = true;
        }
    });

    actions
}

/// Convert keyboard actions to graph intents without applying them.
pub fn intents_from_actions(actions: &KeyboardActions) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    if actions.toggle_overview_plane {
        intents.push(GraphIntent::ToggleGraphViewLayoutManager);
    }
    if actions.close_overview_plane {
        intents.push(GraphIntent::ExitGraphViewLayoutManager);
    }
    if actions.toggle_physics {
        intents.push(GraphIntent::TogglePhysics);
    }
    if actions.toggle_camera_position_fit_lock {
        intents.push(ViewAction::ToggleCameraPositionFitLock.into());
    }
    if actions.toggle_camera_zoom_fit_lock {
        intents.push(ViewAction::ToggleCameraZoomFitLock.into());
    }
    // View toggling is owned by GUI tile logic.
    if actions.fit_to_screen {
        intents.push(ViewAction::RequestFitToScreen.into());
    }
    if actions.zoom_in {
        intents.push(ViewAction::RequestZoomIn.into());
    }
    if actions.zoom_out {
        intents.push(ViewAction::RequestZoomOut.into());
    }
    if actions.zoom_reset {
        intents.push(ViewAction::RequestZoomReset.into());
    }
    if actions.zoom_to_selected {
        intents.push(ViewAction::RequestZoomToSelected.into());
    }
    if actions.reheat_physics {
        intents.push(ViewAction::ReheatPhysics.into());
    }
    if actions.create_node {
        intents.push(GraphMutation::CreateNodeNearCenter.into());
    }
    if actions.connect_selected_pair {
        intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::ConnectSelectedPair,
        });
    }
    if actions.connect_both_directions {
        intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::ConnectBothDirections,
        });
    }
    if actions.remove_user_edge {
        intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::RemoveUserEdge,
        });
    }
    if actions.pin_selected {
        intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::PinSelected,
        });
    }
    if actions.unpin_selected {
        intents.push(GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::UnpinSelected,
        });
    }
    if actions.toggle_pin_primary {
        intents.push(GraphIntent::TogglePrimaryNodePin);
    }
    if actions.delete_selected {
        intents.push(GraphMutation::RemoveSelectedNodes.into());
    }
    if actions.clear_graph {
        intents.push(GraphMutation::ClearGraph.into());
    }
    if actions.undo {
        intents.push(GraphIntent::Undo);
    }
    if actions.redo {
        intents.push(GraphIntent::Redo);
    }
    if actions.select_all {
        intents.push(ViewAction::SelectAll.into());
    }
    intents
}

pub fn workbench_intents_from_actions(actions: &KeyboardActions) -> Vec<WorkbenchIntent> {
    let mut intents = Vec::new();
    if actions.toggle_help_panel {
        intents.push(WorkbenchIntent::ToggleHelpPanel);
    }
    if actions.toggle_command_palette {
        intents.push(WorkbenchIntent::ToggleCommandPalette);
    }
    if actions.toggle_radial_menu {
        intents.push(WorkbenchIntent::ToggleRadialMenu);
    }
    if actions.toggle_workbench_overlay {
        intents.push(WorkbenchIntent::SetWorkbenchOverlayVisible { visible: true });
    }
    if actions.close_workbench_overlay {
        intents.push(WorkbenchIntent::SetWorkbenchOverlayVisible { visible: false });
    }
    if actions.toggle_history_manager {
        intents.push(WorkbenchIntent::OpenToolPane {
            kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager,
        });
    }
    if actions.cycle_focus_region {
        intents.push(WorkbenchIntent::CycleFocusRegion);
    }
    intents
}

pub fn dispatch_runtime_requests_from_actions(actions: &KeyboardActions) {
    if actions.open_physics_settings || actions.open_camera_controls {
        crate::shell::desktop::runtime::registries::phase3_publish_settings_route_requested(
            &VersoAddress::settings(GraphshellSettingsPath::Physics).to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphBrowserApp;
    use egui::{Event, Modifiers, RawInput};

    fn test_app() -> GraphBrowserApp {
        GraphBrowserApp::new_for_testing()
    }

    #[test]
    fn test_toggle_view_action_is_gui_owned() {
        let mut app = test_app();
        use euclid::default::Point2D;
        app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        let selected_before = app.focused_selection().clone();
        let count_before = app.workspace.domain.graph.node_count();

        let intents = intents_from_actions(&KeyboardActions {
            toggle_view: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert_eq!(app.focused_selection(), &selected_before);
        assert_eq!(app.workspace.domain.graph.node_count(), count_before);
    }

    #[test]
    fn test_toggle_physics_action() {
        let mut app = test_app();
        let was_running = app.workspace.graph_runtime.physics.base.is_running;

        let intents = intents_from_actions(&KeyboardActions {
            toggle_physics: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert_ne!(
            app.workspace.graph_runtime.physics.base.is_running,
            was_running
        );
    }

    #[test]
    fn test_toggle_camera_position_fit_lock_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            toggle_camera_position_fit_lock: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::ToggleCameraPositionFitLock))
        );
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, GraphIntent::ToggleCameraZoomFitLock))
        );
    }

    #[test]
    fn test_toggle_camera_zoom_fit_lock_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            toggle_camera_zoom_fit_lock: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::ToggleCameraZoomFitLock))
        );
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, GraphIntent::ToggleCameraPositionFitLock))
        );
    }

    #[test]
    fn test_fit_to_screen_action() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        assert!(app.pending_camera_command().is_none());
        app.clear_pending_camera_command();

        let intents = intents_from_actions(&KeyboardActions {
            fit_to_screen: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert!(matches!(
            app.pending_camera_command(),
            Some(crate::app::CameraCommand::Fit)
        ));
    }

    #[test]
    fn test_zoom_in_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            zoom_in: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::RequestZoomIn))
        );
    }

    #[test]
    fn test_zoom_out_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            zoom_out: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::RequestZoomOut))
        );
    }

    #[test]
    fn test_zoom_reset_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            zoom_reset: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::RequestZoomReset))
        );
    }

    #[test]
    fn test_reheat_physics_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            reheat_physics: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::ReheatPhysics))
        );
    }

    #[test]
    fn test_open_physics_settings_action_publishes_settings_route_request() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = crate::shell::desktop::runtime::registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                        crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::SettingsRouteRequested {
                            url,
                        },
                    ) = &signal.kind
                    {
                        seen.lock()
                            .expect("observer lock poisoned")
                            .push(url.clone());
                    }
                Ok(())
            },
        );

        dispatch_runtime_requests_from_actions(&KeyboardActions {
            open_physics_settings: true,
            ..Default::default()
        });

        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|route| {
                    route
                        == &VersoAddress::settings(GraphshellSettingsPath::Physics).to_string()
                })
        );
        assert!(crate::shell::desktop::runtime::registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn test_open_camera_controls_action_publishes_settings_route_request() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = crate::shell::desktop::runtime::registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                        crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::SettingsRouteRequested {
                            url,
                        },
                    ) = &signal.kind
                    {
                        seen.lock()
                            .expect("observer lock poisoned")
                            .push(url.clone());
                    }
                Ok(())
            },
        );

        dispatch_runtime_requests_from_actions(&KeyboardActions {
            open_camera_controls: true,
            ..Default::default()
        });

        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|route| {
                    route
                        == &VersoAddress::settings(GraphshellSettingsPath::Physics).to_string()
                })
        );
        assert!(crate::shell::desktop::runtime::registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn test_toggle_help_panel_action() {
        let intents = workbench_intents_from_actions(&KeyboardActions {
            toggle_help_panel: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, WorkbenchIntent::ToggleHelpPanel))
        );
    }

    #[test]
    fn test_toggle_command_palette_action() {
        let intents = workbench_intents_from_actions(&KeyboardActions {
            toggle_command_palette: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, WorkbenchIntent::ToggleCommandPalette))
        );
    }

    #[test]
    fn test_toggle_radial_menu_action() {
        let intents = workbench_intents_from_actions(&KeyboardActions {
            toggle_radial_menu: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, WorkbenchIntent::ToggleRadialMenu))
        );
    }

    #[test]
    fn test_toggle_workbench_overlay_action() {
        let intents = workbench_intents_from_actions(&KeyboardActions {
            toggle_workbench_overlay: true,
            ..Default::default()
        });
        assert!(
            intents.iter().any(|i| matches!(
                i,
                WorkbenchIntent::SetWorkbenchOverlayVisible { visible: true }
            ))
        );
    }

    #[test]
    fn test_close_workbench_overlay_action() {
        let intents = workbench_intents_from_actions(&KeyboardActions {
            close_workbench_overlay: true,
            ..Default::default()
        });
        assert!(
            intents.iter().any(|i| matches!(
                i,
                WorkbenchIntent::SetWorkbenchOverlayVisible { visible: false }
            ))
        );
    }

    #[test]
    fn test_create_node_action() {
        let mut app = test_app();
        assert_eq!(app.workspace.domain.graph.node_count(), 0);

        let intents = intents_from_actions(&KeyboardActions {
            create_node: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert_eq!(app.workspace.domain.graph.node_count(), 1);
    }

    #[test]
    fn test_connect_selected_pair_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            connect_selected_pair: true,
            ..Default::default()
        });
        assert!(intents.iter().any(|i| matches!(
            i,
            GraphIntent::ExecuteEdgeCommand {
                command: EdgeCommand::ConnectSelectedPair
            }
        )));
    }

    #[test]
    fn test_connect_both_directions_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            connect_both_directions: true,
            ..Default::default()
        });
        assert!(intents.iter().any(|i| matches!(
            i,
            GraphIntent::ExecuteEdgeCommand {
                command: EdgeCommand::ConnectBothDirections
            }
        )));
    }

    #[test]
    fn test_remove_user_edge_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            remove_user_edge: true,
            ..Default::default()
        });
        assert!(intents.iter().any(|i| matches!(
            i,
            GraphIntent::ExecuteEdgeCommand {
                command: EdgeCommand::RemoveUserEdge
            }
        )));
    }

    #[test]
    fn test_pin_selected_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            pin_selected: true,
            ..Default::default()
        });
        assert!(intents.iter().any(|i| matches!(
            i,
            GraphIntent::ExecuteEdgeCommand {
                command: EdgeCommand::PinSelected
            }
        )));
    }

    #[test]
    fn test_unpin_selected_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            unpin_selected: true,
            ..Default::default()
        });
        assert!(intents.iter().any(|i| matches!(
            i,
            GraphIntent::ExecuteEdgeCommand {
                command: EdgeCommand::UnpinSelected
            }
        )));
    }

    #[test]
    fn test_toggle_pin_primary_action_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            toggle_pin_primary: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, GraphIntent::TogglePrimaryNodePin))
        );
    }

    #[test]
    fn test_delete_selected_action() {
        let mut app = test_app();
        use euclid::default::Point2D;
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        assert_eq!(app.workspace.domain.graph.node_count(), 1);

        let intents = intents_from_actions(&KeyboardActions {
            delete_selected: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert_eq!(app.workspace.domain.graph.node_count(), 0);
    }

    #[test]
    fn test_clear_graph_action() {
        let mut app = test_app();
        use euclid::default::Point2D;
        app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        assert_eq!(app.workspace.domain.graph.node_count(), 2);

        let intents = intents_from_actions(&KeyboardActions {
            clear_graph: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert_eq!(app.workspace.domain.graph.node_count(), 0);
    }

    #[test]
    fn test_no_actions_is_noop() {
        let mut app = test_app();
        use euclid::default::Point2D;
        app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let before_count = app.workspace.domain.graph.node_count();
        let before_physics = app.workspace.graph_runtime.physics.base.is_running;

        let intents = intents_from_actions(&KeyboardActions::default());
        app.apply_reducer_intents(intents);

        assert_eq!(app.workspace.domain.graph.node_count(), before_count);
        assert_eq!(
            app.workspace.graph_runtime.physics.base.is_running,
            before_physics
        );
    }

    #[test]
    fn test_select_all_action_selects_every_node() {
        let mut app = test_app();
        use euclid::default::Point2D;
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(20.0, 0.0));

        let intents = intents_from_actions(&KeyboardActions {
            select_all: true,
            ..Default::default()
        });
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert!(app.focused_selection().contains(&c));
        assert_eq!(app.focused_selection().len(), 3);
    }

    #[test]
    fn test_select_all_maps_to_intent() {
        let intents = intents_from_actions(&KeyboardActions {
            select_all: true,
            ..Default::default()
        });
        assert!(intents.iter().any(|i| matches!(i, GraphIntent::SelectAll)));
    }

    #[test]
    fn test_cycle_focus_region_maps_to_intent() {
        let intents = workbench_intents_from_actions(&KeyboardActions {
            cycle_focus_region: true,
            ..Default::default()
        });
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, WorkbenchIntent::CycleFocusRegion))
        );
    }

    fn collect_actions_with_key_event(
        key: Key,
        modifiers: Modifiers,
        capture_keyboard: bool,
    ) -> KeyboardActions {
        let app = test_app();
        let ctx = egui::Context::default();
        let mut actions = KeyboardActions::default();

        let mut raw = RawInput::default();
        raw.modifiers = modifiers;
        raw.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });

        let _ = ctx.run(raw, |ctx| {
            if capture_keyboard {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut input = String::from("capture");
                    let response = ui.add(egui::TextEdit::singleline(&mut input));
                    response.request_focus();
                });
            }
            actions = collect_actions(ctx, &app);
        });

        actions
    }

    fn collect_actions_with_key_event_for_app(
        app: &GraphBrowserApp,
        key: Key,
        modifiers: Modifiers,
        capture_keyboard: bool,
    ) -> KeyboardActions {
        let ctx = egui::Context::default();
        let mut actions = KeyboardActions::default();

        let mut raw = RawInput::default();
        raw.modifiers = modifiers;
        raw.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });

        let _ = ctx.run(raw, |ctx| {
            if capture_keyboard {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut input = String::from("capture");
                    let response = ui.add(egui::TextEdit::singleline(&mut input));
                    response.request_focus();
                });
            }
            actions = collect_actions(ctx, app);
        });

        actions
    }

    #[test]
    fn collect_actions_suppresses_f6_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::F6, Modifiers::default(), true);
        assert!(
            !actions.cycle_focus_region,
            "F6 cycle-focus should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_allows_f6_when_keyboard_is_not_captured() {
        let actions = collect_actions_with_key_event(Key::F6, Modifiers::default(), false);
        assert!(
            actions.cycle_focus_region,
            "F6 cycle-focus should be available when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_maps_c_and_z_to_split_camera_lock_shortcuts_when_not_captured() {
        let c_actions = collect_actions_with_key_event(Key::C, Modifiers::default(), false);
        assert!(
            c_actions.toggle_camera_position_fit_lock,
            "C should toggle position-fit lock when keyboard input is not captured"
        );
        let z_actions = collect_actions_with_key_event(Key::Z, Modifiers::default(), false);
        assert!(
            z_actions.toggle_camera_zoom_fit_lock,
            "Z should toggle zoom-fit lock when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_keeps_f9_global_for_camera_controls_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::F9, Modifiers::default(), true);
        assert!(
            actions.open_camera_controls,
            "F9 should open camera controls even when text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_maps_f1_to_help_panel_toggle_when_not_captured() {
        let actions = collect_actions_with_key_event(Key::F1, Modifiers::default(), false);
        assert!(
            actions.toggle_help_panel,
            "F1 should trigger help panel toggle when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_maps_f2_to_command_palette_toggle_when_not_captured() {
        let actions = collect_actions_with_key_event(Key::F2, Modifiers::default(), false);
        assert!(
            actions.toggle_command_palette,
            "F2 should trigger command palette toggle when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_maps_f3_to_radial_menu_toggle_when_not_captured() {
        let actions = collect_actions_with_key_event(Key::F3, Modifiers::default(), false);
        assert!(
            actions.toggle_radial_menu,
            "F3 should trigger radial menu toggle when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_suppresses_f1_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::F1, Modifiers::default(), true);
        assert!(
            !actions.toggle_help_panel,
            "F1 should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_suppresses_f2_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::F2, Modifiers::default(), true);
        assert!(
            !actions.toggle_command_palette,
            "F2 should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_suppresses_f3_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::F3, Modifiers::default(), true);
        assert!(
            !actions.toggle_radial_menu,
            "F3 should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_suppresses_character_shortcut_n_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::N, Modifiers::default(), true);
        assert!(
            !actions.create_node,
            "single-key N shortcut should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_suppresses_character_shortcut_t_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::T, Modifiers::default(), true);
        assert!(
            !actions.toggle_physics,
            "single-key T shortcut should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_maps_ctrl_t_to_tag_panel_when_not_captured() {
        let actions = collect_actions_with_key_event(
            Key::T,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            false,
        );
        assert!(
            actions.open_tag_panel,
            "Ctrl+T should open the tag panel when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_suppresses_ctrl_t_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(
            Key::T,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            true,
        );
        assert!(
            !actions.open_tag_panel,
            "Ctrl+T should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_maps_ctrl_alt_t_to_semantic_tab_toggle_when_not_captured() {
        let actions = collect_actions_with_key_event(
            Key::T,
            Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
            false,
        );
        assert!(
            actions.toggle_semantic_tab_group,
            "Ctrl+Alt+T should toggle the semantic tab group when keyboard input is not captured"
        );
        assert!(
            !actions.open_tag_panel,
            "Ctrl+Alt+T should remain distinct from the Ctrl+T tag shortcut"
        );
    }

    #[test]
    fn collect_actions_suppresses_ctrl_alt_t_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(
            Key::T,
            Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
            true,
        );
        assert!(
            !actions.toggle_semantic_tab_group,
            "Ctrl+Alt+T should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_maps_ctrl_shift_o_to_overview_toggle_when_not_captured() {
        let actions = collect_actions_with_key_event(
            Key::O,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
            false,
        );
        assert!(
            actions.toggle_overview_plane,
            "Ctrl+Shift+O should toggle the Overview Plane when keyboard input is not captured"
        );
    }

    #[test]
    fn collect_actions_suppresses_ctrl_shift_o_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(
            Key::O,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
            true,
        );
        assert!(
            !actions.toggle_overview_plane,
            "Ctrl+Shift+O should be suppressed while text input captures keyboard"
        );
    }

    #[test]
    fn collect_actions_maps_escape_to_close_overview_when_manager_is_active() {
        let mut app = test_app();
        app.apply_reducer_intents([GraphIntent::EnterGraphViewLayoutManager]);

        let actions =
            collect_actions_with_key_event_for_app(&app, Key::Escape, Modifiers::default(), false);

        assert!(actions.close_overview_plane);
        assert!(!actions.toggle_view);
    }

    #[test]
    fn collect_actions_does_not_map_escape_to_toggle_view_when_no_surface_is_active() {
        let app = test_app();

        let actions =
            collect_actions_with_key_event_for_app(&app, Key::Escape, Modifiers::default(), false);

        assert!(!actions.close_overview_plane);
        assert!(!actions.close_workbench_overlay);
        assert!(!actions.toggle_view);
    }

    #[test]
    fn collect_actions_maps_escape_to_close_workbench_overlay_when_visible() {
        let mut app = test_app();
        app.set_workbench_overlay_visible(true);

        let actions =
            collect_actions_with_key_event_for_app(&app, Key::Escape, Modifiers::default(), false);

        assert!(actions.close_workbench_overlay);
        assert!(!actions.close_overview_plane);
    }

    #[test]
    fn collect_actions_suppresses_character_shortcut_questionmark_when_keyboard_is_captured() {
        let actions = collect_actions_with_key_event(Key::Questionmark, Modifiers::default(), true);
        assert!(
            !actions.toggle_help_panel,
            "single-key ? shortcut should be suppressed while text input captures keyboard"
        );
    }
}
