/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::ui::gui_state::toolbar_location_input_id;
use crate::util::CoordBridge;

pub(super) fn focused_node_key(gui: &Gui) -> Option<NodeKey> {
    if gui.runtime_state.graph_surface_focused {
        return None;
    }
    tile_compositor::focused_node_pane_for_node_panes(
        &gui.tiles_tree,
        &gui.graph_app,
        gui.runtime_state.focused_node_hint,
    )
    .map(|pane| pane.node_key)
}

pub(super) fn has_focused_node(gui: &Gui) -> bool {
    focused_node_key(gui).is_some()
}

pub(super) fn webview_id_for_node_key(gui: &Gui, node_key: NodeKey) -> Option<WebViewId> {
    gui.graph_app.get_webview_for_node(node_key)
}

pub(super) fn active_tile_webview_id(gui: &Gui) -> Option<WebViewId> {
    tile_compositor::focused_node_pane_for_node_panes(&gui.tiles_tree, &gui.graph_app, None)
        .and_then(|pane| gui.graph_app.get_webview_for_node(pane.node_key))
}

pub(super) fn node_key_for_webview_id(gui: &Gui, webview_id: WebViewId) -> Option<NodeKey> {
    gui.graph_app.get_node_for_webview(webview_id)
}

pub(super) fn location_has_focus(gui: &Gui) -> bool {
    let location_id = toolbar_location_input_id(gui.runtime_state.active_toolbar_pane);
    gui.context
        .egui_context()
        .memory(|m| m.focused().is_some_and(|focused| focused == location_id))
}

pub(super) fn request_location_submit(gui: &mut Gui) {
    gui.toolbar_state.location_submitted = true;
}

pub(super) fn request_command_palette_toggle(gui: &mut Gui) {
    gui.runtime_state.command_palette_toggle_requested = true;
}

pub(super) fn egui_wants_keyboard_input(gui: &Gui) -> bool {
    gui.context.egui_context().wants_keyboard_input()
}

pub(super) fn egui_wants_pointer_input(gui: &Gui) -> bool {
    gui.context.egui_context().wants_pointer_input()
}

pub(super) fn pointer_hover_position(gui: &Gui) -> Option<Point2D<f32, DeviceIndependentPixel>> {
    gui.context
        .egui_context()
        .input(|i| i.pointer.hover_pos())
        .map(|p| p.to_point2d())
}

pub(super) fn ui_overlay_active(gui: &Gui) -> bool {
    ui_overlay_active_from_flags(
        gui.graph_app.workspace.show_command_palette,
        gui.graph_app.workspace.show_help_panel,
        gui.graph_app.workspace.show_radial_menu,
        gui.toolbar_state.show_clear_data_confirm,
    )
}
