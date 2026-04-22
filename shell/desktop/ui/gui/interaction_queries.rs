/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::ui::gui_state::toolbar_location_input_id;
use crate::shell::desktop::ui::nav_targeting;
use crate::util::CoordBridge;

fn active_pane_region_hint(gui: &EguiHost) -> Option<PaneRegionHint> {
    gui.tiles_tree
        .active_tiles()
        .into_iter()
        .find_map(|tile_id| match gui.tiles_tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Graph(_))) => Some(PaneRegionHint::GraphSurface),
            Some(Tile::Pane(TileKind::Node(_))) => Some(PaneRegionHint::NodePane),
            Some(Tile::Pane(TileKind::Tool(_))) => Some(PaneRegionHint::ToolPane),
            _ => None,
        })
}

fn local_widget_focus(gui: &EguiHost) -> Option<LocalFocusTarget> {
    if let Some(local_focus) = gui.runtime.focus_authority.local_widget_focus.clone() {
        Some(local_focus)
    } else if location_has_focus(gui) {
        Some(LocalFocusTarget::ToolbarLocation {
            pane_id: gui.runtime.focus_authority.pane_activation,
        })
    } else if gui.runtime.graph_search_open {
        Some(LocalFocusTarget::GraphSearch)
    } else {
        None
    }
}

pub(super) fn runtime_focus_state(gui: &EguiHost) -> RuntimeFocusState {
    focus_state::desired_runtime_focus_state(
        &gui.runtime.graph_app,
        &gui.runtime.focus_authority,
        local_widget_focus(gui),
        gui.runtime.toolbar_state.show_clear_data_confirm,
    )
}

pub(super) fn runtime_focus_inspector(
    gui: &EguiHost,
) -> crate::shell::desktop::ui::gui_state::RuntimeFocusInspector {
    focus_state::runtime_focus_inspector(
        &gui.runtime.graph_app,
        &gui.runtime.focus_authority,
        local_widget_focus(gui),
        gui.runtime.toolbar_state.show_clear_data_confirm,
    )
}

pub(super) fn focused_node_key(gui: &EguiHost) -> Option<NodeKey> {
    if gui.runtime.graph_surface_focused {
        return None;
    }
    nav_targeting::active_node_pane_node(&gui.runtime.graph_app)
}

pub(super) fn has_focused_node(gui: &EguiHost) -> bool {
    focused_node_key(gui).is_some()
}

pub(super) fn webview_id_for_node_key(gui: &EguiHost, node_key: NodeKey) -> Option<WebViewId> {
    gui.runtime.graph_app.get_webview_for_node(node_key)
}

pub(super) fn active_tile_webview_id(gui: &EguiHost) -> Option<WebViewId> {
    nav_targeting::active_node_pane_node(&gui.runtime.graph_app)
        .and_then(|node_key| gui.runtime.graph_app.get_webview_for_node(node_key))
}

pub(super) fn node_key_for_webview_id(gui: &EguiHost, webview_id: WebViewId) -> Option<NodeKey> {
    gui.runtime.graph_app.get_node_for_webview(webview_id)
}

pub(super) fn location_has_focus(gui: &EguiHost) -> bool {
    let location_id = toolbar_location_input_id(gui.runtime.focus_authority.pane_activation);
    gui.context
        .egui_context()
        .memory(|m| m.focused().is_some_and(|focused| focused == location_id))
}

pub(super) fn request_location_submit(gui: &mut EguiHost) {
    gui.runtime.toolbar_state.editable.location_submitted = true;
}

pub(super) fn request_command_palette_toggle(gui: &mut EguiHost) {
    gui.runtime.command_palette_toggle_requested = true;
}

pub(super) fn egui_wants_keyboard_input(gui: &EguiHost) -> bool {
    gui.context.egui_context().wants_keyboard_input()
}

pub(super) fn egui_wants_pointer_input(gui: &EguiHost) -> bool {
    gui.context.egui_context().wants_pointer_input()
}

pub(super) fn pointer_hover_position(
    gui: &EguiHost,
) -> Option<Point2D<f32, DeviceIndependentPixel>> {
    gui.context
        .egui_context()
        .input(|i| i.pointer.hover_pos())
        .map(|p| p.to_point2d())
}

pub(super) fn ui_overlay_active(gui: &EguiHost) -> bool {
    runtime_focus_state(gui).overlay_active()
}
