/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn should_skip_toolbar_location_sync(toolbar_state: &ToolbarState) -> bool {
    toolbar_state.editable.location.trim_start().starts_with('@')
}

pub(super) fn has_any_node_panes(tiles_tree: &Tree<TileKind>) -> bool {
    pane_queries::tree_has_any_node_panes(tiles_tree)
}

pub(super) fn collect_webview_update_flags(gui: &mut EguiHost, window: &EmbedderWindow) -> bool {
    let focused_node_key = gui.focused_node_key();
    toolbar_status_sync::sync_toolbar_webview_status_fields(
        &mut gui.runtime.toolbar_state,
        focused_node_key,
        &gui.runtime.graph_app,
        window,
    ) | gui.update_location_in_toolbar(window, focused_node_key)
}
