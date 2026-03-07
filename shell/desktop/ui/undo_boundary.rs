/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui_tiles::Tree;

use crate::app::{GraphBrowserApp, UndoBoundaryReason};
use crate::shell::desktop::workbench::tile_kind::TileKind;

pub(crate) fn record_workspace_undo_boundary_from_tiles_tree(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    reason: UndoBoundaryReason,
) {
    if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
        graph_app.record_workspace_undo_boundary(Some(layout_json), reason);
    }
}
