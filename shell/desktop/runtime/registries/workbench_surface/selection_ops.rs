use super::*;

pub(super) fn handle_update_pane_selection_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    pane_id: PaneId,
    mode: SelectionUpdateMode,
) {
    graph_app.prune_workbench_pane_selection(tiles_tree);
    // Verify the pane still exists in the tile tree.
    let pane_exists = tiles_tree.tiles.iter().any(|(_, tile)| match tile {
        Tile::Pane(kind) => kind.pane_id() == pane_id,
        _ => false,
    });
    if !pane_exists {
        return;
    }
    graph_app.update_workbench_pane_selection(pane_id, mode);
}

pub(super) fn handle_group_selected_tiles_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    graph_app.prune_workbench_pane_selection(tiles_tree);
    let selection = graph_app.workbench_tile_selection().clone();

    // Convert PaneId selection to TileId selection for tree manipulation.
    let selected_tile_ids: std::collections::HashSet<egui_tiles::TileId> = tiles_tree
        .tiles
        .iter()
        .filter_map(|(tid, tile)| match tile {
            Tile::Pane(kind) if selection.selected_pane_ids.contains(&kind.pane_id()) => {
                Some(*tid)
            }
            _ => None,
        })
        .collect();
    let primary_tile_id = selection.primary_pane_id.and_then(|pane_id| {
        tiles_tree.tiles.iter().find_map(|(tid, tile)| match tile {
            Tile::Pane(kind) if kind.pane_id() == pane_id => Some(*tid),
            _ => None,
        })
    });

    let Some((result_tile_ids, primary_result_tile_id)) =
        tile_view_ops::group_selected_tiles(tiles_tree, &selected_tile_ids, primary_tile_id)
    else {
        return false;
    };

    // Convert resulting TileIds back to PaneIds for selection state.
    graph_app.clear_workbench_tile_selection();
    if let Some(Tile::Pane(kind)) = tiles_tree.tiles.get(primary_result_tile_id) {
        graph_app.select_workbench_pane(kind.pane_id());
    }
    for tile_id in &result_tile_ids {
        if *tile_id != primary_result_tile_id {
            if let Some(Tile::Pane(kind)) = tiles_tree.tiles.get(*tile_id) {
                graph_app
                    .update_workbench_pane_selection(kind.pane_id(), SelectionUpdateMode::Add);
            }
        }
    }
    if graph_app
        .persist_workbench_tile_group(tiles_tree, &selection.selected_pane_ids)
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
