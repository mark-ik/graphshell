use super::*;

pub(super) fn handle_update_pane_selection_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    pane_id: PaneId,
    mode: SelectionUpdateMode,
) {
    let live_pane_ids = live_pane_ids_from_tiles_tree(tiles_tree);
    graph_app.update_workbench_pane_selection_if_live(&live_pane_ids, pane_id, mode);
}

pub(super) fn handle_group_selected_tiles_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    let live_pane_ids = live_pane_ids_from_tiles_tree(tiles_tree);
    graph_app.prune_workbench_pane_selection_to_live_set(&live_pane_ids);
    let selection = graph_app.workbench_tile_selection().clone();

    let selected_tile_ids = selected_tile_ids_for_panes(tiles_tree, &selection.selected_pane_ids);
    let primary_tile_id = selection
        .primary_pane_id
        .and_then(|pane_id| tile_id_for_pane(tiles_tree, pane_id));

    let Some((result_tile_ids, primary_result_tile_id)) =
        tile_view_ops::group_selected_tiles(tiles_tree, &selected_tile_ids, primary_tile_id)
    else {
        return false;
    };

    let result_pane_ids = pane_ids_for_tile_ids(tiles_tree, &result_tile_ids);
    let primary_result_pane_id = pane_id_for_tile_id(tiles_tree, primary_result_tile_id);

    graph_app.clear_workbench_tile_selection();
    if let Some(pane_id) = primary_result_pane_id {
        graph_app.select_workbench_pane(pane_id);
    }
    for pane_id in result_pane_ids {
        if Some(pane_id) != primary_result_pane_id {
            graph_app.update_workbench_pane_selection(pane_id, SelectionUpdateMode::Add);
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

fn live_pane_ids_from_tiles_tree(tiles_tree: &Tree<TileKind>) -> std::collections::HashSet<PaneId> {
    tiles_tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            Tile::Pane(kind) => Some(kind.pane_id()),
            _ => None,
        })
        .collect()
}

fn tile_id_for_pane(tiles_tree: &Tree<TileKind>, pane_id: PaneId) -> Option<egui_tiles::TileId> {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(kind) if kind.pane_id() == pane_id => Some(*tile_id),
            _ => None,
        })
}

fn pane_id_for_tile_id(tiles_tree: &Tree<TileKind>, tile_id: egui_tiles::TileId) -> Option<PaneId> {
    match tiles_tree.tiles.get(tile_id) {
        Some(Tile::Pane(kind)) => Some(kind.pane_id()),
        _ => None,
    }
}

fn selected_tile_ids_for_panes(
    tiles_tree: &Tree<TileKind>,
    pane_ids: &std::collections::HashSet<PaneId>,
) -> std::collections::HashSet<egui_tiles::TileId> {
    tiles_tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| match tile {
            Tile::Pane(kind) if pane_ids.contains(&kind.pane_id()) => Some(*tile_id),
            _ => None,
        })
        .collect()
}

fn pane_ids_for_tile_ids(
    tiles_tree: &Tree<TileKind>,
    tile_ids: &std::collections::HashSet<egui_tiles::TileId>,
) -> Vec<PaneId> {
    tile_ids
        .iter()
        .filter_map(|tile_id| pane_id_for_tile_id(tiles_tree, *tile_id))
        .collect()
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
