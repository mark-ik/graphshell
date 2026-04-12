use super::*;

pub(super) fn handle_update_tile_selection_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    tile_id: egui_tiles::TileId,
    mode: SelectionUpdateMode,
) {
    graph_app.prune_workbench_tile_selection(tiles_tree);
    if !matches!(tiles_tree.tiles.get(tile_id), Some(Tile::Pane(_))) {
        return;
    }
    graph_app.update_workbench_tile_selection(tile_id, mode);
}

pub(super) fn handle_group_selected_tiles_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    graph_app.prune_workbench_tile_selection(tiles_tree);
    let selection = graph_app.workbench_tile_selection().clone();
    let Some((selected_tile_ids, primary_tile_id)) = tile_view_ops::group_selected_tiles(
        tiles_tree,
        &selection.selected_tile_ids,
        selection.primary_tile_id,
    ) else {
        return false;
    };

    graph_app.clear_workbench_tile_selection();
    graph_app.select_workbench_tile(primary_tile_id);
    for tile_id in selected_tile_ids {
        if tile_id != primary_tile_id {
            graph_app.update_workbench_tile_selection(tile_id, SelectionUpdateMode::Add);
        }
    }
    if graph_app
        .persist_workbench_tile_group(tiles_tree, &selection.selected_tile_ids)
        .is_none()
    {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: 1,
        });
    }
    true
}

pub(super) fn handle_detach_node_to_split_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    key: NodeKey,
) {
    record_workspace_undo_boundary_from_tiles_tree(
        graph_app,
        tiles_tree,
        UndoBoundaryReason::DetachNodeToSplit,
    );
    tile_view_ops::detach_node_pane_to_split(tiles_tree, graph_app, key);
}
