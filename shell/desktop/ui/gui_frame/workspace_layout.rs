/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn handle_pending_history_frame_restore(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    if let Some(layout_json) = graph_app.take_pending_history_frame_layout_json()
        && let Some(restored_tree) = deserialize_history_frame_layout(graph_app, &layout_json)
    {
        apply_restored_history_frame_layout(graph_app, tiles_tree, restored_tree, &layout_json);
    }
}

fn apply_restored_history_frame_layout(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    restored_tree: Tree<TileKind>,
    layout_json: &str,
) {
    *tiles_tree = restored_tree;
    graph_app.mark_session_frame_layout_json(layout_json);
}

fn deserialize_history_frame_layout(
    graph_app: &GraphBrowserApp,
    layout_json: &str,
) -> Option<Tree<TileKind>> {
    match serde_json::from_str::<Tree<TileKind>>(layout_json) {
        Ok(mut restored_tree) => {
            tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
            restored_tree.root().is_some().then_some(restored_tree)
        }
        Err(e) => {
            warn!("Failed to deserialize undo/redo frame snapshot: {e}");
            None
        }
    }
}

pub(super) fn autosave_session_workspace_layout_if_allowed(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if !is_unsaved_workspace_prompt_pending(graph_app) {
        persist_autosave_session_workspace_layout_if_available(graph_app, tiles_tree);
    }
}

pub(super) fn serialize_tiles_tree_layout_json(
    tiles_tree: &Tree<TileKind>,
    context: &str,
) -> Option<String> {
    match serde_json::to_string(tiles_tree) {
        Ok(layout_json) => Some(layout_json),
        Err(e) => {
            warn!("Failed to serialize tile layout for {context}: {e}");
            None
        }
    }
}

fn persist_autosave_session_workspace_layout_if_available(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if let Some(layout_json) = serialize_tiles_tree_layout_json(tiles_tree, "session frame layout")
    {
        graph_app.save_session_workspace_layout_json_if_changed(&layout_json);
    }
}

fn is_unsaved_workspace_prompt_pending(graph_app: &GraphBrowserApp) -> bool {
    graph_app.unsaved_workspace_prompt_request().is_some()
}
