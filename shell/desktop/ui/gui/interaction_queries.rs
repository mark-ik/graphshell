/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::ui::gui_state::toolbar_location_input_id;
use crate::util::CoordBridge;

fn active_pane_region_hint(gui: &Gui) -> Option<PaneRegionHint> {
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

fn local_widget_focus(gui: &Gui) -> Option<LocalFocusTarget> {
    if let Some(local_focus) = gui.runtime_state.focus_authority.local_widget_focus.clone() {
        Some(local_focus)
    } else if location_has_focus(gui) {
        Some(LocalFocusTarget::ToolbarLocation {
            pane_id: gui.runtime_state.focus_authority.pane_activation,
        })
    } else if gui.runtime_state.graph_search_open {
        Some(LocalFocusTarget::GraphSearch)
    } else {
        None
    }
}

pub(super) fn runtime_focus_state(gui: &Gui) -> RuntimeFocusState {
    focus_state::desired_runtime_focus_state(
        &gui.graph_app,
        &gui.runtime_state.focus_authority,
        local_widget_focus(gui),
        gui.toolbar_state.show_clear_data_confirm,
    )
}

pub(super) fn runtime_focus_inspector(
    gui: &Gui,
) -> crate::shell::desktop::ui::gui_state::RuntimeFocusInspector {
    focus_state::runtime_focus_inspector(
        &gui.graph_app,
        &gui.runtime_state.focus_authority,
        local_widget_focus(gui),
        gui.toolbar_state.show_clear_data_confirm,
    )
}

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
    let location_id = toolbar_location_input_id(gui.runtime_state.focus_authority.pane_activation);
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
    runtime_focus_state(gui).overlay_active()
}

