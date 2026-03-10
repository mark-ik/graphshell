/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn apply_node_focus_state(
    runtime_state: &mut GuiRuntimeState,
    node_key: Option<NodeKey>,
) {
    let was_focused_node_hint = runtime_state.focused_node_hint;
    let was_graph_surface_focused = runtime_state.graph_surface_focused;

    runtime_state.focused_node_hint = node_key;
    runtime_state.graph_surface_focused = false;

    if runtime_state.focused_node_hint != was_focused_node_hint
        || runtime_state.graph_surface_focused != was_graph_surface_focused
    {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
}

pub(super) fn apply_graph_surface_focus_state(
    runtime_state: &mut GuiRuntimeState,
    graph_app: &mut GraphBrowserApp,
    active_graph_view: Option<GraphViewId>,
) {
    let was_focused_node_hint = runtime_state.focused_node_hint;
    let was_graph_surface_focused = runtime_state.graph_surface_focused;
    let was_focused_view = graph_app.workspace.focused_view;

    runtime_state.focused_node_hint = None;
    runtime_state.graph_surface_focused = true;
    graph_app.workspace.focused_view = active_graph_view;

    if runtime_state.focused_node_hint != was_focused_node_hint
        || runtime_state.graph_surface_focused != was_graph_surface_focused
        || graph_app.workspace.focused_view != was_focused_view
    {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
}

pub(super) fn ui_overlay_active_from_flags(
    show_command_palette: bool,
    show_help_panel: bool,
    show_radial_menu: bool,
    show_clear_data_confirm: bool,
) -> bool {
    show_command_palette || show_help_panel || show_radial_menu || show_clear_data_confirm
}
