/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn restore_startup_session_frame_if_available(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    if let Some(layout_json) = graph_app
        .load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
        if restored_tree.root().is_some() {
            graph_app.mark_session_frame_layout_json(&layout_json);
            log::debug!("gui: restored startup session frame from session layout json");
            *tiles_tree = restored_tree;
            return true;
        }
    }

    if let Ok(bundle) = persistence_ops::load_named_frame_bundle(
        graph_app,
        GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
    ) && let Ok((restored_tree, _)) =
        persistence_ops::restore_runtime_tree_from_frame_bundle(graph_app, &bundle)
        && restored_tree.root().is_some()
    {
        if let Ok(runtime_layout_json) = serde_json::to_string(&restored_tree) {
            graph_app.mark_session_frame_layout_json(&runtime_layout_json);
        }
        log::debug!("gui: restored startup session frame from legacy session bundle");
        *tiles_tree = restored_tree;
        return true;
    }

    if let Some(layout_json) = graph_app.load_tile_layout_json()
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
        if restored_tree.root().is_some() {
            graph_app.mark_session_frame_layout_json(&layout_json);
            log::debug!("gui: restored startup session frame from compatibility layout json");
            *tiles_tree = restored_tree;
            return true;
        }
    }

    false
}